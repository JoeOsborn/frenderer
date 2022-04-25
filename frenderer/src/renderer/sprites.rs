use super::RenderState;
use crate::assets;
use crate::assets::Texture;
use crate::camera::Camera;
use crate::types::*;
use crate::vulkan::Vulkan;
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
use std::sync::Arc;
use vulkano::buffer::cpu_pool::CpuBufferPoolChunk;
use vulkano::buffer::{BufferUsage, CpuBufferPool, ImmutableBuffer};
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
use vulkano::sampler::Sampler;

#[derive(Clone)]
pub struct SingleRenderState {
    region: Rect,
    transform: Isometry3,
    size: Vec2,
}
impl SingleRenderState {
    pub fn new(region: Rect, transform: Isometry3, size: Vec2) -> Self {
        Self {
            region,
            transform,
            size,
        }
    }
}
impl super::SingleRenderState for SingleRenderState {
    fn interpolate(&self, other: &Self, r: f32) -> Self {
        Self {
            transform: self.transform.interpolate_limit(other.transform, r, 10.0),
            size: self.size.interpolate_limit(other.size, r, 0.5),
            region: other.region,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Default, Pod, Debug, PartialEq)]
struct InstanceData {
    model: [f32; 4 * 4],
    size_uv: [f32; 4],
}
vulkano::impl_vertex!(InstanceData, model, size_uv);
struct BatchData {
    material_pds: Arc<vulkano::descriptor_set::PersistentDescriptorSet>,
    instance_data: Vec<InstanceData>,
    instance_buf:
        Option<Arc<CpuBufferPoolChunk<InstanceData, Arc<vulkano::memory::pool::StdMemoryPool>>>>,
    index_buf: Arc<ImmutableBuffer<[u16]>>,
}

pub struct Renderer {
    pipeline: Arc<vulkano::pipeline::GraphicsPipeline>,
    sampler: Arc<Sampler>,
    // we'll use one uniform buffer across all batches.
    // it will be the projection-view transform.
    uniform_buffers: CpuBufferPool<Mat4>,
    uniform_pds: SingleLayoutDescSetPool,
    uniform_binding: Option<Arc<SingleLayoutDescSet>>,
    index_buf: Arc<ImmutableBuffer<[u16]>>,
    instance_pool: CpuBufferPool<InstanceData, Arc<vulkano::memory::pool::StdMemoryPool>>,
    batches: HashMap<assets::TextureRef, BatchData>,
}
impl super::Renderer for Renderer {
    type BatchRenderKey = assets::TextureRef;
    type SingleRenderState = SingleRenderState;
}
impl Renderer {
    pub fn new(vulkan: &mut Vulkan, cull_back_faces: bool) -> Self {
        mod vs {
            vulkano_shaders::shader! {
                ty: "vertex",
                src: "
#version 450

// vertex attributes---none!
// instance data
layout(location = 0) in mat4 model;
layout(location = 4) in vec4 size_uv;

// outputs
layout(location = 0) out vec2 out_uv;

// uniforms
layout(set=0, binding=0) uniform BatchData { mat4 viewproj; };

void main() {
  float w = size_uv.x;
  float h = size_uv.y;
  vec2 uv = size_uv.zw;
  // 0: TL, 1: BL, 2: BR, 3: TR
  vec2 posns[] = {
    vec2(-0.5, 0.5),
    vec2(-0.5, -0.5),
    vec2(0.5, -0.5),
    vec2(0.5, 0.5),
  };
  vec2 pos = posns[gl_VertexIndex].xy;
  gl_Position = viewproj * model * vec4(pos.xy, 0.0, 1.0);
  out_uv = vec2(uv.x+(pos.x+0.5)*w,1.0-(uv.y+(pos.y+0.5)*h));
}
"
            }
        }

        mod fs {
            vulkano_shaders::shader! {
                ty: "fragment",
                src: "
                #version 450

                layout(set = 1, binding = 0) uniform sampler2D tex;
                layout(location = 0) in vec2 uv;
                layout(location = 0) out vec4 f_color;

                void main() {
                    vec4 col = texture(tex, uv);
                    //col = vec4(1.0, 1.0, 0.0, 1.0);
                    if (col.a < 0.1) { discard; }
                    f_color = col;
                }
            "
            }
        }

        let vs = vs::load(vulkan.device.clone()).unwrap();
        let fs = fs::load(vulkan.device.clone()).unwrap();
        use vulkano::sampler::SamplerCreateInfo;
        let sampler = Sampler::new(vulkan.device.clone(), SamplerCreateInfo::default()).unwrap();
        use vulkano::pipeline::graphics::depth_stencil::*;
        let pipeline = GraphicsPipeline::start()
            .vertex_input_state(BuffersDefinition::new().instance::<InstanceData>())
            .vertex_shader(vs.entry_point("main").unwrap(), ())
            .input_assembly_state(InputAssemblyState::new().topology(
                vulkano::pipeline::graphics::input_assembly::PrimitiveTopology::TriangleList,
            ))
            .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
            .fragment_shader(fs.entry_point("main").unwrap(), ())
            .rasterization_state(
                RasterizationState::new()
                    .cull_mode(if cull_back_faces {
                        vulkano::pipeline::graphics::rasterization::CullMode::Back
                    } else {
                        vulkano::pipeline::graphics::rasterization::CullMode::None
                    })
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

        let (index_buf, fut) = ImmutableBuffer::from_iter(
            [0_u16, 1, 2, 0, 2, 3].into_iter(),
            BufferUsage::index_buffer(),
            vulkan.queue.clone(),
        )
        .unwrap();
        vulkan.wait_for(Box::new(fut));

        Self {
            sampler,
            pipeline,
            uniform_buffers,
            uniform_pds,
            index_buf,
            instance_pool,
            batches: HashMap::new(),
            uniform_binding: None,
        }
    }
    pub fn push_models<'a>(
        &mut self,
        tr: assets::TextureRef,
        texture: &Texture,
        dat: impl IntoIterator<Item = &'a SingleRenderState>,
    ) {
        use std::collections::hash_map::Entry;
        let insts = dat.into_iter().map(|srs| InstanceData {
            model: *(srs.transform.into_homogeneous_matrix()
                * Mat4::from_nonuniform_scale(Vec3::new(srs.size.x, srs.size.y, 1.0)))
            .as_array(),
            size_uv: [
                srs.region.sz.x,
                srs.region.sz.y,
                srs.region.pos.x,
                srs.region.pos.y,
            ],
        });
        match self.batches.entry(tr) {
            Entry::Vacant(v) => {
                let mut b = Self::create_batch(
                    self.pipeline.clone(),
                    self.sampler.clone(),
                    texture,
                    self.index_buf.clone(),
                );
                b.push_instances(insts);
                v.insert(b);
            }
            Entry::Occupied(v) => v.into_mut().push_instances(insts),
        }
    }
    fn create_batch(
        pipeline: Arc<vulkano::pipeline::GraphicsPipeline>,
        sampler: Arc<Sampler>,
        texture: &Texture,
        index_buf: Arc<ImmutableBuffer<[u16]>>,
    ) -> BatchData {
        BatchData {
            instance_data: vec![],
            instance_buf: None,
            index_buf,
            material_pds: PersistentDescriptorSet::new(
                pipeline.layout().set_layouts().get(1).unwrap().clone(),
                [
                    vulkano::descriptor_set::WriteDescriptorSet::image_view_sampler(
                        0,
                        vulkano::image::view::ImageView::new_default(texture.texture.clone())
                            .unwrap(),
                        sampler,
                    ),
                ],
            )
            .unwrap(),
        }
    }
    pub fn prepare(&mut self, rs: &RenderState, assets: &assets::Assets, camera: &Camera) {
        for (tex_id, v) in rs.sprites.interpolated.values() {
            let tex = assets.texture(*tex_id);
            self.push_models(*tex_id, tex, std::iter::once(v));
        }
        for (tex_id, vs) in rs.sprites.raw.iter() {
            let tex = assets.texture(*tex_id);
            self.push_models(*tex_id, tex, vs);
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
            .bind_vertex_buffers(0, [self.instance_buf.clone().unwrap()])
            .bind_index_buffer(self.index_buf.clone())
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
            .draw_indexed(6, self.instance_data.len() as u32, 0, 0, 0)
            .unwrap();
    }
    fn clear_frame(&mut self) {
        self.instance_data.clear();
    }
    fn is_empty(&self) -> bool {
        self.instance_data.is_empty()
    }
    fn push_instances(&mut self, insts: impl IntoIterator<Item = InstanceData>) {
        self.instance_data.extend(insts);
    }
}
