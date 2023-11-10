use frenderer::{Camera2D, Transform};
pub use glam::*;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Zeroable, bytemuck::Pod, Debug)]
pub struct Rect {
    pub corner: Vec2,
    pub size: Vec2,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Zeroable, bytemuck::Pod, Debug)]
pub struct AABB {
    pub center: Vec2,
    pub size: Vec2,
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

impl From<Rect> for Camera2D {
    fn from(val: Rect) -> Self {
        Camera2D {
            screen_pos: val.corner.into(),
            screen_size: val.size.into(),
        }
    }
}

impl From<AABB> for Camera2D {
    fn from(val: AABB) -> Self {
        Camera2D {
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
}
