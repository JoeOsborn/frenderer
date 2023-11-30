use std::ops::RangeBounds;

use crate::{SheetRegion, SpriteRenderer, Transform};

#[derive(Clone, Copy, Debug)]
pub struct BitFont<B: RangeBounds<char> = std::ops::RangeInclusive<char>> {
    region: SheetRegion,
    chars_per_row: u16,
    chars: B,
}

impl<B: RangeBounds<char>> BitFont<B> {
    /// Creates a bitfont data structure; the bounds used must not be open on either end.
    pub fn with_sheet_region(chars: B, uvs: SheetRegion, chars_per_row: u16) -> Self {
        if let std::ops::Bound::Unbounded = chars.start_bound() {
            panic!("Can't use unbounded lower bound on bitfont chars");
        }
        if let std::ops::Bound::Unbounded = chars.end_bound() {
            panic!("Can't use unbounded upper bound on bitfont chars");
        }
        Self {
            chars,
            chars_per_row,
            region: uvs,
        }
    }
    /// Draws the given `text` as a single line of characters of size `char_sz`.
    /// The given position is the top-left corner of the rendered string.
    /// Panics if any character in text is not within the font's character range.
    /// Returns the bottom right corner of the rendered string.
    pub fn draw_text(
        &self,
        sprites: &mut SpriteRenderer,
        group: usize,
        start: usize,
        text: &str,
        mut screen_pos: [f32; 2],
        char_sz: f32,
    ) -> (usize, [f32; 2]) {
        let char_uv_sz = self.region.w / self.chars_per_row;
        let end_char: u32 = match self.chars.end_bound() {
            std::ops::Bound::Included(&c) => u32::from(c) + 1,
            std::ops::Bound::Excluded(&c) => u32::from(c),
            _ => unreachable!(),
        };
        let start_char: u32 = match self.chars.start_bound() {
            std::ops::Bound::Included(&c) => u32::from(c),
            std::ops::Bound::Excluded(&c) => u32::from(c) + 1,
            _ => unreachable!(),
        };
        let char_count = end_char - start_char;
        let rows = (char_count / self.chars_per_row as u32) as u16;
        assert!(self.region.w >= self.chars_per_row * char_uv_sz);
        assert!(self.region.h >= rows * char_uv_sz);
        let (trfs, uvs) = sprites.get_sprites_mut(group);
        screen_pos[0] += char_sz / 2.0;
        screen_pos[1] -= char_sz / 2.0;
        for (chara, (trf, uv)) in text
            .chars()
            .zip(trfs[start..].iter_mut().zip(uvs[start..].iter_mut()))
        {
            if !self.chars.contains(&chara) {
                panic!("Drawing outside of font character range");
            }
            *trf = Transform {
                w: char_sz as u16,
                h: char_sz as u16,
                x: screen_pos[0],
                y: screen_pos[1],
                rot: 0.0,
            };
            let chara = u32::from(chara) - start_char;
            let which_row = chara / self.chars_per_row as u32;
            let which_col = chara % self.chars_per_row as u32;
            *uv = SheetRegion::new(
                self.region.sheet,
                self.region.x + (which_col as u16) * char_uv_sz,
                self.region.y + (which_row as u16) * char_uv_sz,
                0,
                char_uv_sz,
                char_uv_sz,
            );
            screen_pos[0] += char_sz;
        }
        (
            text.len(),
            [screen_pos[0] + char_sz / 2.0, screen_pos[1] + char_sz / 2.0],
        )
    }
}
