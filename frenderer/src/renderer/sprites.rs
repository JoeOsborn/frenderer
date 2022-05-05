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
use vulkano::buffer::CpuBufferPool;
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

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Default, Pod, Debug, PartialEq)]
pub struct SingleRenderState {
    region: Rect,
    transform: Isometry3,
    size: Vec2,
    pad: Vec3,
}
impl SingleRenderState {
    pub fn new(region: Rect, transform: Isometry3, size: Vec2) -> Self {
        Self {
            region,
            transform,
            size,
            pad: Vec3::zero(),
        }
    }
}
impl super::SingleRenderState for SingleRenderState {
    fn interpolate(&self, other: &Self, r: f32) -> Self {
        Self {
            transform: self.transform.interpolate_limit(other.transform, r, 10.0),
            size: self.size.interpolate_limit(other.size, r, 0.5),
            region: other.region,
            pad: Vec3::zero(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Default, Pod, Debug, PartialEq)]
struct InstanceData {
    uv_cel: [f32; 4],
    translationxyz_rotorw: [f32; 4],
    rotorxyz_szx: [f32; 4],
    szy_pad: [f32; 4],
}
vulkano::impl_vertex!(
    InstanceData,
    uv_cel,
    translationxyz_rotorw,
    rotorxyz_szx,
    szy_pad
);
struct BatchData {
    material_pds: Arc<vulkano::descriptor_set::PersistentDescriptorSet>,
    instance_data: Vec<InstanceData>,
    instance_buf:
        Option<Arc<CpuBufferPoolChunk<InstanceData, Arc<vulkano::memory::pool::StdMemoryPool>>>>,
    storage_pds: Option<Arc<SingleLayoutDescSet>>,
}

pub struct Renderer {
    pipeline: Arc<vulkano::pipeline::GraphicsPipeline>,
    sampler: Arc<Sampler>,
    // we'll use one uniform buffer across all batches.
    // it will be the projection-view transform.
    uniform_buffers: CpuBufferPool<Mat4>,
    uniform_pds: SingleLayoutDescSetPool,
    uniform_binding: Option<Arc<SingleLayoutDescSet>>,
    instance_sds: SingleLayoutDescSetPool,
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
// instance data---none!
// outputs
layout(location = 0) out vec2 out_uv;

// uniforms
layout(set=0, binding=0) uniform BatchData { mat4 viewproj; };
// storage buffer uniforms
struct SpriteData {
  vec4 uv_cel;
  vec4 translationxyz_rotorw;
  vec4 rotorxyz_szx;
  vec4 szy_pad;
};
layout(std430, set = 2, binding = 0) buffer Sprites { SpriteData data[]; };

mat4 rotor_to_matrix(vec4 rotor) {
  float s = rotor.x;
  float xy = rotor.y;
  float xz = rotor.z;
  float yz = rotor.w;
  float s2 = s * s;
  float bxy2 = xy * xy;
  float bxz2 = xz * xz;
  float byz2 = yz * yz;
  float s_bxy = s * xy;
  float s_bxz = s * xz;
  float s_byz = s * yz;
  float bxz_byz = xz * yz;
  float bxy_byz = xy * yz;
  float bxy_bxz = xy * xz;

  float two = 2.0;

  return mat4(
    vec4(
      s2 - bxy2 - bxz2 + byz2,
      -2.0 * (bxz_byz + s_bxy),
      2.0 * (bxy_byz - s_bxz),
      0.0
    ),
    vec4(
      2.0 * (s_bxy - bxz_byz),
      s2 - bxy2 + bxz2 - byz2,
      -2.0 * (s_byz + bxy_bxz),
      0.0
    ),
    vec4(
      2.0 * (s_bxz + bxy_byz),
      2.0 * (s_byz - bxy_bxz),
      s2 + bxy2 - bxz2 - byz2,
      0.0
    ),
    vec4(0.0,0.0,0.0,1.0)
  );
}

void main() {
  int which_sprite = gl_VertexIndex / 6;
  int which_vert = gl_VertexIndex % 6;
  SpriteData sd = data[which_sprite];
  vec4 uv_cel = sd.uv_cel;
  vec3 translation = sd.translationxyz_rotorw.xyz;
  vec4 rotor = vec4(sd.translationxyz_rotorw.w, sd.rotorxyz_szx.xyz);
  vec2 size = vec2(sd.rotorxyz_szx.w,sd.szy_pad.x);

  mat4 trans = mat4(
    vec4(1.0,0.0,0.0,0.0),
    vec4(0.0,1.0,0.0,0.0),
    vec4(0.0,0.0,1.0,0.0),
    vec4(translation.xyz,1.0)
  );
  mat4 rot = rotor_to_matrix(rotor);
  mat4 scale = mat4(
    vec4(size.x,0.0,0.0,0.0),
    vec4(0.0,size.y,0.0,0.0),
    vec4(0.0,0.0,1.0,0.0),
    vec4(0.0,0.0,0.0,1.0)
  );
  mat4 model = trans * rot * scale;
  float w = uv_cel.z;
  float h = uv_cel.w;
  vec2 uv = uv_cel.xy;
  // 0: TL, 1: BL, 2: BR, 3: TR
  vec2 posns[] = {
    vec2(-0.5, 0.5), // 0
    vec2(-0.5, -0.5),// 1
    vec2(0.5, -0.5), // 2
    vec2(0.5, 0.5),  // 3
    vec2(-0.5, 0.5), // 0 again
    vec2(0.5, -0.5), // 2 again
    vec2(0.5, 0.5),  // 3 again
  };
  vec2 pos = posns[which_vert].xy;
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
            .vertex_input_state(BuffersDefinition::new())
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
        let instance_pool = CpuBufferPool::new(
            vulkan.device.clone(),
            vulkano::buffer::BufferUsage {
                storage_buffer: true,
                uniform_buffer: true,
                transfer_destination: true,
                ..Default::default()
            },
        );
        let instance_sds =
            SingleLayoutDescSetPool::new(pipeline.layout().set_layouts().get(2).unwrap().clone());

        Self {
            sampler,
            pipeline,
            uniform_buffers,
            uniform_pds,
            instance_sds,
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
        let insts = dat
            .into_iter()
            .map(|srs| unsafe { std::mem::transmute(*srs) });
        match self.batches.entry(tr) {
            Entry::Vacant(v) => {
                let mut b =
                    Self::create_batch(self.pipeline.clone(), self.sampler.clone(), texture);
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
    ) -> BatchData {
        BatchData {
            instance_data: vec![],
            instance_buf: None,
            storage_pds: None,
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
        for (tex_id, (_ks, vs)) in rs.sprites.interpolated.iter() {
            let tex = assets.texture(*tex_id);
            self.push_models(*tex_id, tex, vs.borrow().iter());
        }
        for (tex_id, vs) in rs.sprites.raw.iter() {
            let tex = assets.texture(*tex_id);
            for srs_vec in vs.iter() {
                self.push_models(*tex_id, tex, srs_vec.borrow().iter());
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
            b.prepare_draw(&self.instance_pool, &mut self.instance_sds);
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
        slds_pool: &mut SingleLayoutDescSetPool,
    ) {
        let chunk = instance_pool
            .chunk(self.instance_data.iter().copied())
            .unwrap();
        self.instance_buf = Some(chunk.clone());
        self.storage_pds = Some(
            slds_pool
                .next([vulkano::descriptor_set::WriteDescriptorSet::buffer(
                    0, chunk,
                )])
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
            .bind_descriptor_sets(
                vulkano::pipeline::PipelineBindPoint::Graphics,
                (*pipeline).layout().clone(),
                2,
                self.storage_pds.clone().unwrap(),
            )
            .draw(6 * self.instance_data.len() as u32, 1, 0, 0)
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
