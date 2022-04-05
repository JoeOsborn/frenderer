pub mod flat;
pub mod skinned;
pub mod sprites;
pub mod textured;
use std::rc::Rc;
use crate::animation;
use crate::assets;
use crate::types::*;
use std::collections::HashMap;
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RenderKey(usize);

pub struct RenderState {
    skinned: HashMap<RenderKey, skinned::SingleRenderState>,
    sprites: HashMap<RenderKey, sprites::SingleRenderState>,
    flats: HashMap<RenderKey, flat::SingleRenderState>,
    textured: HashMap<RenderKey, textured::SingleRenderState>,
}
impl RenderState {
    pub fn new() -> Self {
        Self {
            skinned: HashMap::new(),
            sprites: HashMap::new(),
            flats: HashMap::new(),
            textured:HashMap::new(),
        }
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
        for (k, v1) in rs2.sprites.iter() {
            let v0 = rs1.sprites.get(k).unwrap_or(v1);
            self.sprites.insert(*k, v0.interpolate(v1, r));
        }
        for (k, v1) in rs2.flats.iter() {
            let v0 = rs1.flats.get(k).unwrap_or(v1);
            self.flats.insert(*k, v0.interpolate(v1, r));
        }
        for (k, v1) in rs2.textured.iter() {
            let v0 = rs1.textured.get(k).unwrap_or(v1);
            self.textured.insert(*k, v0.interpolate(v1, r));
        }
    }

    pub fn render_skinned(
        &mut self,
        model: Rc<skinned::Model>,
        animation: assets::AnimRef,
        state: animation::AnimationState,
        transform: Similarity3,
        key: usize,
    ) {
        self.skinned.insert(
            RenderKey(key),
            skinned::SingleRenderState::new(model, animation, state, transform),
        );
    }
    pub fn render_textured(
        &mut self,
        model: Rc<textured::Model>,
        transform: Similarity3,
        key: usize,
    ) {
        self.textured.insert(
            RenderKey(key),
            textured::SingleRenderState::new(model, transform),
        );
    }
    pub fn render_sprite(
        &mut self,
        tex: assets::TextureRef,
        region: Rect,
        transform: Isometry3,
        size: Vec2,
        key: usize,
    ) {
        self.sprites.insert(
            RenderKey(key),
            sprites::SingleRenderState::new(tex, region, transform, size),
        );
    }
    pub fn render_flat(&mut self, model: Rc<flat::Model>, transform: Similarity3, key: usize) {
        self.flats.insert(
            RenderKey(key),
            flat::SingleRenderState::new(model, transform),
        );
    }
}
