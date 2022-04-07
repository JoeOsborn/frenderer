use crate::assets::{self, MaterialRef, MeshRef};
use crate::camera::Camera;
use crate::types::*;
use crate::vulkan::Vulkan;
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use vulkano::buffer::cpu_pool::CpuBufferPoolChunk;
use vulkano::buffer::CpuBufferPool;
use vulkano::buffer::ImmutableBuffer;
use vulkano::buffer::TypedBufferAccess;
use vulkano::command_buffer::AutoCommandBufferBuilder;
use vulkano::descriptor_set::single_layout_pool::SingleLayoutDescSet;
use vulkano::descriptor_set::PersistentDescriptorSet;
use vulkano::descriptor_set::SingleLayoutDescSetPool;
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::rasterization::RasterizationState;
use vulkano::pipeline::graphics::vertex_input::BuffersDefinition;
use vulkano::pipeline::graphics::viewport::ViewportState;
use vulkano::pipeline::GraphicsPipeline;
use vulkano::pipeline::Pipeline;
use vulkano::render_pass::Subpass;

#[derive(Clone, Debug)]
pub struct Material {
    color: Vec4,
    buffer: Arc<ImmutableBuffer<[f32; 4]>>,
    name: String,
}
impl Material {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn color(&self) -> Vec4 {
        self.color
    }
    pub(crate) fn new(color: Vec4, name: String, buffer: Arc<ImmutableBuffer<[f32; 4]>>) -> Self {
        Self {
            color,
            name,
            buffer,
        }
    }
}
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Model {
    materials: Vec<MaterialRef<Material>>,
    meshes: Vec<MeshRef<Mesh>>,
}
impl Model {
    pub fn new(meshes: Vec<MeshRef<Mesh>>, materials: Vec<MaterialRef<Material>>) -> Self {
        Self { materials, meshes }
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ModelKey(assets::MeshRef<Mesh>, assets::MaterialRef<Material>);

#[repr(C)]
#[derive(Default, Debug, Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
}
vulkano::impl_vertex!(Vertex, position);
pub struct Mesh {
    pub mesh: russimp::mesh::Mesh,
    pub verts: Arc<ImmutableBuffer<[Vertex]>>,
    pub idx: Arc<ImmutableBuffer<[u32]>>,
}

pub struct SingleRenderState {
    model: Rc<Model>,
    transform: Similarity3,
}
impl SingleRenderState {
    pub fn new(model: Rc<Model>, transform: Similarity3) -> Self {
        Self { model, transform }
    }
    pub fn interpolate(&self, other: &Self, r: f32) -> Self {
        Self {
            model: other.model.clone(),
            transform: self.transform.lerp(&other.transform, r),
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Zeroable, Default, Pod, Debug, PartialEq)]
struct InstanceData {
    model: [f32; 4 * 4],
}
vulkano::impl_vertex!(InstanceData, model);

struct BatchData {
    verts: Arc<ImmutableBuffer<[Vertex]>>,
    idxs: Arc<ImmutableBuffer<[u32]>>,
    material_pds: Arc<vulkano::descriptor_set::PersistentDescriptorSet>,
    instance_data: Vec<InstanceData>,
    instance_buf:
        Option<Arc<CpuBufferPoolChunk<InstanceData, Arc<vulkano::memory::pool::StdMemoryPool>>>>,
}

pub struct Renderer {
    pipeline: Arc<vulkano::pipeline::GraphicsPipeline>,
    // we'll use one uniform buffer across all batches.
    // it will be the projection-view transform.
    uniform_buffers: CpuBufferPool<Mat4>,
    uniform_pds: SingleLayoutDescSetPool,
    uniform_binding: Option<Arc<SingleLayoutDescSet>>,
    instance_pool: CpuBufferPool<InstanceData, Arc<vulkano::memory::pool::StdMemoryPool>>,
    batches: HashMap<ModelKey, BatchData>,
}

impl Renderer {
    pub fn new(vulkan: &mut Vulkan) -> Self {
        mod vs {
            vulkano_shaders::shader! {
                ty: "vertex",
                src: "
#version 450

// vertex attributes
layout(location = 0) in vec3 position;
// instance data
layout(location = 1) in mat4 model;

// uniforms
layout(set=0, binding=0) uniform BatchData { mat4 viewproj; };

void main() {
  gl_Position = viewproj * model * vec4(position.xyz, 1.0);
}
                "
            }
        }

        mod fs {
            vulkano_shaders::shader! {
                ty: "fragment",
                src: "
                #version 450

                layout(set = 1, binding = 0) uniform Material {vec4 color;};
                layout(location = 0) out vec4 f_color;

                void main() {
                    if (color.a < 0.1) { discard; }
                    f_color = color;
                }
            "
            }
        }

        let vs = vs::load(vulkan.device.clone()).unwrap();
        let fs = fs::load(vulkan.device.clone()).unwrap();
        use vulkano::pipeline::graphics::depth_stencil::*;
        let pipeline = GraphicsPipeline::start()
            .vertex_input_state(
                BuffersDefinition::new()
                    .vertex::<Vertex>()
                    .instance::<InstanceData>(),
            )
            .vertex_shader(vs.entry_point("main").unwrap(), ())
            .input_assembly_state(InputAssemblyState::new().topology(
                vulkano::pipeline::graphics::input_assembly::PrimitiveTopology::TriangleList,
            ))
            .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
            .fragment_shader(fs.entry_point("main").unwrap(), ())
            .rasterization_state(
                RasterizationState::new()
                    .cull_mode(vulkano::pipeline::graphics::rasterization::CullMode::Back)
                    .front_face(
                        vulkano::pipeline::graphics::rasterization::FrontFace::CounterClockwise,
                    ),
            )
            .depth_stencil_state(DepthStencilState {
                depth: Some(DepthState {
                    compare_op: vulkano::pipeline::StateMode::Fixed(CompareOp::Greater),
                    enable_dynamic: false,
                    write_enable: vulkano::pipeline::StateMode::Fixed(true),
                }),
                depth_bounds: None,
                stencil: None,
            })
            .render_pass(Subpass::from(vulkan.render_pass.clone(), 0).unwrap())
            .build(vulkan.device.clone())
            .unwrap();

        let uniform_buffers = CpuBufferPool::uniform_buffer(vulkan.device.clone());
        let uniform_pds =
            SingleLayoutDescSetPool::new(pipeline.layout().set_layouts().get(0).unwrap().clone());

        let instance_pool = CpuBufferPool::vertex_buffer(vulkan.device.clone());

        Self {
            pipeline,
            uniform_buffers,
            uniform_pds,
            instance_pool,
            batches: HashMap::new(),
            uniform_binding: None,
        }
    }
    pub(crate) fn push_model(
        &mut self,
        key: ModelKey,
        mesh: &Mesh,
        material: &Material,
        trf: Similarity3,
    ) {
        use std::collections::hash_map::Entry;
        let inst = InstanceData {
            model: *trf.into_homogeneous_matrix().as_array(),
        };
        match self.batches.entry(key) {
            Entry::Vacant(v) => {
                let mut b = Self::create_batch(self.pipeline.clone(), mesh, material);
                b.push_instance(inst);
                v.insert(b);
            }
            Entry::Occupied(v) => v.into_mut().push_instance(inst),
        }
    }
    fn create_batch(
        pipeline: Arc<vulkano::pipeline::GraphicsPipeline>,
        mesh: &Mesh,
        material: &Material,
    ) -> BatchData {
        BatchData {
            verts: mesh.verts.clone(),
            idxs: mesh.idx.clone(),
            instance_data: vec![],
            instance_buf: None,
            material_pds: PersistentDescriptorSet::new(
                pipeline.layout().set_layouts().get(1).unwrap().clone(),
                [vulkano::descriptor_set::WriteDescriptorSet::buffer(
                    0,
                    material.buffer.clone(),
                )],
            )
            .unwrap(),
        }
    }
    pub fn prepare(&mut self, rs: &super::RenderState, assets: &assets::Assets, camera: &Camera) {
        for v in rs.flats.values() {
            for (meshr, matr) in v.model.meshes.iter().zip(v.model.materials.iter()) {
                let mesh = assets.flat_mesh(*meshr);
                let mat = assets.material(*matr);
                self.push_model(ModelKey(*meshr, *matr), mesh, mat, v.transform);
            }
        }
        self.prepare_draw(camera);
    }
    fn prepare_draw(&mut self, camera: &Camera) {
        let buf = self.uniform_buffers.next(camera.as_matrix()).unwrap();
        let uds = self
            .uniform_pds
            .next(vec![vulkano::descriptor_set::WriteDescriptorSet::buffer(
                0, buf,
            )])
            .unwrap();
        self.uniform_binding = Some(uds);
        for (_k, b) in self.batches.iter_mut() {
            b.prepare_draw(&self.instance_pool);
        }
    }
    pub fn draw<P, L>(&mut self, builder: &mut AutoCommandBufferBuilder<P, L>) {
        let uds = self.uniform_binding.clone().unwrap();

        builder.bind_pipeline_graphics(self.pipeline.clone());

        for (_b, dat) in self.batches.iter() {
            dat.draw(self.pipeline.clone(), uds.clone(), builder);
        }
        self.clear_frame();
    }
    fn clear_frame(&mut self) {
        // delete batch data for objects that didn't get rendered this frame.
        // TODO: something more sophisticated!
        self.batches.retain(|_k, v| !v.is_empty());
        // delete instance data from each batch, but don't throw away the vecs' allocations
        self.batches.iter_mut().for_each(|(_k, v)| v.clear_frame());
    }
}

impl BatchData {
    fn prepare_draw(
        &mut self,
        instance_pool: &CpuBufferPool<InstanceData, Arc<vulkano::memory::pool::StdMemoryPool>>,
    ) {
        self.instance_buf = Some(
            instance_pool
                .chunk(self.instance_data.iter().copied())
                .unwrap(),
        );
    }
    fn draw<P, L>(
        &self,
        pipeline: Arc<GraphicsPipeline>,
        unis: Arc<vulkano::descriptor_set::single_layout_pool::SingleLayoutDescSet>,
        builder: &mut AutoCommandBufferBuilder<P, L>,
    ) {
        builder
            .bind_vertex_buffers(0, [self.verts.clone()])
            .bind_vertex_buffers(1, [self.instance_buf.clone().unwrap()])
            .bind_index_buffer(self.idxs.clone())
            .bind_descriptor_sets(
                vulkano::pipeline::PipelineBindPoint::Graphics,
                (*pipeline).layout().clone(),
                0,
                unis,
            )
            .bind_descriptor_sets(
                vulkano::pipeline::PipelineBindPoint::Graphics,
                (*pipeline).layout().clone(),
                1,
                self.material_pds.clone(),
            )
            .draw_indexed(
                self.idxs.len() as u32,
                self.instance_data.len() as u32,
                0,
                0,
                0,
            )
            .unwrap();
    }
    fn clear_frame(&mut self) {
        self.instance_data.clear();
    }
    fn is_empty(&self) -> bool {
        self.instance_data.is_empty()
    }
    fn push_instance(&mut self, inst: InstanceData) {
        self.instance_data.push(inst);
    }
}
