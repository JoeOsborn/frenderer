//! Similar to sprite groups and individual sprites, in frenderer
//! there are mesh groups and individual meshes.  Each individual mesh
//! has some submeshes that are all grouped together (different
//! submeshes can use different materials); meshes can have some
//! number of instances (you set an estimate for the number of
//! instances of each mesh when you're adding the mesh group; it can
//! grow at runtime but it might be costly so try to minimize the
//! amount of growth), and the setting of instance data and uploading
//! of instance data to the GPU are separated like they are for
//! sprites.  The only instance data is a 3D transform (translation,
//! rotation, and a uniform scaling factor (so it fits neatly into 8
//! floats).  Rotations are defined as quaternions.
//!
//! This module defines two renderers: the textured renderer
//! [`MeshRenderer`] and the flat-colored renderer [`FlatRenderer`].
//! They use slightly different vertex coordinates (e.g., the mesh
//! renderer has UV coordinates).
//!
//! 3D graphics in frenderer use a right-handed, y-up coordinate system.

use bytemuck::Zeroable;
use std::{borrow::Cow, marker::PhantomData, ops::Range};
use wgpu::util::{self as wutil, DeviceExt};

/// A vertex for meshes in the [`MeshRenderer`].
#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, PartialEq, Debug)]
pub struct Vertex {
    position: [f32; 3],
    uv_which: [f32; 3],
}
impl Vertex {
    pub const ZERO: Self = Self {
        position: [0.0; 3],
        uv_which: [0.0; 3],
    };
    /// Creates a vertex with the given position, UV coordinates, and index into the texture array.
    pub fn new(position: [f32; 3], uv: [f32; 2], which: u32) -> Self {
        Self {
            position,
            uv_which: [uv[0], uv[1], f32::from_bits(which)],
        }
    }
}
/// A vertex for meshes in the [`FlatRenderer`].
#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, PartialEq, Debug)]
pub struct FlatVertex {
    position_which: [f32; 4],
}
impl FlatVertex {
    pub const ZERO: Self = Self {
        position_which: [0.0; 4],
    };
    /// Creates a vertex with the given position and index into the color array.
    pub fn new(pos: [f32; 3], which: u32) -> Self {
        Self {
            position_which: [pos[0], pos[1], pos[2], f32::from_bits(which)],
        }
    }
}

struct MeshRendererInner<Vtx: bytemuck::Pod + bytemuck::Zeroable + Copy> {
    groups: Vec<Option<MeshGroupData>>,
    free_groups: Vec<usize>,
    bind_group_layout: wgpu::BindGroupLayout,
    camera_bind_group: wgpu::BindGroup,
    camera_buffer: wgpu::Buffer,
    camera: Camera3D,
    pipeline: wgpu::RenderPipeline,
    _vertex_data: PhantomData<Vtx>,
}

/// Renders groups of 3D meshes with textures and no lighting.
pub struct MeshRenderer {
    data: MeshRendererInner<Vertex>,
}
/// Renders groups of 3D meshes with flat colors and no lighting.
pub struct FlatRenderer {
    data: MeshRendererInner<FlatVertex>,
}
struct MeshGroupData {
    instance_data: Vec<Transform3D>,
    instance_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    meshes: Vec<MeshData>,
}

#[derive(Debug)]
struct MeshData {
    instances: Range<u32>,
    submeshes: Vec<SubmeshData>,
}
/// The range of indices and base vertex for a single submesh.
#[derive(Debug)]
pub struct SubmeshData {
    /// A range of indices within the mesh group's index buffer
    pub indices: Range<u32>,
    /// The base vertex to be added to the value of each index in the
    /// submesh.  Warning: `vertex_base` values greater than 0 are not
    /// supported on some targets (notably web).
    pub vertex_base: i32,
}

/// A transform in 3D space comprised of a translation, a rotation (a quaternion), and a scale.
#[repr(C)]
#[derive(bytemuck::Zeroable, bytemuck::Pod, Clone, Copy, PartialEq, Debug)]
pub struct Transform3D {
    pub translation: [f32; 3],
    pub scale: f32,
    pub rotation: [f32; 4],
}

impl Transform3D {
    pub const ZERO: Self = Self {
        translation: [0.0; 3],
        scale: 0.0,
        rotation: [0.0; 4],
    };
}

/// A 3D perspective camera positioned at some point and rotated in some orientation (a quaternion).
#[repr(C)]
#[derive(bytemuck::Zeroable, bytemuck::Pod, Clone, Copy, PartialEq, Debug)]
pub struct Camera3D {
    pub translation: [f32; 3],
    pub near: f32,
    pub far: f32,
    pub rotation: [f32; 4],
    pub aspect: f32,
    pub fov: f32,
}

impl MeshRenderer {
    /// Creates a new `MeshRenderer` meant to draw into the given color target state with the given depth texture format..
    pub fn new(
        gpu: &crate::WGPU,
        color_target: wgpu::ColorTargetState,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        let bind_group_layout =
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
                            visibility: wgpu::ShaderStages::FRAGMENT,
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
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                // uv_which (we lie and say it's three floats)
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: std::mem::size_of::<f32>() as u64 * 3,
                    shader_location: 1,
                },
            ],
            step_mode: wgpu::VertexStepMode::Vertex,
        };
        let data = MeshRendererInner::new(
            gpu,
            wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("static_meshes.wgsl"))),
            "vs_main",
            "fs_main",
            bind_group_layout,
            vertex_layout,
            color_target,
            depth_format,
        );

        Self { data }
    }
    /// Sets the given camera for all mesh groups.
    pub fn set_camera(&mut self, gpu: &crate::WGPU, camera: Camera3D) {
        self.data.set_camera(gpu, camera)
    }
    /// Add a mesh group with the given array texture.  All meshes in
    /// the group pull from the same vertex buffer, and each submesh
    /// is defined in terms of a range of indices within that buffer.
    /// When loading your mesh resources from whatever format they're
    /// stored in, fill out vertex and index vecs while tracking the
    /// beginning and end of each mesh and submesh (see [`MeshEntry`]
    /// for details).
    pub fn add_mesh_group(
        &mut self,
        gpu: &crate::WGPU,
        texture: &wgpu::Texture,
        vertices: Vec<Vertex>,
        indices: Vec<u32>,
        mesh_info: Vec<MeshEntry>,
    ) -> MeshGroup {
        if gpu.is_gl()
            && (texture.depth_or_array_layers() == 1 || texture.depth_or_array_layers() == 6)
        {
            panic!("Array textures with 1 or 6 layers aren't supported in webgl or other GL backends {:?}", texture);
        }

        let view_mesh = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            base_array_layer: 0,
            array_layer_count: match texture.depth_or_array_layers() {
                0 => Some(1),
                layers => Some(layers),
            },
            ..Default::default()
        });
        let sampler_mesh = gpu
            .device()
            .create_sampler(&wgpu::SamplerDescriptor::default());
        let bind_group = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.data.bind_group_layout,
            entries: &[
                // One for the texture, one for the sampler
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view_mesh),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler_mesh),
                },
            ],
        });

        self.data
            .add_mesh_group(gpu, bind_group, vertices, indices, mesh_info)
    }
    /// Change the number of instances of the given mesh of the given mesh group.
    pub fn resize_group_mesh(
        &mut self,
        gpu: &crate::WGPU,
        which: MeshGroup,
        mesh_idx: usize,
        len: usize,
    ) -> usize {
        self.data.resize_group_mesh(gpu, which, mesh_idx, len)
    }
    /// Returns how many mesh groups there are.
    pub fn mesh_group_count(&self) -> usize {
        self.data.mesh_group_count()
    }
    /// Returns how many meshes there are in the given mesh group.
    pub fn mesh_count(&self, which: MeshGroup) -> usize {
        self.data.mesh_count(which)
    }
    /// Returns how many mesh instances there are in the given mesh of the given mesh group.
    pub fn mesh_instance_count(&self, which: MeshGroup, mesh_number: usize) -> usize {
        self.data.mesh_instance_count(which, mesh_number)
    }
    /// Gets the transforms of every instance of the given mesh of a mesh group.
    pub fn get_meshes(&self, which: MeshGroup, mesh_number: usize) -> &[Transform3D] {
        self.data.get_meshes(which, mesh_number)
    }
    /// Gets the (mutable) transforms of every instance of the given mesh of a mesh group.
    pub fn get_meshes_mut(&mut self, which: MeshGroup, mesh_number: usize) -> &mut [Transform3D] {
        self.data.get_meshes_mut(which, mesh_number)
    }
    /// Deletes a mesh group, leaving its slot free to be reused.
    pub fn remove_mesh_group(&mut self, which: MeshGroup) {
        self.data.remove_mesh_group(which)
    }
    /// Uploads a range of instance data for the given mesh of a given mesh group.
    pub fn upload_meshes(
        &mut self,
        gpu: &crate::WGPU,
        which: MeshGroup,
        mesh_number: usize,
        range: impl std::ops::RangeBounds<usize>,
    ) {
        self.data.upload_meshes(gpu, which, mesh_number, range)
    }
    /// Uploads instance data for all the meshes of a given mesh group.
    pub fn upload_meshes_group(&mut self, gpu: &crate::WGPU, which: MeshGroup) {
        self.data.upload_meshes_group(gpu, which)
    }
    /// Renders the given range of mesh groups into the given [`wgpu::RenderPass`].
    pub fn render<'s, 'pass>(
        &'s self,
        rpass: &mut wgpu::RenderPass<'pass>,
        which: impl std::ops::RangeBounds<usize>,
    ) where
        's: 'pass,
    {
        self.data.render(rpass, which)
    }
}

impl FlatRenderer {
    /// Creates a new `FlatRenderer` meant to draw into the given color target state with the given depth texture format.
    pub fn new(
        gpu: &crate::WGPU,
        color_target: wgpu::ColorTargetState,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        let bind_group_layout =
            gpu.device()
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    // It needs the first entry for the texture and the second for the sampler.
                    // This is like defining a type signature.
                    entries: &[
                        // The material binding
                        wgpu::BindGroupLayoutEntry {
                            // This matches the binding in the shader
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            // It's a buffer binding
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                    ],
                });
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<FlatVertex>() as u64,
            attributes: &[
                // position_which (we lie and say it's four floats)
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 0,
                    shader_location: 0,
                },
            ],
            step_mode: wgpu::VertexStepMode::Vertex,
        };
        let data = MeshRendererInner::new(
            gpu,
            wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("static_meshes.wgsl"))),
            "vs_flat_main",
            "fs_flat_main",
            bind_group_layout,
            vertex_layout,
            color_target,
            depth_format,
        );

        Self { data }
    }
    /// Sets the given camera for all mesh groups.
    pub fn set_camera(&mut self, gpu: &crate::WGPU, camera: Camera3D) {
        self.data.set_camera(gpu, camera)
    }
    /// Add a mesh group with the given array of material colors.  All
    /// meshes in the group pull from the same vertex buffer, and each
    /// submesh is defined in terms of a range of indices within that
    /// buffer.  When loading your mesh resources from whatever format
    /// they're stored in, fill out vertex and index vecs while
    /// tracking the beginning and end of each mesh and submesh (see
    /// [`MeshEntry`] for details).
    pub fn add_mesh_group(
        &mut self,
        gpu: &crate::WGPU,
        // RGBA colors (A currently unused)
        material_colors: &[[f32; 4]],
        vertices: Vec<FlatVertex>,
        indices: Vec<u32>,
        mesh_info: Vec<MeshEntry>,
    ) -> MeshGroup {
        let mat_count = material_colors.len();
        if mat_count > 256 {
            panic!("Can't support >256 materials in one group (got {mat_count})");
        }
        let uniforms = gpu.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("flat mesh group"),
            size: 4096,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });
        gpu.queue()
            .write_buffer(&uniforms, 0, bytemuck::cast_slice(material_colors));
        let bind_group = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.data.bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniforms,
                    offset: 0,
                    size: Some(uniforms.size().try_into().unwrap()),
                }),
            }],
        });

        self.data
            .add_mesh_group(gpu, bind_group, vertices, indices, mesh_info)
    }
    /// Change the number of instances of the given mesh of the given mesh group.
    pub fn resize_group_mesh(
        &mut self,
        gpu: &crate::WGPU,
        which: MeshGroup,
        mesh_idx: usize,
        len: usize,
    ) -> usize {
        self.data.resize_group_mesh(gpu, which, mesh_idx, len)
    }
    /// Returns how many mesh groups there are.
    pub fn mesh_group_count(&self) -> usize {
        self.data.mesh_group_count()
    }
    /// Returns how many meshes there are in the given mesh group.
    pub fn mesh_count(&self, which: MeshGroup) -> usize {
        self.data.mesh_count(which)
    }
    /// Returns how many mesh instances there are in the given mesh of the given mesh group.
    pub fn mesh_instance_count(&self, which: MeshGroup, mesh_number: usize) -> usize {
        self.data.mesh_instance_count(which, mesh_number)
    }
    /// Gets the transforms of every instance of the given mesh of a mesh group.
    pub fn get_meshes(&self, which: MeshGroup, mesh_number: usize) -> &[Transform3D] {
        self.data.get_meshes(which, mesh_number)
    }
    /// Gets the (mutable) transforms of every instance of the given mesh of a mesh group.
    pub fn get_meshes_mut(&mut self, which: MeshGroup, mesh_number: usize) -> &mut [Transform3D] {
        self.data.get_meshes_mut(which, mesh_number)
    }
    /// Deletes a mesh group, leaving its slot free to be reused.
    pub fn remove_mesh_group(&mut self, which: MeshGroup) {
        self.data.remove_mesh_group(which)
    }
    /// Uploads a range of instance data for the given mesh of a given mesh group.
    pub fn upload_meshes(
        &mut self,
        gpu: &crate::WGPU,
        which: MeshGroup,
        mesh_number: usize,
        range: impl std::ops::RangeBounds<usize>,
    ) {
        self.data.upload_meshes(gpu, which, mesh_number, range)
    }
    /// Uploads instance data for all the meshes of a given mesh group.
    pub fn upload_meshes_group(&mut self, gpu: &crate::WGPU, which: MeshGroup) {
        self.data.upload_meshes_group(gpu, which)
    }
    /// Renders the given range of mesh groups into the given [`wgpu::RenderPass`].
    pub fn render<'s, 'pass>(
        &'s self,
        rpass: &mut wgpu::RenderPass<'pass>,
        which: impl std::ops::RangeBounds<usize>,
    ) where
        's: 'pass,
    {
        self.data.render(rpass, which)
    }
}

impl<Vtx: bytemuck::Pod + bytemuck::Zeroable + Copy> MeshRendererInner<Vtx> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        gpu: &crate::WGPU,
        shader: wgpu::ShaderSource,
        vs_entry: &str,
        fs_entry: &str,
        bind_group_layout: wgpu::BindGroupLayout,
        vertex_layout: wgpu::VertexBufferLayout,
        color_target: wgpu::ColorTargetState,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = gpu
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: shader,
            });
        let camera_buffer = gpu.device().create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: std::mem::size_of::<[f32; 16]>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group_layout =
            gpu.device()
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[wgpu::BindGroupLayoutEntry {
                        // This matches the binding in the shader
                        binding: 0,
                        // Available in vertex shader
                        visibility: wgpu::ShaderStages::VERTEX,
                        // It's a uniform buffer
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        // No count, not a buffer array binding
                        count: None,
                    }],
                });
        let camera_bind_group = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });
        let pipeline_layout =
            gpu.device()
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &[&camera_bind_group_layout, &bind_group_layout],
                    push_constant_ranges: &[],
                });
        let pipeline = gpu
            .device()
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: vs_entry,
                    buffers: &[
                        vertex_layout,
                        wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<Transform3D>() as u64,
                            attributes: &[
                                // trans_scale
                                wgpu::VertexAttribute {
                                    format: wgpu::VertexFormat::Float32x4,
                                    offset: 0,
                                    shader_location: 2,
                                },
                                // rot
                                wgpu::VertexAttribute {
                                    format: wgpu::VertexFormat::Float32x4,
                                    offset: std::mem::size_of::<f32>() as u64 * 4,
                                    shader_location: 3,
                                },
                            ],
                            step_mode: wgpu::VertexStepMode::Instance,
                        },
                    ],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: fs_entry,
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
        let mut ret = Self {
            groups: vec![],
            free_groups: vec![],
            bind_group_layout,
            camera_bind_group,
            camera_buffer,
            pipeline,
            _vertex_data: PhantomData,
            camera: Camera3D {
                translation: [0.0; 3],
                near: 0.1,
                far: 100.0,
                rotation: ultraviolet::Rotor3::identity().into_quaternion_array(),
                aspect: 4.0 / 3.0,
                fov: std::f32::consts::FRAC_PI_2,
            },
        };
        ret.set_camera(gpu, ret.camera);
        ret
    }

    fn set_camera(&mut self, gpu: &crate::WGPU, camera: Camera3D) {
        self.camera = camera;
        let tr = ultraviolet::Vec3::from(camera.translation);
        let view = (ultraviolet::Mat4::from_translation(tr)
            * ultraviolet::Rotor3::from_quaternion_array(camera.rotation)
                .into_matrix()
                .into_homogeneous())
        .inversed();
        let proj = ultraviolet::projection::rh_yup::perspective_wgpu_dx(
            camera.fov,
            camera.aspect,
            camera.near,
            camera.far,
        );
        let mat = proj * view;
        gpu.queue()
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&mat));
    }
    fn add_mesh_group(
        &mut self,
        gpu: &crate::WGPU,
        bind_group: wgpu::BindGroup,
        vertices: Vec<Vtx>,
        indices: Vec<u32>,
        mesh_info: Vec<MeshEntry>,
    ) -> MeshGroup {
        let group_idx = if let Some(idx) = self.free_groups.pop() {
            idx
        } else {
            self.groups.push(None);
            self.groups.len() - 1
        };
        let vertex_buffer = gpu
            .device()
            .create_buffer_init(&wutil::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
        let index_buffer = gpu
            .device()
            .create_buffer_init(&wutil::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            });
        let instance_count: u32 = mesh_info.iter().map(|me| me.instance_count).sum();
        let instance_data = vec![Transform3D::zeroed(); instance_count as usize];
        let instance_buffer = gpu.device().create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: instance_count as u64 * std::mem::size_of::<Transform3D>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut next_instance = 0_u32;
        let meshes: Vec<_> = mesh_info
            .into_iter()
            .map(|me| {
                let instance = next_instance;
                next_instance += me.instance_count;
                if (gpu.is_gl() || gpu.is_web())
                    && me.submeshes.iter().any(|sm| sm.vertex_base != 0)
                {
                    panic!(
                        "Meshes with non-zero vertex base are not supported in GL or web backends"
                    );
                }
                MeshData {
                    instances: instance..next_instance,
                    submeshes: me.submeshes,
                }
            })
            .collect();
        let group = MeshGroupData {
            instance_data,
            instance_buffer,
            vertex_buffer,
            index_buffer,
            bind_group,
            meshes,
        };
        self.groups[group_idx] = Some(group);
        MeshGroup(group_idx)
    }
    fn resize_group_mesh(
        &mut self,
        gpu: &crate::WGPU,
        which: MeshGroup,
        mesh_idx: usize,
        len: usize,
    ) -> usize {
        let group = self.groups[which.0].as_mut().unwrap();
        let mesh_count = group.meshes.len();
        let mesh = &group.meshes[mesh_idx];
        let new_end = mesh.instances.start + len as u32;
        let old_len = mesh.instances.end as usize - mesh.instances.start as usize;
        let next_mesh = if mesh_idx + 1 < mesh_count {
            Some(mesh_idx + 1)
        } else {
            None
        };
        let old_group_len = group.instance_data.len();
        if old_len == len {
            return old_len;
        } else if len < old_len
            || match next_mesh {
                Some(nm) => new_end < group.meshes[nm].instances.start,
                None => old_group_len > new_end as usize,
            }
        // if there is a next mesh and we fit before it, or this is the last mesh and we still have room in the vec...
        {
            // just increase (or decrease if we're shrinking) the instance data range
            group.meshes[mesh_idx].instances.end = new_end;
        } else
        /* len > old_len, space not free; extend instance data and move stuff over */
        {
            // we may have to realloc.
            // make room in instance data
            let new_group_len = group.instance_data.len() + (len - old_len);
            group
                .instance_data
                .resize(new_group_len, Transform3D::zeroed());
            // move over everything after this mesh
            if let Some(next) = next_mesh {
                let next = &group.meshes[next];
                // new_end is definitely after next_mesh's start.
                group.instance_data.copy_within(
                    next.instances.start as usize..old_group_len,
                    new_end as usize,
                );
                // update start and end indices for later meshes by diff, the amount that the group got pushed by.
                let diff = new_end - next.instances.start;
                for mesh_j in group.meshes[(mesh_idx + 1)..].iter_mut() {
                    mesh_j.instances.start += diff;
                    mesh_j.instances.end += diff;
                    assert!(mesh_j.instances.end <= new_group_len as u32);
                }
            }
            // extend end of mesh.instances
            group.meshes[mesh_idx].instances.end = new_end;
            // grow instance buffer if needed
            let new_len_bytes = std::mem::size_of::<Transform3D>() * new_group_len;
            if new_len_bytes > group.instance_buffer.size() as usize {
                group.instance_buffer = gpu.device().create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: new_len_bytes as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                // write immediately since otherwise it will contain garbage
                gpu.queue().write_buffer(
                    &group.instance_buffer,
                    0,
                    bytemuck::cast_slice(&group.instance_data),
                );
            }
        }
        old_len
    }

    fn mesh_group_count(&self) -> usize {
        self.groups.len()
    }
    fn mesh_count(&self, which: MeshGroup) -> usize {
        self.groups[which.0].as_ref().unwrap().meshes.len()
    }
    fn mesh_instance_count(&self, which: MeshGroup, mesh_number: usize) -> usize {
        let range = &self.groups[which.0].as_ref().unwrap().meshes[mesh_number].instances;
        range.end as usize - range.start as usize
    }
    fn get_meshes(&self, which: MeshGroup, mesh_number: usize) -> &[Transform3D] {
        let group = &self.groups[which.0].as_ref().unwrap();
        let mesh = &group.meshes[mesh_number];
        let range = mesh.instances.clone();
        &group.instance_data[range.start as usize..range.end as usize]
    }
    fn get_meshes_mut(&mut self, which: MeshGroup, mesh_number: usize) -> &mut [Transform3D] {
        let group = self.groups[which.0].as_mut().unwrap();
        let mesh = &mut group.meshes[mesh_number];
        let range = mesh.instances.clone();
        &mut group.instance_data[range.start as usize..range.end as usize]
    }
    /// Deletes a mesh group, leaving an empty placeholder.
    fn remove_mesh_group(&mut self, which: MeshGroup) {
        if self.groups[which.0].is_some() {
            self.groups[which.0] = None;
            self.free_groups.push(which.0);
        }
    }
    fn upload_meshes(
        &mut self,
        gpu: &crate::WGPU,
        which: MeshGroup,
        mesh_number: usize,
        range: impl std::ops::RangeBounds<usize>,
    ) {
        let group = &self.groups[which.0].as_ref().unwrap();
        let mesh = &group.meshes[mesh_number];
        let range = crate::range(
            range,
            mesh.instances.end as usize - mesh.instances.start as usize,
        );
        // offset range by instance_start
        gpu.queue().write_buffer(
            &group.instance_buffer,
            ((mesh.instances.start as usize + range.start as usize)
                * std::mem::size_of::<Transform3D>()) as u64,
            bytemuck::cast_slice(
                &group.instance_data[(mesh.instances.start as usize + range.start)
                    ..(mesh.instances.start as usize + range.end)],
            ),
        );
    }
    fn upload_meshes_group(&mut self, gpu: &crate::WGPU, which: MeshGroup) {
        // upload the whole instance buffer
        let group = &self.groups[which.0].as_ref().unwrap();
        gpu.queue().write_buffer(
            &group.instance_buffer,
            0,
            bytemuck::cast_slice(&group.instance_data),
        );
    }
    fn render<'s, 'pass>(
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
        // camera
        rpass.set_bind_group(0, &self.camera_bind_group, &[]);
        for group in self.groups[which].iter().filter_map(|o| o.as_ref()) {
            rpass.set_bind_group(1, &group.bind_group, &[]);
            rpass.set_vertex_buffer(0, group.vertex_buffer.slice(..));
            rpass.set_vertex_buffer(1, group.instance_buffer.slice(..));
            rpass.set_index_buffer(group.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            for mesh in group.meshes.iter() {
                for submesh in mesh.submeshes.iter() {
                    rpass.draw_indexed(
                        submesh.indices.clone(),
                        submesh.vertex_base,
                        mesh.instances.clone(),
                    );
                }
            }
        }
    }
}

/// An opaque identifier for a mesh group.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct MeshGroup(usize);
impl MeshGroup {
    pub fn index(&self) -> usize {
        self.0
    }
}
impl From<usize> for MeshGroup {
    fn from(value: usize) -> Self {
        Self(value)
    }
}
/// An entry in a mesh group, i.e. a 3D model.
#[derive(Debug)]
pub struct MeshEntry {
    /// How many instances of this model should be allocated
    pub instance_count: u32,
    /// The submeshes making up this model.
    pub submeshes: Vec<SubmeshEntry>,
}
pub type SubmeshEntry = SubmeshData;
