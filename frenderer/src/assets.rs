use crate::animation;
use crate::color_eyre::eyre::{ensure, eyre};
use crate::image::Image;
use crate::renderer::{flat, skinned, textured};
use crate::types::*;
use crate::vulkan::Vulkan;
use crate::Result;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;
use thunderdome::{Arena, Index};
use vulkano::image::immutable::ImmutableImage;
use vulkano::sync::GpuFuture;

pub struct Texture {
    pub image: Image,
    pub texture: Arc<ImmutableImage>,
}
pub struct Assets {
    skinned_meshes: Arena<skinned::Mesh>,
    textured_meshes: Arena<textured::Mesh>,
    animations: Arena<animation::Animation>,
    textures: Arena<Texture>,
    materials: Arena<flat::Material>,
    materials_by_name: HashMap<String, MaterialRef<flat::Material>>,
    flat_meshes: Arena<flat::Mesh>,
}
impl Assets {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            skinned_meshes: Arena::new(),
            textured_meshes: Arena::new(),
            animations: Arena::new(),
            textures: Arena::new(),
            flat_meshes: Arena::new(),
            materials: Arena::new(),
            materials_by_name: HashMap::new(),
        }
    }
    pub fn load_texture(
        &mut self,
        path: &std::path::Path,
        vulkan: &mut Vulkan,
    ) -> Result<TextureRef> {
        let img = Image::from_file(path)?;
        let (vulk_img, fut) = ImmutableImage::from_iter(
            img.as_slice().iter().copied(),
            vulkano::image::ImageDimensions::Dim2d {
                width: img.sz.x,
                height: img.sz.y,
                array_layers: 1,
            },
            vulkano::image::MipmapsCount::One,
            vulkano::format::Format::R8G8B8A8_SRGB,
            vulkan.queue.clone(),
        )?;
        vulkan.wait_for(Box::new(fut));
        let tid = self.textures.insert(Texture {
            image: img,
            texture: vulk_img,
        });
        Ok(TextureRef(tid))
    }
    pub fn load_skinned(
        &mut self,
        path: &std::path::Path,
        node_root: &[&str],
        vulkan: &mut Vulkan,
    ) -> Result<Vec<MeshRef<skinned::Mesh>>> {
        use russimp::scene::{PostProcess, Scene};
        let scene = Scene::from_file(
            path.to_str()
                .ok_or_else(|| eyre!("Mesh path can't be converted to string: {:?}", path))?,
            vec![
                PostProcess::GenerateUVCoords,
                PostProcess::Triangulate,
                PostProcess::JoinIdenticalVertices,
                PostProcess::FlipUVs,
                PostProcess::LimitBoneWeights,
            ],
        )?;
        let meshes: Result<Vec<_>, _> = scene
            .meshes
            .into_iter()
            .map(|mesh| {
                let rig = animation::Rig::load(
                    scene.root.as_ref().unwrap().clone(),
                    &mesh.bones,
                    node_root,
                )?;
                let verts = &mesh.vertices;
                let uvs = mesh
                    .texture_coords
                    .first()
                    .ok_or_else(|| eyre!("Mesh fbx has no texture coords: {:?}", path))?;
                let uvs = uvs.clone().unwrap_or_else(|| {
                    vec![
                        russimp::Vector3D {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0
                        };
                        verts.len()
                    ]
                });
                ensure!(
                    mesh.faces[0].0.len() == 3,
                    "Mesh face has too many indices: {:?}",
                    mesh.faces[0]
                );
                let mut bone_weights: Vec<[f32; 4]> = vec![[1.0, 0.0, 0.0, 0.0]; verts.len()];
                let mut bone_usage: Vec<[u8; 4]> = vec![[255, 255, 255, 255]; verts.len()];
                for bone in mesh.bones.iter() {
                    let which_bone = rig.which_joint(&bone.name);
                    for vert_weight in bone.weights.iter() {
                        let which_weight = bone_usage[vert_weight.vertex_id as usize]
                            .iter_mut()
                            .position(|b| *b == 255)
                            .unwrap() as usize;
                        bone_usage[vert_weight.vertex_id as usize][which_weight] = which_bone;
                        bone_weights[vert_weight.vertex_id as usize][which_weight] =
                            vert_weight.weight;
                    }
                }
                //dbg!(&bone_weights, &bone_usage);
                // This is safe to allow because we need an ExactSizeIterator of faces
                #[allow(clippy::needless_collect)]
                let faces: Vec<u32> = mesh
                    .faces
                    .iter()
                    .flat_map(|v| v.0.iter().copied())
                    .collect();
                let (vb, vb_fut) = vulkano::buffer::ImmutableBuffer::from_iter(
                    verts
                        .iter()
                        .zip(uvs.into_iter())
                        .zip(bone_weights.iter())
                        .zip(bone_usage.iter())
                        .map(|(((pos, uv), weights), usage)| skinned::Vertex {
                            position: [pos.x, pos.y, pos.z],
                            uv: [uv.x, uv.y],
                            bone_weights: {
                                let w: f32 = weights.iter().sum();
                                [
                                    weights[0] / w,
                                    weights[1] / w,
                                    weights[2] / w,
                                    weights[3] / w,
                                ]
                            },
                            bone_ids: ((usage[0] as u32) << 24)
                                | ((usage[1] as u32) << 16)
                                | ((usage[2] as u32) << 8)
                                | (usage[3] as u32),
                        }),
                    vulkano::buffer::BufferUsage::vertex_buffer(),
                    vulkan.queue.clone(),
                )?;
                let (ib, ib_fut) = vulkano::buffer::ImmutableBuffer::from_iter(
                    faces.into_iter(),
                    vulkano::buffer::BufferUsage::index_buffer(),
                    vulkan.queue.clone(),
                )?;

                let load_fut = vb_fut.join(ib_fut);
                vulkan.wait_for(Box::new(load_fut));

                let mid = self.skinned_meshes.insert(skinned::Mesh {
                    mesh,
                    rig,
                    verts: vb,
                    idx: ib,
                });
                Ok(MeshRef(mid, PhantomData))
            })
            .collect();
        meshes
    }
    pub fn load_textured(
        &mut self,
        path: &std::path::Path,
        vulkan: &mut Vulkan,
    ) -> Result<Vec<MeshRef<textured::Mesh>>> {
        use russimp::scene::{PostProcess, Scene};
        let scene = Scene::from_file(
            path.to_str()
                .ok_or_else(|| eyre!("Mesh path can't be converted to string: {:?}", path))?,
            vec![
                PostProcess::GenerateUVCoords,
                PostProcess::Triangulate,
                PostProcess::JoinIdenticalVertices,
                PostProcess::FlipUVs,
            ],
        )?;
        let meshes: Result<Vec<_>, _> = scene
            .meshes
            .into_iter()
            .map(|mesh| {
                let verts = &mesh.vertices;
                let uvs = mesh
                    .texture_coords
                    .first()
                    .ok_or_else(|| eyre!("Mesh fbx has no texture coords: {:?}", path))?;
                let uvs = uvs.clone().unwrap_or_else(|| {
                    vec![
                        russimp::Vector3D {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0
                        };
                        verts.len()
                    ]
                });
                ensure!(
                    mesh.faces[0].0.len() == 3,
                    "Mesh face has too many indices: {:?}",
                    mesh.faces[0]
                );
                // This is safe to allow because we need an ExactSizeIterator of faces
                #[allow(clippy::needless_collect)]
                let faces: Vec<u32> = mesh
                    .faces
                    .iter()
                    .flat_map(|v| v.0.iter().copied())
                    .collect();
                let (vb, vb_fut) = vulkano::buffer::ImmutableBuffer::from_iter(
                    verts.iter().zip(uvs.into_iter()).map(|(pos, uv)| {
                        crate::renderer::textured::Vertex {
                            position: [pos.x, pos.y, pos.z],
                            uv: [uv.x, uv.y],
                        }
                    }),
                    vulkano::buffer::BufferUsage::vertex_buffer(),
                    vulkan.queue.clone(),
                )?;
                let (ib, ib_fut) = vulkano::buffer::ImmutableBuffer::from_iter(
                    faces.into_iter(),
                    vulkano::buffer::BufferUsage::index_buffer(),
                    vulkan.queue.clone(),
                )?;

                let load_fut = vb_fut.join(ib_fut);
                vulkan.wait_for(Box::new(load_fut));

                let mid = self
                    .textured_meshes
                    .insert(crate::renderer::textured::Mesh {
                        mesh,
                        verts: vb,
                        idx: ib,
                    });
                Ok(MeshRef(mid, PhantomData))
            })
            .collect();
        meshes
    }
    pub fn load_anim(
        &mut self,
        path: &std::path::Path,
        mesh: MeshRef<skinned::Mesh>,
        settings: animation::AnimationSettings,
        which: &str,
    ) -> Result<AnimRef> {
        use russimp::scene::Scene;
        let scene = Scene::from_file(
            path.to_str()
                .ok_or_else(|| eyre!("Anim path can't be converted to string: {:?}", path))?,
            vec![],
        )?;
        let rig = &self.skinned_mesh(mesh).rig;
        // assumption: one animation per file
        let anim = animation::Animation::load(
            scene
                .animations
                .iter()
                .find(|a| a.name == which)
                .ok_or_else(|| eyre!("Animation {:?} not found", which))?,
            rig,
            settings,
        )?;
        let aid = self.animations.insert(anim);
        Ok(AnimRef(aid))
    }
    pub fn load_flat(
        &mut self,
        path: &std::path::Path,
        vulkan: &mut Vulkan,
    ) -> Result<Rc<flat::Model>> {
        use russimp::scene::{PostProcess, Scene};
        let scene = Scene::from_file(
            path.to_str()
                .ok_or_else(|| eyre!("Mesh path can't be converted to string: {:?}", path))?,
            vec![
                PostProcess::Triangulate,
                PostProcess::JoinIdenticalVertices,
                PostProcess::LimitBoneWeights,
            ],
        )?;
        let mats: Vec<MaterialRef<flat::Material>> = scene
            .materials
            .into_iter()
            .map(|mat| {
                let color = mat
                    .properties
                    .iter()
                    .find(|p| p.key == "$clr.base")
                    .and_then(|p| {
                        if let russimp::material::PropertyTypeInfo::FloatArray(fs) = &p.data {
                            Some(Vec4::new(fs[0], fs[1], fs[2], fs[3]))
                        } else {
                            None
                        }
                    })
                    .unwrap_or(Vec4::new(1., 1., 1., 1.));
                let name = mat
                    .properties
                    .iter()
                    .find(|p| p.key == "?mat.name")
                    .and_then(|p| {
                        if let russimp::material::PropertyTypeInfo::String(n) = &p.data {
                            Some(n.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| "BLANK".to_string());
                match self.materials_by_name.entry(name.clone()) {
                    std::collections::hash_map::Entry::Occupied(e) => {
                        println!(
                            "Skip material {:?}, already found {:?}",
                            (color, name),
                            self.materials[e.get().0]
                        );
                        *e.get()
                    }
                    std::collections::hash_map::Entry::Vacant(e) => {
                        let (buffer, fut) = vulkano::buffer::ImmutableBuffer::from_data(
                            color.into(),
                            vulkano::buffer::BufferUsage::uniform_buffer(),
                            vulkan.queue.clone(),
                        )
                        .unwrap();
                        vulkan.wait_for(Box::new(fut));
                        let mat_ref = self
                            .materials
                            .insert(flat::Material::new(color, name, buffer));
                        let mat_ref = MaterialRef(mat_ref, PhantomData);
                        e.insert(mat_ref);
                        mat_ref
                    }
                }
            })
            .collect();
        let meshes: Result<Vec<_>, _> = scene
            .meshes
            .into_iter()
            .map(|mesh| {
                let verts = &mesh.vertices;
                ensure!(
                    mesh.faces[0].0.len() == 3,
                    "Mesh face has too many indices: {:?}",
                    mesh.faces[0]
                );
                // This is safe to allow because we need an ExactSizeIterator of faces
                #[allow(clippy::needless_collect)]
                let faces: Vec<u32> = mesh
                    .faces
                    .iter()
                    .flat_map(|v| v.0.iter().copied())
                    .collect();
                let (vb, vb_fut) = vulkano::buffer::ImmutableBuffer::from_iter(
                    verts.iter().map(|pos| flat::Vertex {
                        position: [pos.x, pos.y, pos.z],
                    }),
                    vulkano::buffer::BufferUsage::vertex_buffer(),
                    vulkan.queue.clone(),
                )?;
                let (ib, ib_fut) = vulkano::buffer::ImmutableBuffer::from_iter(
                    faces.into_iter(),
                    vulkano::buffer::BufferUsage::index_buffer(),
                    vulkan.queue.clone(),
                )?;

                let load_fut = vb_fut.join(ib_fut);
                vulkan.wait_for(Box::new(load_fut));

                let mat = mats[mesh.material_index as usize];
                let mid = self.flat_meshes.insert(flat::Mesh {
                    mesh,
                    verts: vb,
                    idx: ib,
                });
                Ok((MeshRef(mid, PhantomData), mat))
            })
            .collect();
        let meshes = meshes?;
        Ok(Rc::new(flat::Model::new(
            meshes.iter().map(|(m, _)| m).copied().collect(),
            meshes.iter().map(|(_, m)| m).copied().collect(),
        )))
    }
    pub fn skinned_mesh(&self, m: MeshRef<skinned::Mesh>) -> &skinned::Mesh {
        &self.skinned_meshes[m.0]
    }
    pub fn textured_mesh(&self, m: MeshRef<textured::Mesh>) -> &textured::Mesh {
        &self.textured_meshes[m.0]
    }
    pub fn flat_mesh(&self, m: MeshRef<flat::Mesh>) -> &flat::Mesh {
        &self.flat_meshes[m.0]
    }
    pub fn material(&self, m: MaterialRef<flat::Material>) -> &flat::Material {
        &self.materials[m.0]
    }
    pub fn texture(&self, m: TextureRef) -> &Texture {
        &self.textures[m.0]
    }
    pub fn animation(&self, m: AnimRef) -> &animation::Animation {
        &self.animations[m.0]
    }
}

pub struct MeshRef<M>(Index, PhantomData<M>);
impl<M> Clone for MeshRef<M> {
    fn clone(&self) -> Self {
        Self(self.0, PhantomData)
    }
}
impl<M> Copy for MeshRef<M> {}
impl<M> PartialEq<MeshRef<M>> for MeshRef<M> {
    fn eq(&self, other: &MeshRef<M>) -> bool {
        self.0 == other.0
    }
}
impl<M> Eq for MeshRef<M> {}
impl<M> std::hash::Hash for MeshRef<M> {
    fn hash<H>(&self, h: &mut H)
    where
        H: std::hash::Hasher,
    {
        self.0.hash(h);
    }
}

pub struct MaterialRef<M>(Index, PhantomData<M>);
impl<M> Clone for MaterialRef<M> {
    fn clone(&self) -> Self {
        Self(self.0, PhantomData)
    }
}
impl<M> Copy for MaterialRef<M> {}
impl<M> PartialEq<MaterialRef<M>> for MaterialRef<M> {
    fn eq(&self, other: &MaterialRef<M>) -> bool {
        self.0 == other.0
    }
}
impl<M> Eq for MaterialRef<M> {}
impl<M> std::hash::Hash for MaterialRef<M> {
    fn hash<H>(&self, h: &mut H)
    where
        H: std::hash::Hasher,
    {
        self.0.hash(h);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureRef(Index);
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnimRef(Index);
