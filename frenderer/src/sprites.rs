//! A sprite renderer with multiple layers ("sprite groups") which can
//! be independently translated; each layer can have several
//! spritesheets and numerous sprites.  For efficiency, it's best to
//! minimize the number of groups.

use std::{borrow::Cow, ops::Range};

use crate::WGPU;
use bytemuck::{Pod, Zeroable};

/// A SheetRegion defines the visual appearance of a sprite: which spritesheet (of an array of spritesheets), its pixel region within the spritesheet, and its visual depth (larger meaning further away).
#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod, Debug, Default)]
pub struct SheetRegion {
    /// Which array texture layer to use
    pub sheet: u16,
    /// How deep into the Z axis this sprite should be drawn; the range `0..u16::MAX` will be mapped onto `0.0..1.0`.
    pub depth: u16,
    /// The x coordinate in pixels of the top left corner of this sprite within the spritesheet texture.
    pub x: u16,
    /// The y coordinate in pixels of the top left corner of this sprite within the spritesheet texture.
    pub y: u16,
    /// The width in pixels of this sprite within the spritesheet texture. May be negative.
    pub w: i16,
    /// The height in pixels of this sprite within the spritesheet texture.  May be negative.
    pub h: i16,
    /// RGBA bytes where alpha indicates intensity of color modulation
    pub colormod: [u8; 4],
}

impl SheetRegion {
    pub const ZERO: Self = Self {
        sheet: 0,
        depth: 0,
        x: 0,
        y: 0,
        w: 0,
        h: 0,
        colormod: [0; 4],
    };
    /// Create a new [`SheetRegion`] with the given parameters.
    pub const fn new(sheet: u16, x: u16, y: u16, depth: u16, w: i16, h: i16) -> Self {
        Self {
            sheet,
            x,
            y,
            w,
            h,
            depth,
            colormod: [0; 4],
        }
    }
    /// Create a simple [`SheetRegion`] with just the rectangle coordinates ([`SheetRegion::sheet`] and [`SheetRegion::depth`] will be set to 0).
    pub const fn rect(x: u16, y: u16, w: i16, h: i16) -> Self {
        Self::new(0, x, y, 0, w, h)
    }
    /// Produce a new [`SheetRegion`] on a different spritesheet layer.
    pub const fn with_sheet(self, which: u16) -> Self {
        Self {
            sheet: which,
            ..self
        }
    }
    /// Produce a new [`SheetRegion`] drawn at a different depth level.
    pub const fn with_depth(self, depth: u16) -> Self {
        Self { depth, ..self }
    }
    /// Flips the texture horizontally by moving its position and inverting its width
    pub const fn flip_horizontal(self) -> Self {
        Self {
            x: (self.x as i32 + self.w as i32) as u16,
            w: -self.w,
            ..self
        }
    }
    /// Flips the texture vertically by moving its position and inverting its height
    pub const fn flip_vertical(self) -> Self {
        Self {
            y: (self.y as i32 + self.h as i32) as u16,
            h: -self.h,
            ..self
        }
    }
    /// Changes the color modulation of the sheet region
    pub const fn with_colormod(self, colormod: [u8; 4]) -> Self {
        Self { colormod, ..self }
    }
}

/// A Transform describes a location, an extent, and a rotation in 2D
/// space.  Width and height are crammed into 4 bytes meaning the
/// maximum width and height are [`u16::MAX`] and fractional widths
/// and heights are not supported.  The location `(x,y)` is typically
/// interpreted as the center of the object after translation.
/// Rotations are in radians, counterclockwise about the center point.
#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod, Debug)]
pub struct Transform {
    /// The horizontal scale of the transform
    pub w: u16,
    /// The vertical scale of the transform
    pub h: u16,
    /// The x coordinate of the translation
    pub x: f32,
    /// The y coordinate of the translation
    pub y: f32,
    /// A rotation in radians counterclockwise about the center
    pub rot: f32,
}

impl Transform {
    pub const ZERO: Self = Self {
        w: 0,
        h: 0,
        x: 0.0,
        y: 0.0,
        rot: 0.0,
    };
    /// Returns the Transform's translation as a pair of `f32`.
    pub fn translation(&self) -> [f32; 2] {
        [self.x, self.y]
    }
    /// Returns the Transform's scale as a pair of `f32`.
    pub fn scale(&self) -> [f32; 2] {
        [self.w as f32, self.h as f32]
    }
}

/// Camera2D is a transform for a sprite layer, defining a scale
/// followed by a translation.
#[repr(C)]
#[derive(Clone, Copy, Zeroable, Pod, Debug)]
pub struct Camera2D {
    /// The position of the camera in world space
    pub screen_pos: [f32; 2],
    /// The size of the camera viewport in world space pixels
    pub screen_size: [f32; 2],
}

struct SpriteGroup {
    world_buffer: wgpu::Buffer,
    sheet_buffer: wgpu::Buffer,
    world_transforms: Vec<Transform>,
    sheet_regions: Vec<SheetRegion>,
    camera: Camera2D,
    camera_buffer: wgpu::Buffer,
    tex_bind_group: wgpu::BindGroup,
    sprite_bind_group: wgpu::BindGroup,
}

/// SpriteRenderer hosts a number of sprite groups.  Each group has a
/// specified spritesheet texture array, parallel vectors of
/// [`Transform`]s and [`SheetRegion`]s, and a [`Camera2D`] to define
/// its transform.  All groups render into the same depth
/// buffer, so their outputs are interleaved.
pub struct SpriteRenderer {
    pipeline: wgpu::RenderPipeline,
    sprite_bind_group_layout: wgpu::BindGroupLayout,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    groups: Vec<Option<SpriteGroup>>,
    free_groups: Vec<usize>,
    use_storage: bool,
}

impl SpriteRenderer {
    /// Create a new [`SpriteRenderer`] meant to draw into the given color target and with the given depth texture format.
    pub fn new(
        gpu: &WGPU,
        color_target: wgpu::ColorTargetState,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = gpu
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("sprites.wgsl"))),
            });

        let texture_bind_group_layout =
            gpu.device()
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    // It needs the first entry for the texture and the second for the sampler.
                    // This is like defining a type signature.
                    entries: &[
                        // The texture binding
                        wgpu::BindGroupLayoutEntry {
                            // This matches the binding in the shader
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                            // It's a texture binding
                            ty: wgpu::BindingType::Texture {
                                // We can use it with float samplers
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                // It's being used as a 2D texture
                                view_dimension: wgpu::TextureViewDimension::D2Array,
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
        let use_storage = gpu.supports_storage();
        let sprite_bind_group_layout = if use_storage {
            gpu.device()
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
            gpu.device()
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[camera_layout_entry],
                })
        };
        let pipeline_layout =
            gpu.device()
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &[&sprite_bind_group_layout, &texture_bind_group_layout],
                    push_constant_ranges: &[],
                });

        assert_eq!(std::mem::size_of::<Transform>(), 4 * 4);
        assert_eq!(std::mem::size_of::<SheetRegion>(), 4 * 4);
        let pipeline = gpu
            .device()
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: if use_storage {
                        "vs_storage_main"
                    } else {
                        "vs_vbuf_main"
                    },
                    buffers: if use_storage {
                        &[]
                    } else {
                        &[
                            wgpu::VertexBufferLayout {
                                array_stride: std::mem::size_of::<Transform>() as u64,
                                step_mode: wgpu::VertexStepMode::Instance,
                                attributes: &[wgpu::VertexAttribute {
                                    // This is a fun little trick, we
                                    // lie and say it's four floats.
                                    // In the shader the first float
                                    // is cast bitwise to a u32 and
                                    // then the w and h are masked out
                                    // and casted back to f32.
                                    format: wgpu::VertexFormat::Float32x4,
                                    offset: 0,
                                    shader_location: 0,
                                }],
                            },
                            wgpu::VertexBufferLayout {
                                array_stride: std::mem::size_of::<SheetRegion>() as u64,
                                step_mode: wgpu::VertexStepMode::Instance,
                                attributes: &[wgpu::VertexAttribute {
                                    format: wgpu::VertexFormat::Uint32x4,
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
                    targets: &[Some(color_target)],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: depth_format,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        Self {
            pipeline,
            use_storage,
            free_groups: Vec::new(),
            groups: Vec::with_capacity(4),
            sprite_bind_group_layout,
            texture_bind_group_layout,
        }
    }
    /// Create a new sprite group sized to fit `world_transforms` and
    /// `sheet_regions`, which should be the same length.  Returns the
    /// sprite group index corresponding to this group.
    pub fn add_sprite_group(
        &mut self,
        gpu: &WGPU,
        tex: &wgpu::Texture,
        world_transforms: Vec<Transform>,
        sheet_regions: Vec<SheetRegion>,
        camera: Camera2D,
    ) -> usize {
        if gpu.is_gl() && (tex.depth_or_array_layers() == 1 || tex.depth_or_array_layers() == 6) {
            panic!("Array textures with 1 or 6 layers aren't supported in webgl or other GL backends {:?}", tex);
        }
        let group_idx = if let Some(idx) = self.free_groups.pop() {
            idx
        } else {
            self.groups.push(None);
            self.groups.len() - 1
        };
        let view_sprite = tex.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            base_array_layer: 0,
            array_layer_count: match tex.depth_or_array_layers() {
                0 => Some(1),
                layers => Some(layers),
            },
            ..Default::default()
        });
        let sampler_sprite = gpu
            .device()
            .create_sampler(&wgpu::SamplerDescriptor::default());
        let tex_bind_group = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
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
        let buffer_world = gpu.device().create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: world_transforms.len() as u64 * std::mem::size_of::<Transform>() as u64,
            usage: if self.use_storage {
                wgpu::BufferUsages::STORAGE
            } else {
                wgpu::BufferUsages::VERTEX
            } | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let buffer_sheet = gpu.device().create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: sheet_regions.len() as u64 * std::mem::size_of::<SheetRegion>() as u64,
            usage: if self.use_storage {
                wgpu::BufferUsages::STORAGE
            } else {
                wgpu::BufferUsages::VERTEX
            } | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_buffer = gpu.device().create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: std::mem::size_of::<Camera2D>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sprite_bind_group = if self.use_storage {
            gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
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
            gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.sprite_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                }],
            })
        };
        gpu.queue()
            .write_buffer(&buffer_world, 0, bytemuck::cast_slice(&world_transforms));
        gpu.queue()
            .write_buffer(&buffer_sheet, 0, bytemuck::cast_slice(&sheet_regions));
        gpu.queue()
            .write_buffer(&camera_buffer, 0, bytemuck::bytes_of(&camera));
        self.groups[group_idx] = Some(SpriteGroup {
            world_buffer: buffer_world,
            sheet_buffer: buffer_sheet,
            world_transforms,
            sheet_regions,
            tex_bind_group,
            sprite_bind_group,
            camera,
            camera_buffer,
        });
        group_idx
    }
    /// Returns the number of sprite groups (including placeholders for removed groups).
    pub fn sprite_group_count(&self) -> usize {
        self.groups.len()
    }
    /// Deletes a sprite group, leaving an empty group slot behind (this might get recycled later).
    pub fn remove_sprite_group(&mut self, which: usize) {
        if self.groups[which].is_some() {
            self.groups[which] = None;
            self.free_groups.push(which);
        }
    }
    /// Reports the size of the given sprite group.  Panics if the given sprite group is not populated.
    pub fn sprite_group_size(&self, which: usize) -> usize {
        self.groups[which].as_ref().unwrap().world_transforms.len()
    }
    /// Resizes a sprite group.  If the new size is smaller, this is
    /// very cheap; if it's larger than it's ever been before, it
    /// might involve reallocating the [`Vec<Transform>`],
    /// [`Vec<SheetRegion>`], or the GPU buffer used to draw sprites,
    /// so it could be expensive.  If this happens, the buffer will
    /// also be uploaded to prevent garbage data from being used in
    /// the shader.  To avoid redundant work, resize upwards as few
    /// times as possible.
    ///
    /// Panics if the given sprite group is not populated.
    pub fn resize_sprite_group(&mut self, gpu: &WGPU, which: usize, len: usize) -> usize {
        let group = &mut self.groups[which].as_mut().unwrap();
        let old_len = group.world_transforms.len();
        if old_len == len {
            return old_len;
        }
        assert_eq!(old_len, group.sheet_regions.len());
        // shrink or grow sprite vecs
        group.world_transforms.resize(len, Transform::zeroed());
        group.sheet_regions.resize(len, SheetRegion::zeroed());
        // realloc buffer if needed, remake sprite_bind_group if using storage buffers
        let new_size = len * std::mem::size_of::<Transform>();
        if new_size > group.world_buffer.size() as usize {
            group.world_buffer = gpu.device().create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: new_size as u64,
                usage: if self.use_storage {
                    wgpu::BufferUsages::STORAGE
                } else {
                    wgpu::BufferUsages::VERTEX
                } | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            group.sheet_buffer = gpu.device().create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: new_size as u64,
                usage: if self.use_storage {
                    wgpu::BufferUsages::STORAGE
                } else {
                    wgpu::BufferUsages::VERTEX
                } | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            if self.use_storage {
                group.sprite_bind_group =
                    gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
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
            gpu.queue().write_buffer(
                &group.world_buffer,
                0,
                bytemuck::cast_slice(&group.world_transforms),
            );
            gpu.queue().write_buffer(
                &group.sheet_buffer,
                0,
                bytemuck::cast_slice(&group.sheet_regions),
            );
        }
        old_len
    }
    /// Set the given camera transform on all sprite groups.  Uploads to the GPU.
    pub fn set_camera_all(&mut self, gpu: &WGPU, camera: Camera2D) {
        for sg_index in 0..self.groups.len() {
            if self.groups[sg_index].is_some() {
                self.set_camera(gpu, sg_index, camera);
            }
        }
    }
    /// Set the given camera transform on a specific sprite group.  Uploads to the GPU.
    /// Panics if the given sprite group is not populated.
    pub fn set_camera(&mut self, gpu: &WGPU, which: usize, camera: Camera2D) {
        let sg = &mut self.groups[which].as_mut().unwrap();
        sg.camera = camera;
        gpu.queue()
            .write_buffer(&sg.camera_buffer, 0, bytemuck::bytes_of(&sg.camera));
    }
    /// Send a range of stored sprite data for a particular group to the GPU.
    /// You must call this yourself after modifying sprite data.
    /// Panics if the given sprite group is not populated.
    pub fn upload_sprites(&mut self, gpu: &WGPU, which: usize, range: Range<usize>) {
        let range = crate::range(range, self.sprite_group_size(which));
        self.upload_world_transforms(gpu, which, range.clone());
        self.upload_sheet_regions(gpu, which, range);
    }
    /// Upload only position changes to the GPU.
    /// Panics if the given sprite group is not populated.
    pub fn upload_world_transforms(&mut self, gpu: &WGPU, which: usize, range: Range<usize>) {
        let group = self.groups[which].as_ref().unwrap();
        gpu.queue().write_buffer(
            &group.world_buffer,
            (range.start * std::mem::size_of::<Transform>()) as u64,
            bytemuck::cast_slice(&group.world_transforms[range]),
        );
    }
    /// Upload only visual changes to the GPU.
    /// Panics if the given sprite group is not populated.
    pub fn upload_sheet_regions(&mut self, gpu: &WGPU, which: usize, range: Range<usize>) {
        let group = self.groups[which].as_ref().unwrap();
        gpu.queue().write_buffer(
            &group.sheet_buffer,
            (range.start * std::mem::size_of::<SheetRegion>()) as u64,
            bytemuck::cast_slice(&group.sheet_regions[range]),
        );
    }
    /// Get a read-only slice of a specified sprite group's world transforms and texture regions.
    /// Panics if the given sprite group is not populated.
    pub fn get_sprites(&self, which: usize) -> (&[Transform], &[SheetRegion]) {
        let group = self.groups[which].as_ref().unwrap();
        (&group.world_transforms, &group.sheet_regions)
    }
    /// Get a mutable slice of a specified sprite group's world transforms and texture regions.
    /// Panics if the given sprite group is not populated.
    pub fn get_sprites_mut(&mut self, which: usize) -> (&mut [Transform], &mut [SheetRegion]) {
        let group = self.groups[which].as_mut().unwrap();
        (&mut group.world_transforms, &mut group.sheet_regions)
    }
    /// Render the given range of sprite groups into the given pass.
    pub fn render<'s, 'pass>(
        &'s self,
        rpass: &mut wgpu::RenderPass<'pass>,
        which: impl std::ops::RangeBounds<usize>,
    ) where
        's: 'pass,
    {
        if self.groups.is_empty() {
            return;
        }
        rpass.set_pipeline(&self.pipeline);
        let which = crate::range(which, self.groups.len());
        for group in self.groups[which].iter().filter_map(|o| o.as_ref()) {
            if group.world_transforms.is_empty() {
                continue;
            }
            if !self.use_storage {
                rpass.set_vertex_buffer(0, group.world_buffer.slice(..));
                rpass.set_vertex_buffer(1, group.sheet_buffer.slice(..));
            }
            rpass.set_bind_group(0, &group.sprite_bind_group, &[]);
            rpass.set_bind_group(1, &group.tex_bind_group, &[]);
            // draw two triangles per sprite, and sprites-many sprites.
            // this uses instanced drawing, but it would also be okay
            // to draw 6 * sprites.len() vertices and use modular arithmetic
            // to figure out which sprite we're drawing.
            assert_eq!(group.world_transforms.len(), group.sheet_regions.len());
            rpass.draw(0..6, 0..group.world_transforms.len() as u32);
        }
    }
}
