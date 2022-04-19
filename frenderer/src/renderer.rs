pub mod flat;
pub mod skinned;
pub mod sprites;
pub mod textured;
use crate::animation;
use crate::assets;
use crate::camera::Camera;
use crate::types::*;
use std::collections::HashMap;
use std::rc::Rc;
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RenderKey(usize);

trait SingleRenderState: Clone {
    fn interpolate(&self, other: &Self, r: f32) -> Self;
}
trait Renderer {
    type BatchRenderKey: Clone + std::hash::Hash + Eq;
    type SingleRenderState: SingleRenderState;
}
struct RenderTable<T: Renderer> {
    // TODO: replace with vec, require renderkeys ascending?  or a sparse vec?
    // It may make sense to use something like:
    // Hash <batch key, (render key, singlerenderstate)>
    // but that makes interpolation hard.
    // So just leave it as is for now.
    interpolated: HashMap<RenderKey, (T::BatchRenderKey, T::SingleRenderState)>,
    raw: HashMap<T::BatchRenderKey, Vec<T::SingleRenderState>>,
}
impl<T: Renderer> RenderTable<T> {
    fn new() -> Self {
        Self {
            interpolated: HashMap::new(),
            raw: HashMap::new(),
        }
    }
    fn clear(&mut self) {
        self.interpolated.clear();
        for v in self.raw.values_mut() {
            v.clear();
        }
    }
    fn insert(&mut self, rk: RenderKey, bk: T::BatchRenderKey, rs: T::SingleRenderState) {
        assert!(self.interpolated.insert(rk, (bk, rs)).is_none());
    }
    fn extend_raw(
        &mut self,
        bk: T::BatchRenderKey,
        rs: impl IntoIterator<Item = T::SingleRenderState>,
    ) {
        self.raw.entry(bk).or_insert(vec![]).extend(rs);
    }
    fn interpolate_from(&mut self, rt1: &Self, rt2: &Self, r: f32) {
        for (k, (bk, v1)) in rt2.interpolated.iter() {
            let v0 = rt1.interpolated.get(k).map(|(_, v0)| v0);
            self.interpolated.insert(
                *k,
                (
                    bk.clone(),
                    v0.map(|v0| v0.interpolate(v1, r))
                        .unwrap_or_else(|| v1.clone()),
                ),
            );
        }
        for (k, vs) in rt2.raw.iter() {
            self.raw
                .entry(k.clone())
                .or_insert(vec![])
                .extend(vs.into_iter().cloned());
        }
    }
}

pub struct RenderState {
    skinned: HashMap<RenderKey, skinned::SingleRenderState>,
    sprites: RenderTable<sprites::Renderer>,
    flats: HashMap<RenderKey, flat::SingleRenderState>,
    textured: HashMap<RenderKey, textured::SingleRenderState>,
    pub(crate) camera: Camera,
}
impl RenderState {
    pub fn new(cam: Camera) -> Self {
        Self {
            skinned: HashMap::new(),
            sprites: RenderTable::new(),
            flats: HashMap::new(),
            textured: HashMap::new(),
            camera: cam,
        }
    }
    pub fn camera_mut(&mut self) -> &mut Camera {
        &mut self.camera
    }
    pub fn set_camera(&mut self, c: Camera) {
        self.camera = c;
    }
    pub fn clear(&mut self) {
        self.skinned.clear();
        self.sprites.clear();
        self.flats.clear();
        self.textured.clear();
    }
    pub fn interpolate_from(&mut self, rs1: &Self, rs2: &Self, r: f32) {
        for (k, v1) in rs2.skinned.iter() {
            let v0 = rs1.skinned.get(k).unwrap_or(v1);
            self.skinned.insert(*k, v0.interpolate(v1, r));
        }
        self.sprites.interpolate_from(&rs1.sprites, &rs2.sprites, r);
        for (k, v1) in rs2.flats.iter() {
            let v0 = rs1.flats.get(k).unwrap_or(v1);
            self.flats.insert(*k, v0.interpolate(v1, r));
        }
        for (k, v1) in rs2.textured.iter() {
            let v0 = rs1.textured.get(k).unwrap_or(v1);
            self.textured.insert(*k, v0.interpolate(v1, r));
        }
        self.camera = rs1.camera.interpolate(&rs2.camera, r);
    }

    pub fn render_skinned(
        &mut self,
        model: Rc<skinned::Model>,
        animation: assets::AnimRef,
        state: animation::AnimationState,
        transform: Similarity3,
        key: usize,
    ) {
        assert!(self
            .skinned
            .insert(
                RenderKey(key),
                skinned::SingleRenderState::new(model, animation, state, transform),
            )
            .is_none());
    }
    pub fn render_textured(
        &mut self,
        model: Rc<textured::Model>,
        transform: Similarity3,
        key: usize,
    ) {
        assert!(self
            .textured
            .insert(
                RenderKey(key),
                textured::SingleRenderState::new(model, transform),
            )
            .is_none());
    }
    pub fn render_sprite(
        &mut self,
        key: usize,
        tex: assets::TextureRef,
        data: sprites::SingleRenderState,
    ) {
        self.sprites.insert(RenderKey(key), tex, data);
    }
    pub fn render_sprites_raw(
        &mut self,
        tex: assets::TextureRef,
        data: impl IntoIterator<Item = sprites::SingleRenderState>,
    ) {
        self.sprites.extend_raw(tex, data);
    }
    pub fn render_flat(&mut self, model: Rc<flat::Model>, transform: Similarity3, key: usize) {
        assert!(self
            .flats
            .insert(
                RenderKey(key),
                flat::SingleRenderState::new(model, transform),
            )
            .is_none());
    }
}
