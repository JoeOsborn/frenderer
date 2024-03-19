use crate::geom::*;
use crate::grid::{self, Grid};
use crate::TILE_SZ;
use engine_ecs_app::frenderer::{
    sprites::{SheetRegion, Transform},
    Immediate,
};
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Clone, Copy, Debug)]
struct TileData {
    solid: bool,
    sheet_region: SheetRegion,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntityType {
    Player,
    Enemy,
    // which level, grid x in dest level, grid y in dest level
    #[allow(dead_code)]
    Door(String, u16, u16),
}

#[allow(dead_code)]
pub struct Level {
    name: String,
    bg: SheetRegion,
    grid: Grid<u8>,
    tileset: Tileset,
    starts: Vec<(EntityType, Vec2)>,
}

impl Level {
    /*
    We'll read from an ad hoc format like this, where FLAGS is either S (solid) or O (open) but could be other stuff later:

    LEVELNAME W H
    ====
    SYM FLAGS X Y W H
    SYM FLAGS X Y W H
    SYM FLAGS X Y W H
    ====
    SYM SYM SYM SYM SYM
    SYM SYM SYM SYM SYM
    SYM SYM SYM SYM SYM
    SYM SYM SYM SYM SYM
    SYM SYM SYM SYM SYM
    ====
    player X Y
    enemy X Y
    enemy X Y
    enemy X Y
    door LEVELNAME TO-X TO-Y X Y
    you can add more types of thing if you want
    */
    pub fn from_str(s: &str) -> Self {
        enum State {
            Metadata,
            Legend,
            Map,
            Starts,
            Done,
        }
        impl State {
            fn next(self) -> Self {
                match self {
                    Self::Metadata => Self::Legend,
                    Self::Legend => Self::Map,
                    Self::Map => Self::Starts,
                    Self::Starts => Self::Done,
                    Self::Done => Self::Done,
                }
            }
        }
        let mut state = State::Metadata;
        let mut name = None;
        let mut dims = None;
        let mut legend: HashMap<String, (u8, TileData)> = std::collections::HashMap::new();
        let mut grid = vec![];
        let mut starts = vec![];
        let mut bg = SheetRegion::ZERO;
        for line in s.lines() {
            if line.is_empty() {
                continue;
            } else if line.chars().all(|c| c == '=') {
                state = state.next();
            } else {
                match state {
                    State::Metadata => {
                        let mut chunks = line.split_whitespace();
                        let md = chunks
                            .next()
                            .expect("No metadata decl string in metadata line {line}");
                        if md == "bg" {
                            if bg.w != 0 {
                                panic!("Two bg entries in metadata");
                            }
                            bg = SheetRegion::rect(
                                u16::from_str(chunks.next().expect("No x in metadata line {line}"))
                                    .expect("Couldn't parse x as u16 in {line}"),
                                u16::from_str(chunks.next().expect("No y in metadata line {line}"))
                                    .expect("Couldn't parse y as u16 in {line}"),
                                i16::from_str(
                                    chunks.next().expect("No width in metadata line {line}"),
                                )
                                .expect("Couldn't parse width as i16 in {line}"),
                                i16::from_str(
                                    chunks.next().expect("No height in metadata line {line}"),
                                )
                                .expect("Couldn't parse height as i16 in {line}"),
                            )
                            .with_depth(u16::MAX - 1);
                        } else {
                            if name.is_some() {
                                panic!("Two name entries in metadata");
                            }
                            name = Some(md.to_string());
                            dims = Some((
                                u16::from_str(
                                    chunks.next().expect("No width in metadata line {line}"),
                                )
                                .expect("Couldn't parse width as u16 in {line}"),
                                u16::from_str(
                                    chunks.next().expect("No height in metadata line {line}"),
                                )
                                .expect("Couldn't parse height as u16 in {line}"),
                            ));
                        }
                    }
                    State::Legend => {
                        let mut chunks = line.split_whitespace();
                        let sym = chunks.next().expect("Couldn't get tile symbol in {line}");
                        assert!(!legend.contains_key(sym), "Symbol {sym} already in legend");
                        let flags = chunks
                            .next()
                            .expect("Couldn't get tile flags in {line}")
                            .to_lowercase();
                        assert!(flags == "o" || flags == "s", "The only valid flags are o(pen) or s(olid), but you could parse other kinds here in {line}");
                        let x =
                            u16::from_str(chunks.next().expect("No sheet x in legend line {line}"))
                                .expect("Couldn't parse sheet x as u16 in {line}");
                        let y =
                            u16::from_str(chunks.next().expect("No sheet y in legend line {line}"))
                                .expect("Couldn't parse sheet y as u16 in {line}");
                        let w =
                            i16::from_str(chunks.next().expect("No sheet w in legend line {line}"))
                                .expect("Couldn't parse sheet w as i16 in {line}");
                        let h =
                            i16::from_str(chunks.next().expect("No sheet h in legend line {line}"))
                                .expect("Couldn't parse sheet h as i16 in {line}");
                        let data = TileData {
                            solid: flags == "s",
                            sheet_region: SheetRegion::new(0, x, y, 16, w, h),
                        };
                        legend.insert(sym.to_string(), (legend.len() as u8, data));
                    }
                    State::Map => {
                        let old_len = grid.len();
                        grid.extend(line.split_whitespace().map(|sym| legend[sym].0));
                        assert_eq!(
                            old_len + dims.unwrap().0 as usize,
                            grid.len(),
                            "map line is too short: {line} for map dims {dims:?}"
                        );
                    }
                    State::Starts => {
                        let mut chunks = line.split_whitespace();
                        let etype = chunks
                            .next()
                            .expect("Couldn't get entity start type {line}");
                        let etype = match etype {
                            "player" => EntityType::Player,
                            "enemy" => EntityType::Enemy,
                            "door" => {
                                let to_room = chunks.next().expect("Couldn't get dest room {line}");
                                let to_x = u16::from_str(
                                    chunks.next().expect("No dest x coord in door line {line}"),
                                )
                                .expect("Couldn't parse x coord as u16 in {line}");
                                let to_y = u16::from_str(
                                    chunks.next().expect("No dest y coord in door line {line}"),
                                )
                                .expect("Couldn't parse y coord as u16 in {line}");
                                EntityType::Door(to_room.to_string(), to_x, to_y)
                            }
                            _ => panic!("Unrecognized entity type in {line}"),
                        };
                        let x =
                            u16::from_str(chunks.next().expect("No x coord in start line {line}"))
                                .expect("Couldn't parse x coord as u16 in {line}");
                        let y =
                            u16::from_str(chunks.next().expect("No y coord in start line {line}"))
                                .expect("Couldn't parse y coord as u16 in {line}");
                        starts.push((
                            etype,
                            Vec2 {
                                x: (x as usize * TILE_SZ) as f32 + TILE_SZ as f32 / 2.0,
                                y: ((dims.unwrap().1 - y) as usize * TILE_SZ) as f32
                                    - TILE_SZ as f32 / 2.0,
                            },
                        ));
                    }
                    State::Done => {
                        panic!("Unexpected file content after parsing finished in {line}")
                    }
                }
            }
        }
        assert_ne!(name, None);
        let name = name.unwrap();
        assert_ne!(dims, None);
        let (w, h) = dims.unwrap();
        assert!(!legend.is_empty());
        assert_eq!(grid.len(), w as usize * h as usize);
        let mut tiles: Vec<(u8, TileData)> = legend.into_values().collect();
        tiles.sort_by_key(|(num, _)| *num);
        Self {
            bg,
            name: name.to_string(),
            grid: Grid::new(w as usize, h as usize, grid),
            tileset: Tileset {
                tiles: tiles.into_iter().map(|(_num, val)| val).collect(),
            },
            starts,
        }
    }
    pub fn sprite_count(&self) -> usize {
        self.grid.width() * self.grid.height() + 1
    }
    pub fn render_immediate(&self, frend: &mut Immediate) -> usize {
        let len = self.sprite_count();
        let (trfs, uvs) = frend.draw_sprites(0, len);
        self.render_into(trfs, uvs)
    }
    pub fn render_into(&self, trfs: &mut [Transform], uvs: &mut [SheetRegion]) -> usize {
        let w = self.grid.width();
        let h = self.grid.height();
        assert_eq!(trfs.len(), uvs.len());
        assert_eq!(trfs.len(), w * h + 1);
        for ((y, row), (trfs, uvs)) in self
            .grid
            .row_iter()
            .enumerate()
            .zip(trfs.chunks_exact_mut(w).zip(uvs.chunks_exact_mut(w)))
        {
            for ((x, tile), (trf, uv)) in row
                .iter()
                .enumerate()
                .zip(trfs.iter_mut().zip(uvs.iter_mut()))
            {
                // NOTE: we're converting from grid coordinates to "sprite center coordinates", so we have to flip y...
                let y = h - y - 1;
                *trf = Transform {
                    // and multiply by tile sz *and* offset by half tile sz
                    x: (x * TILE_SZ + TILE_SZ / 2) as f32,
                    y: (y * TILE_SZ + TILE_SZ / 2) as f32,
                    w: TILE_SZ as u16,
                    h: TILE_SZ as u16,
                    rot: 0.0,
                };
                *uv = self.tileset[*tile as usize].sheet_region;
            }
        }
        if self.bg.w != 0 {
            trfs[trfs.len() - 1] = Transform {
                x: (self.grid.width() * TILE_SZ) as f32 / 2.0,
                y: (self.grid.height() * TILE_SZ) as f32 / 2.0,
                w: (self.grid.width() as u16 * TILE_SZ as u16),
                h: (self.grid.height() as u16 * TILE_SZ as u16),
                rot: 0.0,
            };
            uvs[uvs.len() - 1] = self.bg;
        }
        w * h + 1
    }
    #[allow(dead_code)]
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn starts(&self) -> &[(EntityType, Vec2)] {
        &self.starts
    }
    pub fn get_tile_at(&self, pos: Vec2) -> Option<&TileData> {
        let (gx, gy) = self.world_to_grid(pos);
        self.grid.get(gx, gy).map(|t| &self.tileset[*t as usize])
    }
    pub fn tile_index_at(&self, pos: Vec2) -> Option<usize> {
        let (gx, gy) = self.world_to_grid(pos);
        self.grid.xy_to_index(gx, gy)
    }
    pub fn grid_to_world(&self, pos: grid::Coord) -> Vec2 {
        Vec2 {
            x: pos.0 as f32 * TILE_SZ as f32,
            y: (self.grid.height() - pos.1 - 1) as f32 * TILE_SZ as f32,
        }
    }
    pub fn world_to_grid(&self, pos: Vec2) -> grid::Coord {
        (
            (pos.x / TILE_SZ as f32) as usize,
            (((self.grid.height() as f32 * TILE_SZ as f32) - pos.y - 1.0) / TILE_SZ as f32)
                as usize,
        )
    }
    pub fn tiles_within(&self, rect: Rect) -> impl Iterator<Item = (Rect, &TileData)> {
        let (l, t) = self.world_to_grid(Vec2 {
            x: rect.x,
            y: rect.y,
        });
        let (r, b) = self.world_to_grid(Vec2 {
            x: rect.x + rect.w as f32,
            y: rect.y + rect.h as f32,
        });
        ((b.max(1) - 1)..(t + 2)).flat_map(move |row| {
            ((l.max(1) - 1)..(r + 2)).filter_map(move |col| {
                self.grid.get(col, row).map(|tile_dat| {
                    let world = self.grid_to_world((col, row));
                    (
                        Rect {
                            x: world.x,
                            y: world.y,
                            w: TILE_SZ as u16,
                            h: TILE_SZ as u16,
                        },
                        &self.tileset[*tile_dat as usize],
                    )
                })
            })
        })
    }
    pub fn width(&self) -> usize {
        self.grid.width()
    }
    pub fn height(&self) -> usize {
        self.grid.height()
    }
}

#[derive(Debug)]
struct Tileset {
    tiles: Vec<TileData>,
}
impl std::ops::Index<usize> for Tileset {
    type Output = TileData;
    fn index(&self, index: usize) -> &Self::Output {
        &self.tiles[index]
    }
}
