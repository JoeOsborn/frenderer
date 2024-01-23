#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Spritesheet(pub(crate) usize);

pub struct BitFont<B: std::ops::RangeBounds<char> = std::ops::RangeInclusive<char>> {
    pub(crate) _spritesheet: Spritesheet,
    pub(crate) font: frenderer::bitfont::BitFont<B>,
}
use crate::geom;
pub(crate) struct TextDraw(
    pub(crate) frenderer::bitfont::BitFont,
    pub(crate) String,
    pub(crate) geom::Vec2,
    pub(crate) f32,
);
