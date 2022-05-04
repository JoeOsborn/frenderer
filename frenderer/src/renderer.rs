pub mod billboard;
mod common;
pub mod flat;
pub mod skinned;
pub mod sprites;
pub mod textured;
use crate::camera::Camera;
use crate::{assets, types::Interpolate};
use std::{cell::RefCell, rc::Rc};
use std::{collections::HashMap, fmt::Debug};
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct RenderKey(usize);

trait SingleRenderState: Clone {
    fn interpolate(&self, other: &Self, r: f32) -> Self;
}
trait Renderer {
    type BatchRenderKey: Clone + std::hash::Hash + Eq + Debug;
    type SingleRenderState: SingleRenderState + Debug;
}
struct RenderTable<T: Renderer> {
    // TODO: replace with vec, require renderkeys ascending?  or a sparse vec?
    interpolated:
        HashMap<T::BatchRenderKey, (Vec<RenderKey>, Rc<RefCell<Vec<T::SingleRenderState>>>)>,
    raw: HashMap<T::BatchRenderKey, Vec<Rc<RefCell<Vec<T::SingleRenderState>>>>>,
}
impl<T: Renderer> RenderTable<T> {
    fn new() -> Self {
        Self {
            interpolated: HashMap::new(),
            raw: HashMap::new(),
        }
    }
    fn clear(&mut self) {
        for (v1, v2) in self.interpolated.values_mut() {
            v1.clear();
            v2.borrow_mut().clear();
        }
        for v in self.raw.values_mut() {
            v.clear();
        }
    }
    fn insert_interpolated(
        &mut self,
        bk: T::BatchRenderKey,
        rk: RenderKey,
        rs: T::SingleRenderState,
    ) {
        self.extend_interpolated(bk, std::iter::once(rk), std::iter::once(rs));
    }
    fn extend_interpolated(
        &mut self,
        bk: T::BatchRenderKey,
        rk: impl IntoIterator<Item = RenderKey>,
        rs: impl IntoIterator<Item = T::SingleRenderState>,
    ) {
        let (keys, states) = self
            .interpolated
            .entry(bk.clone())
            .or_insert((vec![], Rc::new(RefCell::new(vec![]))));
        let mut states = states.borrow_mut();
        for (rk, rs) in rk.into_iter().zip(rs.into_iter()) {
            if keys.is_empty() || *keys.last().unwrap() < rk {
                // linear if keys are ordered
                keys.push(rk);
                states.push(rs);
            } else {
                // TODO measure out of order inserts for performance advice
                // each one linear-logarithmic if keys are in random order
                let pos = keys.binary_search(&rk);
                assert!(
                    pos.is_err(),
                    "Duplicate render key {:?} {:?} {:?}",
                    rk,
                    &bk,
                    rs
                );
                let pos = pos.unwrap_err();
                keys.insert(pos, rk);
                states.insert(pos, rs);
            }
        }
    }
    fn extend_raw(&mut self, bk: T::BatchRenderKey, rs: Rc<RefCell<Vec<T::SingleRenderState>>>) {
        self.raw.entry(bk).or_insert(vec![]).push(rs);
    }
    fn interpolate_from(&mut self, rt1: &Self, rt2: &Self, r: f32) {
        self.clear();
        for (bk, (ks, vs)) in rt2.interpolated.iter() {
            let vs = vs.borrow();
            // walk through ks and ks0, skipping entries in ks0 that aren't in ks
            let (keys, vals) = self
                .interpolated
                .entry(bk.clone())
                .or_insert((vec![], Rc::new(RefCell::new(vec![]))));
            let mut vals = vals.borrow_mut();
            keys.extend(ks.iter().copied());
            if let Some((ks0, vs0)) = rt1.interpolated.get(bk) {
                let vs0 = vs0.borrow();
                let j_max = ks0.len();
                let mut j = 0;
                // linear in the number of keys
                vals.extend(ks.iter().zip(vs.iter()).map(|(k1, v1)| {
                    if j < j_max {
                        let mut kj = ks0[j];
                        while kj < *k1 && j < j_max - 1 {
                            j += 1;
                            // used to render kj, not rendering it now, skip it
                            kj = ks0[j];
                        }
                        if j < j_max {
                            if kj == *k1 {
                                let v0 = &vs0[j];
                                v0.interpolate(v1, r)
                            } else {
                                // kj > k1, skip ahead
                                v1.clone()
                            }
                        } else {
                            // no kj
                            v1.clone()
                        }
                    } else {
                        // no kj
                        v1.clone()
                    }
                }));
            } else {
                vals.extend(vs.iter().cloned())
            }
        }
        for (k, vs) in rt2.raw.iter() {
            *self.raw.entry(k.clone()).or_insert(vec![]) = vs.clone();
        }
    }
}

pub struct RenderState {
    skinned: RenderTable<skinned::Renderer>,
    sprites: RenderTable<sprites::Renderer>,
    billboards: RenderTable<billboard::Renderer>,
    flats: RenderTable<flat::Renderer>,
    textured: RenderTable<textured::Renderer>,
    pub(crate) camera: Camera,
}
impl RenderState {
    pub fn new(cam: Camera) -> Self {
        Self {
            skinned: RenderTable::new(),
            sprites: RenderTable::new(),
            billboards: RenderTable::new(),
            flats: RenderTable::new(),
            textured: RenderTable::new(),
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
        self.billboards.clear();
        self.flats.clear();
        self.textured.clear();
    }
    pub fn interpolate_from(&mut self, rs1: &Self, rs2: &Self, r: f32) {
        self.skinned.interpolate_from(&rs1.skinned, &rs2.skinned, r);
        self.sprites.interpolate_from(&rs1.sprites, &rs2.sprites, r);
        self.billboards
            .interpolate_from(&rs1.billboards, &rs2.billboards, r);
        self.flats.interpolate_from(&rs1.flats, &rs2.flats, r);
        self.textured
            .interpolate_from(&rs1.textured, &rs2.textured, r);
        self.camera = rs1.camera.interpolate_limit(rs2.camera, r, 5.0);
    }
    pub fn render_skinned_interpolated(
        &mut self,
        model: Rc<skinned::Model>,
        key: usize,
        data: skinned::SingleRenderState,
    ) {
        self.skinned
            .insert_interpolated(model, RenderKey(key), data);
    }
    pub fn render_skinned_interpolated_batch(
        &mut self,
        model: Rc<skinned::Model>,
        keys: impl IntoIterator<Item = usize>,
        data: impl IntoIterator<Item = skinned::SingleRenderState>,
    ) {
        self.skinned
            .extend_interpolated(model, keys.into_iter().map(RenderKey), data);
    }
    pub fn render_skinned_batch(
        &mut self,
        model: Rc<skinned::Model>,
        data: Rc<RefCell<Vec<skinned::SingleRenderState>>>,
    ) {
        self.skinned.extend_raw(model, data);
    }
    pub fn render_textured_interpolated(
        &mut self,
        model: Rc<textured::Model>,
        key: usize,
        data: textured::SingleRenderState,
    ) {
        self.textured
            .insert_interpolated(model, RenderKey(key), data);
    }
    pub fn render_textured_interpolated_batch(
        &mut self,
        model: Rc<textured::Model>,
        keys: impl IntoIterator<Item = usize>,
        data: impl IntoIterator<Item = textured::SingleRenderState>,
    ) {
        self.textured
            .extend_interpolated(model, keys.into_iter().map(RenderKey), data);
    }
    pub fn render_textured_batch(
        &mut self,
        model: Rc<textured::Model>,
        data: Rc<RefCell<Vec<textured::SingleRenderState>>>,
    ) {
        self.textured.extend_raw(model, data);
    }
    pub fn render_sprite_interpolated(
        &mut self,
        tex: assets::TextureRef,
        key: usize,
        data: sprites::SingleRenderState,
    ) {
        self.sprites.insert_interpolated(tex, RenderKey(key), data);
    }
    pub fn render_sprite_interpolated_batch(
        &mut self,
        tex: assets::TextureRef,
        keys: impl IntoIterator<Item = usize>,
        data: impl IntoIterator<Item = sprites::SingleRenderState>,
    ) {
        self.sprites
            .extend_interpolated(tex, keys.into_iter().map(RenderKey), data);
    }
    pub fn render_sprite_batch(
        &mut self,
        tex: assets::TextureRef,
        data: Rc<RefCell<Vec<sprites::SingleRenderState>>>,
    ) {
        self.sprites.extend_raw(tex, data);
    }
    pub fn render_flat_interpolated(
        &mut self,
        model: Rc<flat::Model>,
        key: usize,
        data: flat::SingleRenderState,
    ) {
        self.flats.insert_interpolated(model, RenderKey(key), data);
    }
    pub fn render_flat_interpolated_batch(
        &mut self,
        model: Rc<flat::Model>,
        keys: impl IntoIterator<Item = usize>,
        data: impl IntoIterator<Item = flat::SingleRenderState>,
    ) {
        self.flats
            .extend_interpolated(model, keys.into_iter().map(RenderKey), data);
    }
    pub fn render_flat_batch(
        &mut self,
        model: Rc<flat::Model>,
        data: Rc<RefCell<Vec<flat::SingleRenderState>>>,
    ) {
        self.flats.extend_raw(model, data);
    }
    pub fn render_billboard_interpolated(
        &mut self,
        (tex, mode): (assets::TextureRef, billboard::BlendMode),
        key: usize,
        data: billboard::SingleRenderState,
    ) {
        self.billboards
            .insert_interpolated((tex, mode), RenderKey(key), data);
    }
    pub fn render_billboard_interpolated_batch(
        &mut self,
        batch: (assets::TextureRef, billboard::BlendMode),
        keys: impl IntoIterator<Item = usize>,
        data: impl IntoIterator<Item = billboard::SingleRenderState>,
    ) {
        self.billboards
            .extend_interpolated(batch, keys.into_iter().map(RenderKey), data);
    }
    pub fn render_billboard_batch(
        &mut self,
        (tex, mode): (assets::TextureRef, billboard::BlendMode),
        data: Rc<RefCell<Vec<billboard::SingleRenderState>>>,
    ) {
        self.billboards.extend_raw((tex, mode), data);
    }
}
