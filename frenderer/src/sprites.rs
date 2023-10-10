//! A sprite renderer with multiple layers ("sprite groups") which can
//! be independently transformed.

use std::{borrow::Cow, ops::Range};

use crate::{USE_STORAGE, WGPU};
use bytemuck::{Pod, Zeroable};

/// GPUSprite is in essence a blit operation to be carried out on the
/// GPU, with a destination region (in world coordinates) and a
/// spritesheet region (in normalized texture coordinates).
#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct Region {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// GPUCamera is a transform for a sprite layer, defining a scale
/// followed by a translation.
#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod)]
pub struct GPUCamera {
    pub screen_pos: [f32; 2],
    pub screen_size: [f32; 2],
}

#[allow(dead_code)]
struct SpriteGroup {
    tex: wgpu::Texture,
    world_buffer: wgpu::Buffer,
    sheet_buffer: wgpu::Buffer,
    world_regions: Vec<Region>,
    sheet_regions: Vec<Region>,
    camera: GPUCamera,
    camera_buffer: wgpu::Buffer,
    tex_bind_group: wgpu::BindGroup,
    sprite_bind_group: wgpu::BindGroup,
}

/// SpriteRenderer hosts a number of sprite layers (called groups).
/// Each layer has a specified spritesheet texture, a vector of
/// [`GPUSprite`], and a [`GPUCamera`] to define its transform.
pub struct SpriteRenderer {
    pipeline: wgpu::RenderPipeline,
    sprite_bind_group_layout: wgpu::BindGroupLayout,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    groups: Vec<SpriteGroup>,
}

impl SpriteRenderer {
    pub(crate) fn new(gpu: &WGPU) -> Self {
        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
            });

        let texture_bind_group_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    // It needs the first entry for the texture and the second for the sampler.
                    // This is like defining a type signature.
                    entries: &[
                        // The texture binding
                        wgpu::BindGroupLayoutEntry {
                            // This matches the binding in the shader
                            binding: 0,
                            // Only available in the fragment shader
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
                            binding: 1,
                            // Only available in the fragment shader
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            // It's a sampler
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            // No count
                            count: None,
                        },
                    ],
                });
        // The camera binding
        let camera_layout_entry = wgpu::BindGroupLayoutEntry {
            // This matches the binding in the shader
            binding: 0,
            // Available in vertex shader
            visibility: wgpu::ShaderStages::VERTEX,
            // It's a buffer
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            // No count, not a buffer array binding
            count: None,
        };
        let sprite_bind_group_layout = if USE_STORAGE {
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[
                        camera_layout_entry,
                        wgpu::BindGroupLayoutEntry {
                            // This matches the binding in the shader
                            binding: 1,
                            // Available in vertex shader
                            visibility: wgpu::ShaderStages::VERTEX,
                            // It's a buffer
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            // No count, not a buffer array binding
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            // This matches the binding in the shader
                            binding: 2,
                            // Available in vertex shader
                            visibility: wgpu::ShaderStages::VERTEX,
                            // It's a buffer
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            // No count, not a buffer array binding
                            count: None,
                        },
                    ],
                })
        } else {
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[camera_layout_entry],
                })
        };
        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&sprite_bind_group_layout, &texture_bind_group_layout],
                push_constant_ranges: &[],
            });

        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: if USE_STORAGE {
                        "vs_storage_main"
                    } else {
                        "vs_vbuf_main"
                    },
                    buffers: if USE_STORAGE {
                        &[]
                    } else {
                        &[
                            wgpu::VertexBufferLayout {
                                array_stride: std::mem::size_of::<Region>() as u64,
                                step_mode: wgpu::VertexStepMode::Instance,
                                attributes: &[wgpu::VertexAttribute {
                                    format: wgpu::VertexFormat::Float32x4,
                                    offset: 0,
                                    shader_location: 0,
                                }],
                            },
                            wgpu::VertexBufferLayout {
                                array_stride: std::mem::size_of::<Region>() as u64,
                                step_mode: wgpu::VertexStepMode::Instance,
                                attributes: &[wgpu::VertexAttribute {
                                    format: wgpu::VertexFormat::Float32x4,
                                    offset: 0,
                                    shader_location: 1,
                                }],
                            },
                        ]
                    },
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(gpu.config.format.into())],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        Self {
            pipeline,
            groups: Vec::default(),
            sprite_bind_group_layout,
            texture_bind_group_layout,
        }
    }
    /// Create a new sprite group sized to fit `sprites`.  Returns a
    /// sprite group handle (for now, a usize).
    pub fn add_sprite_group(
        &mut self,
        gpu: &WGPU,
        tex: wgpu::Texture,
        world_regions: Vec<Region>,
        sheet_regions: Vec<Region>,
        camera: GPUCamera,
    ) -> usize {
        let view_sprite = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler_sprite = gpu
            .device
            .create_sampler(&wgpu::SamplerDescriptor::default());
        let tex_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.texture_bind_group_layout,
            entries: &[
                // One for the texture, one for the sampler
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view_sprite),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler_sprite),
                },
            ],
        });
        let buffer_world = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: world_regions.len() as u64 * std::mem::size_of::<Region>() as u64,
            usage: if USE_STORAGE {
                wgpu::BufferUsages::STORAGE
            } else {
                wgpu::BufferUsages::VERTEX
            } | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let buffer_sheet = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: sheet_regions.len() as u64 * std::mem::size_of::<Region>() as u64,
            usage: if USE_STORAGE {
                wgpu::BufferUsages::STORAGE
            } else {
                wgpu::BufferUsages::VERTEX
            } | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: std::mem::size_of::<GPUCamera>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sprite_bind_group = if USE_STORAGE {
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.sprite_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: camera_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: buffer_world.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: buffer_sheet.as_entire_binding(),
                    },
                ],
            })
        } else {
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.sprite_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                }],
            })
        };
        gpu.queue
            .write_buffer(&buffer_world, 0, bytemuck::cast_slice(&world_regions));
        gpu.queue
            .write_buffer(&buffer_sheet, 0, bytemuck::cast_slice(&sheet_regions));
        gpu.queue
            .write_buffer(&camera_buffer, 0, bytemuck::bytes_of(&camera));
        self.groups.push(SpriteGroup {
            tex,
            world_buffer: buffer_world,
            sheet_buffer: buffer_sheet,
            world_regions,
            sheet_regions,
            tex_bind_group,
            sprite_bind_group,
            camera,
            camera_buffer,
        });
        self.groups.len() - 1
    }
    /// Deletes a sprite group.  Note that this currently invalidates
    /// all the old handles, which is not great.  Only use it on the
    /// last sprite group if that matters to you.
    pub fn remove_sprite_group(&mut self, which: usize) {
        self.groups.remove(which);
    }
    /// Resizes a sprite group.  If the new size is smaller, this is
    /// very cheap; if it's larger, it might involve reallocating the
    /// [`Vec<GPUSprite>`] or the GPU buffer used to draw sprites, so
    /// it could be expensive.
    pub fn resize_sprite_group(&mut self, gpu: &WGPU, which: usize, len: usize) -> usize {
        let group = &mut self.groups[which];
        let old_len = group.world_regions.len();
        assert_eq!(old_len, group.sheet_regions.len());
        // shrink or grow sprite vecs
        group.world_regions.resize(len, Region::zeroed());
        group.sheet_regions.resize(len, Region::zeroed());
        // realloc buffer if needed, remake sprite_bind_group if using storage buffers
        let new_size = len * std::mem::size_of::<Region>();
        if new_size > group.world_buffer.size() as usize {
            group.world_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: new_size as u64,
                usage: if USE_STORAGE {
                    wgpu::BufferUsages::STORAGE
                } else {
                    wgpu::BufferUsages::VERTEX
                } | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            group.sheet_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: new_size as u64,
                usage: if USE_STORAGE {
                    wgpu::BufferUsages::STORAGE
                } else {
                    wgpu::BufferUsages::VERTEX
                } | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            if USE_STORAGE {
                group.sprite_bind_group =
                    gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: None,
                        layout: &self.sprite_bind_group_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: group.camera_buffer.as_entire_binding(),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: group.world_buffer.as_entire_binding(),
                            },
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: group.sheet_buffer.as_entire_binding(),
                            },
                        ],
                    });
            };
            gpu.queue.write_buffer(
                &group.world_buffer,
                0,
                bytemuck::cast_slice(&group.world_regions),
            );
            gpu.queue.write_buffer(
                &group.sheet_buffer,
                0,
                bytemuck::cast_slice(&group.sheet_regions),
            );
        }
        old_len
    }
    /// Set the given camera transform on all sprite groups.  Uploads to the GPU.
    pub fn set_camera_all(&mut self, gpu: &WGPU, camera: GPUCamera) {
        for sg_index in 0..self.groups.len() {
            self.set_camera(gpu, sg_index, camera);
        }
    }
    /// Set the given camera transform on a specific sprite group.  Uploads to the GPU.
    pub fn set_camera(&mut self, gpu: &WGPU, which: usize, camera: GPUCamera) {
        let sg = &mut self.groups[which];
        sg.camera = camera;
        gpu.queue
            .write_buffer(&sg.camera_buffer, 0, bytemuck::bytes_of(&sg.camera));
    }
    /// Send a range of stored sprite data for a particular group to the GPU.
    /// You must call this yourself after modifying sprite data.
    pub fn upload_sprites(&mut self, gpu: &WGPU, which: usize, range: Range<usize>) {
        self.upload_world_regions(gpu, which, range.clone());
        self.upload_sheet_regions(gpu, which, range);
    }
    /// Upload only position changes to the GPU
    pub fn upload_world_regions(&mut self, gpu: &WGPU, which: usize, range: Range<usize>) {
        gpu.queue.write_buffer(
            &self.groups[which].world_buffer,
            range.start as u64,
            bytemuck::cast_slice(&self.groups[which].world_regions[range]),
        );
    }
    /// Upload only animation changes to the GPU
    pub fn upload_sheet_regions(&mut self, gpu: &WGPU, which: usize, range: Range<usize>) {
        gpu.queue.write_buffer(
            &self.groups[which].sheet_buffer,
            range.start as u64,
            bytemuck::cast_slice(&self.groups[which].sheet_regions[range]),
        );
    }
    /// Get a read-only slice of a specified sprite group's world positions and texture regions.
    pub fn get_sprites(&self, which: usize) -> (&[Region], &[Region]) {
        (
            &self.groups[which].world_regions,
            &self.groups[which].sheet_regions,
        )
    }
    /// Get a mutable slice of a specified sprite group's world positions and texture regions.
    pub fn get_sprites_mut(&mut self, which: usize) -> (&mut [Region], &mut [Region]) {
        let group = &mut self.groups[which];
        (&mut group.world_regions, &mut group.sheet_regions)
    }
    /// Render all sprite groups into the given pass.
    pub fn render<'s, 'pass>(&'s self, rpass: &mut wgpu::RenderPass<'pass>)
    where
        's: 'pass,
    {
        rpass.set_pipeline(&self.pipeline);
        for group in self.groups.iter() {
            if !USE_STORAGE {
                rpass.set_vertex_buffer(0, group.world_buffer.slice(..));
                rpass.set_vertex_buffer(0, group.sheet_buffer.slice(..));
            }
            rpass.set_bind_group(0, &group.sprite_bind_group, &[]);
            rpass.set_bind_group(1, &group.tex_bind_group, &[]);
            // draw two triangles per sprite, and sprites-many sprites.
            // this uses instanced drawing, but it would also be okay
            // to draw 6 * sprites.len() vertices and use modular arithmetic
            // to figure out which sprite we're drawing.
            assert_eq!(group.world_regions.len(), group.sheet_regions.len());
            rpass.draw(0..6, 0..group.world_regions.len() as u32);
            //rpass.draw(0..(6 * group.sprites.len() as u32), 0..1);
        }
    }
}
