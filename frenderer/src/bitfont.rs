use std::ops::RangeBounds;

use crate::{SheetRegion, SpriteRenderer, Transform};

/// A bitmapped font helper described as a rectangular area of a spritesheet.
#[derive(Clone, Copy, Debug)]
pub struct BitFont<B: RangeBounds<char> = std::ops::RangeInclusive<char>> {
    region: SheetRegion,
    char_w: u16,
    char_h: u16,
    chars: B,
}

impl<B: RangeBounds<char>> BitFont<B> {
    /// Creates a bitfont data structure; the bounds used must not be
    /// open on either end.  Each character is assumed to be the same
    /// size, with width equal to the width of the region divided by
    /// the number of characters in the row and height equal to the
    /// height of the region divided by the number of rows (the number
    /// of characters divided by the number of rows).
    ///
    /// Panics if the sheet region is not big enough to hold all the
    /// characters at the given character sizes, or if the sheet
    /// region's width or height are not multiples of the character
    /// width and height.
    pub fn with_sheet_region(chars: B, region: SheetRegion, char_w: u16, char_h: u16) -> Self {
        if let std::ops::Bound::Unbounded = chars.start_bound() {
            panic!("Can't use unbounded lower bound on bitfont chars");
        }
        if let std::ops::Bound::Unbounded = chars.end_bound() {
            panic!("Can't use unbounded upper bound on bitfont chars");
        }
        let end_char: u32 = match chars.end_bound() {
            std::ops::Bound::Included(&c) => u32::from(c) + 1,
            std::ops::Bound::Excluded(&c) => u32::from(c),
            _ => unreachable!(),
        };
        let start_char: u32 = match chars.start_bound() {
            std::ops::Bound::Included(&c) => u32::from(c),
            std::ops::Bound::Excluded(&c) => u32::from(c) + 1,
            _ => unreachable!(),
        };
        let char_count = end_char - start_char;
        let chars_per_row = region.w / char_w;
        let rows = (char_count / chars_per_row as u32) as u16;
        assert_eq!(
            region.w % char_w,
            0,
            "Sheet region width must be a multiple of character width"
        );
        assert_eq!(
            region.h % char_h,
            0,
            "Sheet region height must be a multiple of character height"
        );
        assert!(region.w >= chars_per_row * char_w);
        assert!(region.h >= rows * char_h);
        Self {
            chars,
            char_w,
            char_h,
            region,
        }
    }
    /// Draws the given `text` as a single line of characters of height `char_height`.
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
        char_height: f32,
    ) -> [f32; 2] {
        let start_char: u32 = match self.chars.start_bound() {
            std::ops::Bound::Included(&c) => u32::from(c),
            std::ops::Bound::Excluded(&c) => u32::from(c) + 1,
            _ => unreachable!(),
        };
        let chars_per_row = self.region.w / self.char_w;
        let (trfs, uvs) = sprites.get_sprites_mut(group);
        let aspect = self.char_w as f32 / self.char_h as f32;
        let char_width = aspect * char_height;
        screen_pos[0] += char_width / 2.0;
        screen_pos[1] -= char_height / 2.0;
        for (chara, (trf, uv)) in text
            .chars()
            .zip(trfs[start..].iter_mut().zip(uvs[start..].iter_mut()))
        {
            if !self.chars.contains(&chara) {
                panic!("Drawing outside of font character range");
            }
            *trf = Transform {
                w: char_width as u16,
                h: char_height as u16,
                x: screen_pos[0],
                y: screen_pos[1],
                rot: 0.0,
            };
            let chara = u32::from(chara) - start_char;
            let which_row = chara / chars_per_row as u32;
            let which_col = chara % chars_per_row as u32;
            *uv = SheetRegion::new(
                self.region.sheet,
                self.region.x + (which_col as u16) * self.char_w,
                self.region.y + (which_row as u16) * self.char_h,
                0,
                self.char_w,
                self.char_h,
            );
            screen_pos[0] += char_width;
        }
        [
            screen_pos[0] + char_width / 2.0,
            screen_pos[1] + char_height / 2.0,
        ]
    }
}
