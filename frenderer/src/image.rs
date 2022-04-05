use super::types::*;
use color_eyre::eyre::Result;
pub struct Vec2i {
    pub x: u32,
    pub y: u32,
}
pub struct Image {
    buffer: Box<[Color]>,
    pub sz: Vec2i,
}

impl Image {
    pub fn new(w: u32, h: u32) -> Self {
        Self {
            buffer: vec![Color(0, 0, 0, 255); (w * h) as usize].into_boxed_slice(),
            sz: Vec2i { x: w, y: h },
        }
    }
    pub fn as_slice(&self) -> &[Color] {
        &self.buffer
    }
    pub fn from_file(p: &std::path::Path) -> Result<Self> {
        let img = image_reading::open(p)?.into_rgba8();
        let sz = Vec2i {
            x: img.width(),
            y: img.height(),
        };
        let img = img.into_vec();
        Ok(Self {
            buffer: img
                .chunks_exact(4)
                .map(|px| {
                    let a = px[3] as f32 / 255.0;
                    let r = (px[0] as f32 * a) as u8;
                    let g = (px[1] as f32 * a) as u8;
                    let b = (px[2] as f32 * a) as u8;
                    Color(r, g, b, (a * 255.0) as u8)
                })
                .collect(),
            sz,
        })
    }
}
