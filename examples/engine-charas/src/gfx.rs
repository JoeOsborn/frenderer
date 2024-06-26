#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Spritesheet(pub(crate) usize);

pub struct BitFont {
    pub(crate) _spritesheet: Spritesheet,
    pub(crate) font: frenderer::bitfont::BitFont,
}
use crate::geom;
pub(crate) struct TextDraw(
    pub(crate) frenderer::bitfont::BitFont,
    pub(crate) String,
    pub(crate) geom::Vec2,
    pub(crate) f32,
);
