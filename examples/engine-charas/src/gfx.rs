#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Spritesheet(pub(crate) usize);

pub struct BitFont<B: std::ops::RangeBounds<char> = std::ops::RangeInclusive<char>> {
    pub(crate) _spritesheet: Spritesheet,
    pub(crate) font: helperer::BitFont<B>,
}
use crate::geom;
pub(crate) struct TextDraw(
    pub(crate) helperer::BitFont,
    pub(crate) String,
    pub(crate) geom::Vec2,
    pub(crate) f32,
);
