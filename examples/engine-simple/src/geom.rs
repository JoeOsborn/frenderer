use frenderer::{GPUCamera, Region, Transform};
pub use glam::*;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Zeroable, bytemuck::Pod, Debug)]
pub struct Rect {
    pub pos: Vec2,
    pub sz: Vec2,
}

// TODO: this should probably offset position by half size, since pos is usually the bottom left corner
impl From<Rect> for Transform {
    fn from(val: Rect) -> Self {
        Transform {
            w: val.sz.x as u16,
            h: val.sz.y as u16,
            x: val.pos.x,
            y: val.pos.y,
            rot: 0.0,
        }
    }
}

impl From<Rect> for Region {
    fn from(val: Rect) -> Self {
        Region {
            w: val.sz.x,
            h: val.sz.y,
            x: val.pos.x,
            y: val.pos.y,
        }
    }
}

impl From<Rect> for GPUCamera {
    fn from(val: Rect) -> Self {
        GPUCamera {
            screen_pos: val.pos.into(),
            screen_size: val.sz.into(),
        }
    }
}

impl Rect {
    pub fn displacement(&self, other: Rect) -> Option<Vec2> {
        let x_overlap =
            (self.pos.x + self.sz.x).min(other.pos.x + other.sz.x) - self.pos.x.max(other.pos.x);
        let y_overlap =
            (self.pos.y + self.sz.y).min(other.pos.y + other.sz.y) - self.pos.y.max(other.pos.y);
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
