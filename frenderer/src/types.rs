use bytemuck::{Pod, Zeroable};
pub use std::f32::consts::PI;
pub use ultraviolet::bivec::Bivec3;
pub use ultraviolet::mat::Mat4;
pub use ultraviolet::rotor::Rotor3;
pub use ultraviolet::transform::{Isometry3, Similarity3};
pub use ultraviolet::vec::{Vec2, Vec3, Vec4};
pub use ultraviolet::Lerp;

#[repr(C)]
#[derive(Clone, Copy, Zeroable, Default, Pod, Debug, PartialEq)]
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
impl Color {
    pub fn to_rgba8888_array(&self) -> [u8; 4] {
        [self.0, self.1, self.2, self.3]
    }
    // crummy, bad, linear interpolation
    pub fn interpolate(&self, other: &Self, r: f32) -> Self {
        let Color(sr, sg, sb, sa) = *self;
        let Color(or, og, ob, oa) = *other;
        let (sr, sg, sb, sa) = (sr as f32, sg as f32, sb as f32, sa as f32);
        let (or, og, ob, oa) = (or as f32, og as f32, ob as f32, oa as f32);
        Self(
            sr.interpolate(or, r) as u8,
            sg.interpolate(og, r) as u8,
            sb.interpolate(ob, r) as u8,
            sa.interpolate(oa, r) as u8,
        )
    }
}

pub trait Interpolate {
    fn interpolate(&self, other: Self, r: f32) -> Self;
    fn interpolate_limit(&self, other: Self, r: f32, lim: f32) -> Self;
}
impl Interpolate for f32 {
    fn interpolate(&self, other: Self, r: f32) -> Self {
        self.lerp(other, r)
    }
    fn interpolate_limit(&self, other: Self, r: f32, lim: f32) -> Self {
        if (other - self).abs() >= lim {
            other
        } else {
            self.interpolate(other, r)
        }
    }
}
impl Interpolate for Similarity3 {
    fn interpolate(&self, other: Self, r: f32) -> Self {
        Self::new(
            self.translation.interpolate(other.translation, r),
            self.rotation.interpolate(other.rotation, r),
            self.scale.interpolate(other.scale, r),
        )
    }
    fn interpolate_limit(&self, other: Self, r: f32, lim: f32) -> Self {
        Self::new(
            self.translation
                .interpolate_limit(other.translation, r, lim),
            self.rotation.interpolate_limit(other.rotation, r, PI / 4.0),
            self.scale.interpolate_limit(other.scale, r, 0.5),
        )
    }
}
impl Interpolate for Isometry3 {
    fn interpolate(&self, other: Self, r: f32) -> Self {
        Self::new(
            self.translation.interpolate(other.translation, r),
            self.rotation.interpolate(other.rotation, r),
        )
    }
    fn interpolate_limit(&self, other: Self, r: f32, lim: f32) -> Self {
        Self::new(
            self.translation
                .interpolate_limit(other.translation, r, lim),
            self.rotation.interpolate_limit(other.rotation, r, PI / 4.0),
        )
    }
}

impl Interpolate for Rect {
    fn interpolate(&self, other: Self, r: f32) -> Self {
        Self {
            pos: self.pos.interpolate(other.pos, r),
            sz: self.sz.interpolate(other.sz, r),
        }
    }
    fn interpolate_limit(&self, other: Self, r: f32, lim: f32) -> Self {
        Self {
            pos: self.pos.interpolate_limit(other.pos, r, lim),
            sz: self.sz.interpolate_limit(other.sz, r, 0.5),
        }
    }
}

impl Interpolate for Vec2 {
    fn interpolate(&self, other: Self, r: f32) -> Self {
        Vec2::lerp(self, other, r)
    }
    fn interpolate_limit(&self, other: Self, r: f32, lim: f32) -> Self {
        if (other - *self).mag_sq() >= lim * lim {
            other
        } else {
            self.interpolate(other, r)
        }
    }
}

impl Interpolate for Vec3 {
    fn interpolate(&self, other: Self, r: f32) -> Self {
        Vec3::lerp(self, other, r)
    }
    fn interpolate_limit(&self, other: Self, r: f32, lim: f32) -> Self {
        if (other - *self).mag_sq() >= lim * lim {
            other
        } else {
            self.interpolate(other, r)
        }
    }
}

impl Interpolate for Rotor3 {
    fn interpolate(&self, other: Self, r: f32) -> Self {
        self.lerp(other, r).normalized()
    }
    fn interpolate_limit(&self, other: Self, r: f32, _lim: f32) -> Self {
        self.interpolate(other, r)
    }
}
