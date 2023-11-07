use std::ops::Add;

pub use bytemuck::Zeroable;
use frenderer::{GPUCamera, Transform};
pub use glam::*;
#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Zeroable, bytemuck::Pod, Debug)]
pub struct Rect {
    pub corner: Vec2,
    pub size: Vec2,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Zeroable, bytemuck::Pod, Debug, Default)]
pub struct AABB {
    pub center: Vec2,
    pub size: Vec2,
}

impl Add<Vec2> for AABB {
    type Output = AABB;

    fn add(self, rhs: Vec2) -> Self::Output {
        Self {
            center: self.center + rhs,
            ..self
        }
    }
}

impl Add<[f32; 2]> for AABB {
    type Output = AABB;

    fn add(self, rhs: [f32; 2]) -> Self::Output {
        self + Vec2::from(rhs)
    }
}

impl From<AABB> for Transform {
    fn from(val: AABB) -> Self {
        Transform {
            w: val.size.x as u16,
            h: val.size.y as u16,
            x: val.center.x,
            y: val.center.y,
            rot: 0.0,
        }
    }
}

impl From<Rect> for Transform {
    fn from(val: Rect) -> Self {
        Transform {
            w: val.size.x as u16,
            h: val.size.y as u16,
            x: val.corner.x + val.size.x / 2.0,
            y: val.corner.y + val.size.y / 2.0,
            rot: 0.0,
        }
    }
}

impl From<Rect> for GPUCamera {
    fn from(val: Rect) -> Self {
        GPUCamera {
            screen_pos: val.corner.into(),
            screen_size: val.size.into(),
        }
    }
}

impl From<AABB> for GPUCamera {
    fn from(val: AABB) -> Self {
        GPUCamera {
            screen_pos: (val.center - val.size / 2.0).into(),
            screen_size: val.size.into(),
        }
    }
}

impl From<AABB> for Rect {
    fn from(val: AABB) -> Self {
        Rect {
            corner: (val.center - val.size / 2.0),
            size: val.size,
        }
    }
}

impl From<Rect> for AABB {
    fn from(val: Rect) -> Self {
        AABB {
            center: (val.corner + val.size / 2.0),
            size: val.size,
        }
    }
}

impl Rect {
    pub fn displacement(&self, other: Rect) -> Option<Vec2> {
        let x_overlap = (self.corner.x + self.size.x).min(other.corner.x + other.size.x)
            - self.corner.x.max(other.corner.x);
        let y_overlap = (self.corner.y + self.size.y).min(other.corner.y + other.size.y)
            - self.corner.y.max(other.corner.y);
        if x_overlap >= 0.0 && y_overlap >= 0.0 {
            // This will return the magnitude of overlap in each axis.
            Some(Vec2 {
                x: x_overlap,
                y: y_overlap,
            })
        } else {
            None
        }
    }
}

impl AABB {
    pub fn displacement(&self, other: AABB) -> Option<Vec2> {
        Rect::from(*self).displacement(Rect::from(other))
    }
    pub fn union(&self, other: AABB) -> Self {
        if self.size.length() < std::f32::EPSILON {
            return other;
        }
        if other.size.length() < std::f32::EPSILON {
            return *self;
        }
        let (bl, tr) = self.corners();
        let (blo, tro) = other.corners();
        let left = bl.x.min(blo.x);
        let right = tr.x.max(tro.x);
        let bot = bl.y.min(blo.y);
        let top = tr.y.max(tro.y);
        let w = right - left;
        let h = top - bot;
        AABB {
            center: Vec2 {
                x: left + w / 2.0,
                y: bot + h / 2.0,
            },
            size: Vec2 { x: w, y: h },
        }
    }
    pub fn corners(&self) -> (Vec2, Vec2) {
        (self.center - self.size / 2.0, self.center + self.size / 2.0)
    }
    // grow to fit other without changing center
    pub fn dilate(&self, other: AABB) -> Self {
        let (blo, tro) = other.corners();
        let blo_xdist = self.center.x - blo.x;
        let blo_ydist = self.center.y - blo.y;
        let tro_xdist = tro.x - self.center.x;
        let tro_ydist = tro.y - self.center.y;
        Self {
            center: self.center,
            size: Vec2 {
                x: (blo_xdist * 2.0).max(tro_xdist * 2.0).max(self.size.x),
                y: (blo_ydist * 2.0).max(tro_ydist * 2.0).max(self.size.y),
            },
        }
    }
}
