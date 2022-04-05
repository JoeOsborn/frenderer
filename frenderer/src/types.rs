use bytemuck::{Pod, Zeroable};
pub use std::f32::consts::PI;
pub use ultraviolet::bivec::Bivec3;
pub use ultraviolet::mat::Mat4;
pub use ultraviolet::rotor::Rotor3;
pub use ultraviolet::transform::{Isometry3, Similarity3};
pub use ultraviolet::vec::{Vec2, Vec3, Vec4};
pub use ultraviolet::Lerp;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Rect {
    pub pos: Vec2,
    pub sz: Vec2,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            pos: Vec2::new(x, y),
            sz: Vec2::new(w, h),
        }
    }
    pub fn contains(&self, other: Rect) -> bool {
        let br = self.pos + self.sz;
        let obr = other.pos + other.sz;
        self.pos.x <= other.pos.x && self.pos.y <= other.pos.y && obr.x <= br.x && obr.y <= br.y
    }
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Zeroable, Pod)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

pub trait LerpF {
    fn lerp(&self, other: &Self, r: f32) -> Self;
}
impl LerpF for Similarity3 {
    fn lerp(&self, other: &Self, r: f32) -> Self {
        Self::new(
            self.translation.lerp(other.translation, r),
            self.rotation.lerp(other.rotation, r).normalized(),
            self.scale.lerp(other.scale, r),
        )
    }
}
impl LerpF for Isometry3 {
    fn lerp(&self, other: &Self, r: f32) -> Self {
        Self::new(
            self.translation.lerp(other.translation, r),
            self.rotation.lerp(other.rotation, r).normalized(),
        )
    }
}

impl LerpF for Rect {
    fn lerp(&self, other: &Self, r: f32) -> Self {
        Self {
            pos: self.pos.lerp(other.pos, r),
            sz: self.sz.lerp(other.sz, r),
        }
    }
}
