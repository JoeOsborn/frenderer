use std::borrow::Cow;

use crate::gpu::WGPU;
use wgpu::util::DeviceExt;

pub struct ColorGeo {
    pipeline: wgpu::RenderPipeline,
    transform_bind_group: wgpu::BindGroup,
    texture_bind_group: wgpu::BindGroup,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    transform: Transform,
    transform_buf: wgpu::Buffer,
    colormod: ColorTransform,
    colormod_buf: wgpu::Buffer,
    color_texture_view: wgpu::TextureView,
    lut_texture_view: wgpu::TextureView,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
struct Transform {
    mat: [f32; 16],
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Zeroable, bytemuck::Pod)]
struct ColorTransform {
    mat: [f32; 16],
    saturation_padding: [f32; 4],
}

pub fn lut_identity(gpu: &WGPU) -> wgpu::Texture {
    const CUBE: u32 = 64;
    gpu.device().create_texture_with_data(
        gpu.queue(),
        &wgpu::TextureDescriptor {
            label: Some("lut:identity"),
            size: wgpu::Extent3d {
                width: CUBE,
                height: CUBE,
                depth_or_array_layers: CUBE,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        // red horizontal
        // green vertical
        // blue depth
        &(0..CUBE)
            .flat_map(|z| {
                let b = z as f32 / CUBE as f32;
                (0..CUBE).flat_map(move |y| {
                    let g = y as f32 / CUBE as f32;
                    (0..CUBE).flat_map(move |x| {
                        let r = x as f32 / CUBE as f32;
                        [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, 255]
                    })
                })
            })
            .collect::<Vec<u8>>(),
    )
}

impl ColorGeo {
    pub fn new(
        gpu: &WGPU,
        color_texture: &wgpu::Texture,
        lut_texture: &wgpu::Texture,
        color_target: wgpu::ColorTargetState,
    ) -> Self {
        let shader = gpu
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("post:shader"),
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("colorgeo.wgsl"))),
            });
        let transform_bind_group_layout =
            gpu.device()
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("post:transform_bgl"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: std::num::NonZeroU64::new(
                                std::mem::size_of::<Transform>() as u64,
                            ),
                        },
                        count: None,
                    }],
                });
        let texture_bind_group_layout =
            gpu.device()
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("post:colormod_texture_bgl"),
                    // It needs the first entry for the texture and the second for the sampler.
                    // This is like defining a type signature.
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<
                                    ColorTransform,
                                >(
                                )
                                    as u64),
                            },
                            count: None,
                        },
                        // Color texture binding
                        wgpu::BindGroupLayoutEntry {
                            // This matches the binding in the shader
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            // It's a texture binding
                            ty: wgpu::BindingType::Texture {
                                // We can use it with float samplers
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                // It's being used as a 2D texture
                                view_dimension: wgpu::TextureViewDimension::D2,
                                // This is not a multisampled texture
                                multisampled: false,
                            },
                            count: None,
                        },
                        // The sampler binding
                        wgpu::BindGroupLayoutEntry {
                            // This matches the binding in the shader
                            binding: 2,
                            // Only available in the fragment shader
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            // It's a sampler
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            // No count
                            count: None,
                        },
                        // LUT texture binding
                        wgpu::BindGroupLayoutEntry {
                            // This matches the binding in the shader
                            binding: 3,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            // It's a texture binding
                            ty: wgpu::BindingType::Texture {
                                // We can use it with float samplers
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                // It's being used as a 3D texture
                                view_dimension: wgpu::TextureViewDimension::D3,
                                // This is not a multisampled texture
                                multisampled: false,
                            },
                            count: None,
                        },
                        // The sampler binding
                        wgpu::BindGroupLayoutEntry {
                            // This matches the binding in the shader
                            binding: 4,
                            // Only available in the fragment shader
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            // It's a sampler
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            // No count
                            count: None,
                        },
                    ],
                });
        let pipeline_layout =
            gpu.device()
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("post:pipeline_layout"),
                    bind_group_layouts: &[&transform_bind_group_layout, &texture_bind_group_layout],
                    push_constant_ranges: &[],
                });
        let transform = Transform {
            mat: [
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        };
        let colormod = ColorTransform {
            mat: [
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
            saturation_padding: [0.0; 4],
        };
        let transform_buf = gpu
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("post:transform_buffer"),
                contents: bytemuck::bytes_of(&transform),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let colormod_buf = gpu
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("post:colormod_buffer"),
                contents: bytemuck::bytes_of(&colormod),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let transform_bind_group = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("post:transform_bg"),
            layout: &transform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &transform_buf,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        let color_texture_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let lut_texture_view = lut_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let texture_bind_group = Self::create_bind_group(
            &texture_bind_group_layout,
            &colormod_buf,
            &color_texture_view,
            &lut_texture_view,
            gpu,
        );

        let pipeline = gpu
            .device()
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("post:pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_vbuf_main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(color_target)],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        Self {
            pipeline,
            transform,
            colormod,
            transform_buf,
            colormod_buf,
            transform_bind_group,
            texture_bind_group_layout,
            texture_bind_group,
            color_texture_view,
            lut_texture_view,
        }
    }
    pub fn set_post(&mut self, gpu: &WGPU, trf: [f32; 16], color_trf: [f32; 16], sat: f32) {
        // update buffers
        self.transform.mat = trf;
        self.colormod.mat = color_trf;
        self.colormod.saturation_padding = [sat, 0.0, 0.0, 0.0];
        gpu.queue()
            .write_buffer(&self.transform_buf, 0, bytemuck::bytes_of(&self.transform));
        gpu.queue()
            .write_buffer(&self.colormod_buf, 0, bytemuck::bytes_of(&self.colormod));
    }
    pub fn replace_color_texture(&mut self, gpu: &WGPU, color: &wgpu::Texture) {
        self.color_texture_view = color.create_view(&wgpu::TextureViewDescriptor::default());
        self.texture_bind_group = Self::create_bind_group(
            &self.texture_bind_group_layout,
            &self.colormod_buf,
            &self.color_texture_view,
            &self.lut_texture_view,
            gpu,
        );
    }
    pub fn replace_lut(&mut self, gpu: &WGPU, lut: &wgpu::Texture) {
        self.lut_texture_view = lut.create_view(&wgpu::TextureViewDescriptor::default());
        self.texture_bind_group = Self::create_bind_group(
            &self.texture_bind_group_layout,
            &self.colormod_buf,
            &self.color_texture_view,
            &self.lut_texture_view,
            gpu,
        );
    }
    fn create_bind_group(
        texture_bind_group_layout: &wgpu::BindGroupLayout,
        colormod_buf: &wgpu::Buffer,
        color_texture_view: &wgpu::TextureView,
        lut_texture_view: &wgpu::TextureView,
        gpu: &WGPU,
    ) -> wgpu::BindGroup {
        gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("post:colormod_texture_bg"),
            layout: texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: colormod_buf,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(color_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&gpu.device().create_sampler(
                        &wgpu::SamplerDescriptor {
                            label: Some("post:color_sampler"),
                            address_mode_u: wgpu::AddressMode::ClampToEdge,
                            address_mode_v: wgpu::AddressMode::ClampToEdge,
                            mag_filter: wgpu::FilterMode::Nearest,
                            min_filter: wgpu::FilterMode::Nearest,
                            ..Default::default()
                        },
                    )),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(lut_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&gpu.device().create_sampler(
                        &wgpu::SamplerDescriptor {
                            label: Some("post:lut_sampler"),
                            address_mode_u: wgpu::AddressMode::ClampToEdge,
                            address_mode_v: wgpu::AddressMode::ClampToEdge,
                            address_mode_w: wgpu::AddressMode::ClampToEdge,
                            mag_filter: wgpu::FilterMode::Linear,
                            min_filter: wgpu::FilterMode::Linear,
                            ..Default::default()
                        },
                    )),
                },
            ],
        })
    }
    pub fn render<'s, 'pass>(&'s self, rpass: &mut wgpu::RenderPass<'pass>)
    where
        's: 'pass,
    {
        rpass.set_pipeline(&self.pipeline);
        // todo future: subdivide quad according to params
        rpass.set_bind_group(0, &self.transform_bind_group, &[]);
        rpass.set_bind_group(1, &self.texture_bind_group, &[]);
        // todo future: rpass.set_bind_group(2, self.lut_bind_group);
        rpass.draw(0..6, 0..1);
    }

    pub fn transform(&self) -> [f32; 16] {
        self.transform.mat
    }
    pub fn color_transform(&self) -> [f32; 16] {
        self.colormod.mat
    }
    pub fn saturation(&self) -> f32 {
        self.colormod.saturation_padding[0]
    }
    pub fn set_transform(&mut self, gpu: &WGPU, mat: [f32; 16]) {
        self.set_post(
            gpu,
            mat,
            self.colormod.mat,
            self.colormod.saturation_padding[0],
        );
    }
    pub fn set_color_transform(&mut self, gpu: &WGPU, mat: [f32; 16]) {
        self.set_post(
            gpu,
            self.transform.mat,
            mat,
            self.colormod.saturation_padding[0],
        );
    }
    pub fn set_saturation(&mut self, gpu: &WGPU, sat: f32) {
        self.set_post(gpu, self.transform.mat, self.colormod.mat, sat);
    }
}
