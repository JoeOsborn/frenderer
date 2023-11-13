//! Similar to sprite groups and individual sprites, in frenderer
//! there are mesh groups and individual meshes.  Each individual mesh
//! has some submeshes that are all grouped together (this is kind of
//! a quirk of the glTF format but it means different submeshes can
//! use different materials in principle); meshes can have some number
//! of instances (you set an estimate for the number of instances of
//! each mesh when you're adding the mesh group; it can grow at
//! runtime but it might be costly so try to minimize the amount of
//! growth), and the setting of instance data and uploading of
//! instance data to the GPU are separated like they are for sprites.
//! The only instance data is a 3D transform (translation, rotation,
//! and a uniform scaling factor (so it fits neatly into 8 floats).
//! Rotations are defined as quaternions.
//!
//! Mesh groups share a single array texture.  The vertex format is 3
//! xyz coordinates, 2 uv coordinates, and an integer indicating which
//! texture to use.  The shader is a *flat* shader that doesn't do any
//! lighting or other fancy stuff (turning the quaternion into a
//! rotation matrix involved some math I had to look up from a
//! reference).
//!
//! 3D graphics in frenderer use a right-handed, y-up coordinate system (to match glTF).

use bytemuck::Zeroable;
use std::{borrow::Cow, ops::Range};
use wgpu::util::{self as wutil, DeviceExt};
#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, PartialEq, Debug)]
pub struct Vertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub which: u32,
}

pub struct MeshRenderer {
    groups: Vec<MeshGroupData>,
    tex_bind_group_layout: wgpu::BindGroupLayout,
    camera_bind_group: wgpu::BindGroup,
    camera_buffer: wgpu::Buffer,
    camera: Camera3D,
    pipeline: wgpu::RenderPipeline,
}
struct MeshGroupData {
    instance_data: Vec<Transform3D>,
    instance_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    tex_bind_group: wgpu::BindGroup,
    meshes: Vec<MeshData>,
}

struct MeshData {
    instances: Range<u32>,
    submeshes: Vec<SubmeshData>,
}
pub struct SubmeshData {
    pub indices: Range<u32>,
    pub vertex_base: i32,
}

#[repr(C)]
#[derive(bytemuck::Zeroable, bytemuck::Pod, Clone, Copy, PartialEq, Debug)]
pub struct Transform3D {
    pub translation: [f32; 3],
    pub scale: f32,
    pub rotation: [f32; 4],
}

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
    pub(crate) fn new(gpu: &crate::WGPU) -> Self {
        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("static_meshes.wgsl"))),
            });
        let camera_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: std::mem::size_of::<[f32; 16]>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group_layout =
            gpu.device
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
        let tex_bind_group_layout =
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
        let camera_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });
        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&camera_bind_group_layout, &tex_bind_group_layout],
                push_constant_ranges: &[],
            });
        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[
                        wgpu::VertexBufferLayout {
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
                        },
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
                    entry_point: "fs_main",
                    targets: &[Some(gpu.config.format.into())],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
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
            tex_bind_group_layout,
            camera_bind_group,
            camera_buffer,
            pipeline,
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

    pub fn set_camera(&mut self, gpu: &crate::WGPU, camera: Camera3D) {
        self.camera = camera;
        let tr = ultraviolet::Vec3::from(camera.translation);
        let view = ultraviolet::Mat4::from_translation(tr)
            * ultraviolet::Rotor3::from_quaternion_array(camera.rotation)
                .into_matrix()
                .into_homogeneous();
        let proj = ultraviolet::projection::rh_yup::perspective_wgpu_dx(
            camera.fov,
            camera.aspect,
            camera.near,
            camera.far,
        );
        let mat = proj * view;
        gpu.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&mat));
    }
    pub fn add_mesh_group(
        &mut self,
        gpu: &crate::WGPU,
        texture: &wgpu::Texture,
        vertices: Vec<Vertex>,
        indices: Vec<u32>,
        mesh_info: Vec<MeshEntry>,
    ) -> MeshGroup {
        let vertex_buffer = gpu.device.create_buffer_init(&wutil::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let index_buffer = gpu.device.create_buffer_init(&wutil::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        });
        let instance_count: u32 = mesh_info.iter().map(|me| me.instance_count).sum();
        let instance_data = vec![Transform3D::zeroed(); instance_count as usize];
        let instance_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
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
                MeshData {
                    instances: instance..next_instance,
                    submeshes: me.submeshes,
                }
            })
            .collect();
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
            .device
            .create_sampler(&wgpu::SamplerDescriptor::default());
        let tex_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.tex_bind_group_layout,
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
        let group = MeshGroupData {
            instance_data,
            instance_buffer,
            vertex_buffer,
            index_buffer,
            tex_bind_group,
            meshes,
        };
        self.groups.push(group);
        MeshGroup(self.groups.len() - 1)
    }
    pub fn resize_group_mesh(
        &mut self,
        gpu: &crate::WGPU,
        which: MeshGroup,
        mesh_idx: usize,
        len: usize,
    ) -> usize {
        let group = &mut self.groups[which.0];
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
                group.instance_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: new_len_bytes as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                // write immediately since otherwise it will contain garbage
                gpu.queue.write_buffer(
                    &group.instance_buffer,
                    0,
                    bytemuck::cast_slice(&group.instance_data),
                );
            }
        }
        old_len
    }

    pub fn mesh_group_count(&mut self) -> usize {
        self.groups.len()
    }
    pub fn mesh_count(&mut self, which: MeshGroup) -> usize {
        self.groups[which.0].meshes.len()
    }
    pub fn mesh_instance_count(&mut self, which: MeshGroup, mesh_number: usize) -> usize {
        let range = &self.groups[which.0].meshes[mesh_number].instances;
        range.end as usize - range.start as usize
    }
    pub fn get_meshes(&mut self, which: MeshGroup, mesh_number: usize) -> &[Transform3D] {
        let group = &self.groups[which.0];
        let mesh = &group.meshes[mesh_number];
        let range = mesh.instances.clone();
        &group.instance_data[range.start as usize..range.end as usize]
    }
    pub fn get_meshes_mut(&mut self, which: MeshGroup, mesh_number: usize) -> &mut [Transform3D] {
        let group = &mut self.groups[which.0];
        let mesh = &mut group.meshes[mesh_number];
        let range = mesh.instances.clone();
        &mut group.instance_data[range.start as usize..range.end as usize]
    }
    /// Deletes a mesh group.  Note that this currently invalidates
    /// all the MeshGroup handles after this one, which is not great.  Only use it on the
    /// last mesh group if that matters to you.
    pub fn remove_mesh_group(&mut self, which: MeshGroup) {
        self.groups.remove(which.0);
    }
    pub fn upload_meshes(
        &mut self,
        gpu: &crate::WGPU,
        which: MeshGroup,
        mesh_number: usize,
        range: impl std::ops::RangeBounds<usize>,
    ) {
        let group = &self.groups[which.0];
        let mesh = &group.meshes[mesh_number];
        let range = crate::range(
            range,
            mesh.instances.end as usize - mesh.instances.start as usize,
        );
        // offset range by instance_start
        gpu.queue.write_buffer(
            &group.instance_buffer,
            mesh.instances.start as u64 + range.start as u64,
            bytemuck::cast_slice(
                &group.instance_data[(mesh.instances.start as usize + range.start)
                    ..(mesh.instances.start as usize + range.end)],
            ),
        );
    }
    pub fn upload_meshes_group(&mut self, gpu: &crate::WGPU, which: MeshGroup) {
        // upload the whole instance buffer
        let group = &self.groups[which.0];
        gpu.queue.write_buffer(
            &group.instance_buffer,
            0,
            bytemuck::cast_slice(&group.instance_data),
        );
    }
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
        // camera
        rpass.set_bind_group(0, &self.camera_bind_group, &[]);
        for group in self.groups[which].iter() {
            rpass.set_bind_group(1, &group.tex_bind_group, &[]);
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MeshGroup(usize);

pub struct MeshEntry {
    pub instance_count: u32,
    pub submeshes: Vec<SubmeshEntry>,
}
pub type SubmeshEntry = SubmeshData;
