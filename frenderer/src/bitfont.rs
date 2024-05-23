use std::ops::RangeBounds;

use crate::sprites::{SheetRegion, Transform};

/// A bitmapped font helper described as a rectangular area of a spritesheet.
#[derive(Clone, Copy, Debug)]
pub struct BitFont {
    region: SheetRegion,
    char_w: u16,
    char_h: u16,
    padding_x: u16,
    padding_y: u16,
    start_char: u32,
    end_char: u32,
}

impl BitFont {
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
    pub fn with_sheet_region(
        chars: impl RangeBounds<char>,
        region: SheetRegion,
        char_w: u16,
        char_h: u16,
        padding_x: u16,
        padding_y: u16,
    ) -> Self {
        assert!(region.w > 0);
        assert!(region.h > 0);
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
        let net_char_w = char_w + padding_x;
        let net_char_h = char_h + padding_y;
        let chars_per_row = (region.w as u16 / net_char_w) as u32;
        let rows = (char_count / chars_per_row) as u16;
        assert_eq!(
            region.w as u16 % net_char_w,
            0,
            "Sheet region width must be a multiple of character width"
        );
        assert_eq!(
            region.h as u16 % net_char_h,
            0,
            "Sheet region height must be a multiple of character height"
        );
        assert!(region.w as u16 >= chars_per_row as u16 * net_char_w);
        assert!(region.h as u16 >= rows * net_char_h);
        Self {
            region,
            char_w,
            char_h,
            padding_x,
            padding_y,
            start_char,
            end_char,
        }
    }
    /// Returns a `BitFont` which is the same in every way except that a different colormod is used.
    pub fn colormod(&self, cmod: [u8; 4]) -> Self {
        let mut copy = *self;
        copy.region.colormod = cmod;
        copy
    }
    /// Draws the given `text` as a single line of characters of height `char_height`.
    /// The given position is the top-left corner of the rendered string.
    /// Panics if any character in text is not within the font's character range.
    /// Returns the bottom right corner of the rendered string and how many sprites were used.
    pub fn draw_text(
        &self,
        trfs: &mut [crate::sprites::Transform],
        uvs: &mut [crate::sprites::SheetRegion],
        text: &str,
        mut screen_pos: [f32; 2],
        depth: u16,
        char_height: f32,
    ) -> ([f32; 2], usize) {
        let start_char: u32 = self.start_char;
        trfs[0..text.len()].fill(Transform::ZERO);
        uvs[0..text.len()].fill(SheetRegion::ZERO);
        let chars_per_row = self.region.w as u16 / (self.char_w + self.padding_x);
        let aspect = self.char_w as f32 / self.char_h as f32;
        let char_width = aspect * char_height;
        screen_pos[0] += char_width / 2.0;
        screen_pos[1] -= char_height / 2.0;
        let mut used = 0;
        for (chara, (trf, uv)) in text.chars().zip(trfs.iter_mut().zip(uvs.iter_mut())) {
            // we'll collapse all whitespace into one space
            if chara.is_whitespace() {
                screen_pos[0] += char_width;
            }
            if !(self.start_char..self.end_char).contains(&u32::from(chara)) {
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
                self.region.x + (which_col as u16) * (self.char_w + self.padding_x),
                self.region.y + (which_row as u16) * (self.char_h + self.padding_y),
                depth,
                self.char_w as i16,
                self.char_h as i16,
            )
            .with_colormod(self.region.colormod);
            used += 1;
            screen_pos[0] += char_width;
        }
        (
            [
                screen_pos[0] + char_width / 2.0,
                screen_pos[1] + char_height / 2.0,
            ],
            used,
        )
    }
}
