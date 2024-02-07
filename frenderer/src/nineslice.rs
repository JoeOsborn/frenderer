use crate::sprites::{SheetRegion, Transform};

/// A repeat mode for 9-slice centers and edges.  [`Repeat::Stretch`]
/// will scale one sprite to fill the space, while [`Repeat::Tile`]
/// will only fill exact multiples of the slice's size (i.e., it may not
/// fill the entire space if the space is not a multiple of the size).
#[derive(Clone, Copy, Debug)]
pub enum Repeat {
    Stretch,
    Tile,
}

/// Describes a slice that can be stretched or tiled.  In stretching mode, w and h are minimum sizes.
#[derive(Clone, Copy, Debug)]
pub struct Slice {
    pub w: f32,
    pub h: f32,
    pub region: SheetRegion,
    pub repeat: Repeat,
}

/// Describes a slice that decorates a box, always drawn at the given native size.
#[derive(Clone, Copy, Debug)]
pub struct CornerSlice {
    pub w: f32,
    pub h: f32,
    pub region: SheetRegion,
}

/// A nine-slice sized box drawing helper built for use with SpriteRenderer.  It is the caller's responsibility to set the depth on the slices' sheet regions to achieve the desired rendering effects.
#[derive(Clone, Debug)]
pub struct NineSlice {
    top_left: CornerSlice,
    top_right: CornerSlice,
    bottom_left: CornerSlice,
    bottom_right: CornerSlice,
    top: Slice,
    left: Slice,
    right: Slice,
    bottom: Slice,
    center: Slice,
}

impl NineSlice {
    pub fn with_corner_edge_center(
        corner_top_left: CornerSlice,
        edge_left: Slice,
        edge_top: Slice,
        center: Slice,
    ) -> Self {
        Self::new(
            corner_top_left,
            CornerSlice {
                w: corner_top_left.h,
                h: corner_top_left.w,
                region: corner_top_left.region.flip_horizontal(),
            },
            CornerSlice {
                region: corner_top_left.region.flip_vertical(),
                w: corner_top_left.h,
                h: corner_top_left.w,
            },
            CornerSlice {
                region: corner_top_left.region.flip_vertical().flip_horizontal(),
                ..corner_top_left
            },
            edge_top,
            Slice {
                region: edge_top.region.flip_vertical(),
                ..edge_top
            },
            edge_left,
            Slice {
                region: edge_left.region.flip_horizontal(),
                ..edge_left
            },
            center,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        top_left: CornerSlice,
        top_right: CornerSlice,
        bottom_left: CornerSlice,
        bottom_right: CornerSlice,
        top: Slice,
        bottom: Slice,
        left: Slice,
        right: Slice,
        center: Slice,
    ) -> Self {
        assert_eq!(top_left.w, bottom_left.w);
        assert_eq!(top_right.w, bottom_right.w);
        assert_eq!(top_left.h, top_right.h);
        assert_eq!(bottom_left.h, bottom_right.h);
        Self {
            top_left,
            top_right,
            bottom_left,
            bottom_right,
            top,
            left,
            right,
            bottom,
            center,
        }
    }
    /// Returns how many sprites will be needed to render this nineslice box at the given width and height.
    /// This may be an overestimate if the box is very small, but nineslice will zero out any sprites it doesn't use.
    pub fn sprite_count(&self, w: f32, h: f32) -> usize {
        let mut count: usize = 4; // 4 corners
        for edge in &[self.left, self.right] {
            count += match edge.repeat {
                Repeat::Stretch => 1,
                Repeat::Tile => (h / edge.h) as usize,
            };
        }
        for edge in &[self.top, self.bottom] {
            count += match edge.repeat {
                Repeat::Stretch => 1,
                Repeat::Tile => (w / edge.w) as usize,
            };
        }
        count += match self.center.repeat {
            Repeat::Stretch => 1,
            Repeat::Tile => ((w / self.center.w) as usize) * ((h / self.center.h) as usize),
        };
        count
    }
    /// Draws a nineslice box from (x,y) (bottom left corner) to (x+w, y+h) (top right corner)
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        trf: &mut [Transform],
        uvs: &mut [SheetRegion],
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        z_offset: u16,
    ) -> usize {
        let mut which = 0;
        let limit = self.sprite_count(w, h);
        // draw center
        {
            let x0 = x + self.top_left.w;
            let x1 = x + w - self.top_right.w;
            let w = x1 - x0;
            let y0 = y + self.bottom_left.h;
            let y1 = y + h - self.top_left.h;
            let h = y1 - y0;
            match self.center.repeat {
                Repeat::Stretch => {
                    trf[which] = Transform {
                        w: w as u16,
                        h: h as u16,
                        x: x0 + w / 2.0,
                        y: y0 + h / 2.0,
                        rot: 0.0,
                    };
                    uvs[which] = self.center.region;
                    uvs[which].depth += z_offset;
                    which += 1;
                }
                Repeat::Tile => {
                    for row in 0..((h / self.center.h) as usize) {
                        for (col, (trf, uv)) in (0..(w / self.center.w) as usize)
                            .zip(trf[which..].iter_mut().zip(uvs[which..].iter_mut()))
                        {
                            *trf = Transform {
                                w: self.center.w as u16,
                                h: self.center.h as u16,
                                rot: 0.0,
                                x: x0 + (col as f32 * self.center.w) + (self.center.w / 2.0),
                                y: y0 + (row as f32 * self.center.h) + (self.center.h / 2.0),
                            };
                            *uv = self.center.region;
                            uv.depth += z_offset;
                            which += 1;
                        }
                    }
                }
            }
        }
        // draw edges
        for (edge, xpos) in &[
            (self.left, x + self.left.w / 2.0),
            (self.right, x + w - self.right.w / 2.0),
        ] {
            let y = y + self.bottom_left.h;
            match edge.repeat {
                Repeat::Stretch => {
                    let w = edge.w;
                    let h = (h - self.bottom_left.h - self.top_left.h).max(edge.h);
                    trf[which] = Transform {
                        w: w as u16,
                        h: h as u16,
                        x: *xpos,
                        y: y + h / 2.0,
                        rot: 0.0,
                    };
                    uvs[which] = edge.region;
                    uvs[which].depth += z_offset;
                    which += 1;
                }
                Repeat::Tile => {
                    let h = h - self.bottom_left.h - self.top_left.h;
                    for (row, (trf, uv)) in (0..((h / edge.h) as usize))
                        .zip(trf[which..].iter_mut().zip(uvs[which..].iter_mut()))
                    {
                        *trf = Transform {
                            w: edge.w as u16,
                            h: edge.h as u16,
                            rot: 0.0,
                            x: *xpos,
                            y: y + (row as f32 * edge.h) + (edge.h / 2.0),
                        };
                        *uv = edge.region;
                        uv.depth += z_offset;
                        which += 1;
                    }
                }
            };
        }

        for (edge, ypos) in &[
            (self.bottom, y + self.bottom.h / 2.0),
            (self.top, y + h - self.bottom.h / 2.0),
        ] {
            let x = x + self.top_left.w;
            match edge.repeat {
                Repeat::Stretch => {
                    let w = (w - self.top_left.w - self.top_right.w).max(edge.w);
                    let h = edge.h;
                    trf[which] = Transform {
                        w: w as u16,
                        h: h as u16,
                        y: *ypos,
                        x: x + w / 2.0,
                        rot: 0.0,
                    };
                    uvs[which] = edge.region;
                    uvs[which].depth += z_offset;
                    which += 1;
                }
                Repeat::Tile => {
                    let w = w - self.top_left.w - self.top_right.w;
                    for (col, (trf, uv)) in (0..((w / edge.w) as usize))
                        .zip(trf[which..].iter_mut().zip(uvs[which..].iter_mut()))
                    {
                        *trf = Transform {
                            w: edge.w as u16,
                            h: edge.h as u16,
                            rot: 0.0,
                            y: *ypos,
                            x: x + (col as f32 * edge.w) + (edge.w / 2.0),
                        };
                        *uv = edge.region;
                        uv.depth += z_offset;
                        which += 1;
                    }
                }
            };
        }
        // draw corners
        for ((corner, x, y), (trf, uv)) in [
            (
                self.top_left,
                x + self.top_left.w / 2.0,
                y + h - self.top_left.h / 2.0,
            ),
            (
                self.top_right,
                x + w - self.top_right.w / 2.0,
                y + h - self.top_right.h / 2.0,
            ),
            (
                self.bottom_left,
                x + self.bottom_left.w / 2.0,
                y + self.bottom_left.h / 2.0,
            ),
            (
                self.bottom_right,
                x + w - self.bottom_right.w / 2.0,
                y + self.bottom_right.h / 2.0,
            ),
        ]
        .iter()
        .zip(trf[which..].iter_mut().zip(uvs[which..].iter_mut()))
        {
            *trf = Transform {
                x: *x,
                y: *y,
                w: corner.w as u16,
                h: corner.h as u16,
                rot: 0.0,
            };
            *uv = corner.region;
            uv.depth += z_offset;
        }
        which += 4;

        trf[which..limit].fill(Transform::ZERO);
        uvs[which..limit].fill(SheetRegion::ZERO);
        which
    }
}
