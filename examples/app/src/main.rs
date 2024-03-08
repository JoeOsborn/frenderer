use frapp::assets_manager::asset::Png;
use frapp::frenderer::input::Input;
use frapp::frenderer::sprites::{Camera2D, SheetRegion, Transform};
use frapp::frenderer::*;
use frapp::*;
use rand::{seq::SliceRandom, Rng};

const COUNT: usize = 100;
const W: f32 = 1024.0;
const H: f32 = 768.0;

struct TestApp {
    #[allow(dead_code)]
    assets: AssetCache,
    sprites: Vec<(f32, f32, f32, SheetRegion)>,
}
impl App for TestApp {
    type Renderer = Immediate;
    const DT: f32 = 1.0 / 60.0;
    fn new(renderer: &mut Self::Renderer, assets: AssetCache) -> Self {
        let sprite_img_handle = assets.load::<Png>("king").expect("Couldn't load king img");
        let sprite_img = sprite_img_handle.read().0.to_rgba8();

        let sprite_invert_img_handle = assets
            .load::<Png>("king_invert")
            .expect("Couldn't load inverse king");
        let sprite_invert_img = sprite_invert_img_handle.read().0.to_rgba8();

        let sprite_tex = renderer.create_array_texture(
            &[&sprite_img, &sprite_invert_img],
            wgpu::TextureFormat::Rgba8UnormSrgb,
            sprite_img.dimensions(),
            Some("spr-king.png"),
        );

        let mut rng = rand::thread_rng();
        renderer.sprite_group_add(
            &sprite_tex,
            (0..COUNT + 1_000)
                .map(|_n| Transform {
                    x: rng.gen_range(0.0..(W - 16.0)),
                    y: rng.gen_range(0.0..(H - 16.0)),
                    w: 11,
                    h: 16,
                    rot: rng.gen_range(0.0..(std::f32::consts::TAU)),
                })
                .collect(),
            (0..COUNT + 1_000)
                .map(|_n| {
                    SheetRegion::new(rng.gen_range(0..2), 0, 16, 8, 11, 16).with_colormod([
                        rng.gen(),
                        rng.gen(),
                        rng.gen(),
                        *[0, 128, 255].choose(&mut rng).unwrap(),
                    ])
                })
                .collect(),
            Camera2D {
                screen_pos: [0.0, 0.0],
                screen_size: [W, H],
            },
        );

        let sprites: Vec<_> = {
            let mut rng = rand::thread_rng();
            (0..COUNT)
                .map(|_| {
                    (
                        rng.gen_range(0.0..(W - 16.0)),
                        rng.gen_range(0.0..(H - 16.0)),
                        rng.gen_range(0.0..std::f32::consts::TAU),
                        SheetRegion::new(rng.gen_range(0..2), 0, 16, 8, 11, 16).with_colormod([
                            rng.gen(),
                            rng.gen(),
                            rng.gen(),
                            *[0, 128, 255].choose(&mut rng).unwrap(),
                        ]),
                    )
                })
                .collect()
        };
        Self { assets, sprites }
    }
    fn update(&mut self, _renderer: &mut Self::Renderer, _input: &mut Input) {
        let mut rng = rand::thread_rng();
        for (x, y, rot, _gfx) in self.sprites.iter_mut() {
            *x += rng.gen_range((-1.0)..1.0);
            *y += rng.gen_range((-1.0)..1.0);
            *rot += rng.gen_range((-0.05)..0.05);
        }
        if rng.gen_bool(0.05) && !self.sprites.is_empty() {
            self.sprites
                .swap_remove(rng.gen_range(0..self.sprites.len()));
        }
        if rng.gen_bool(0.01) {
            self.sprites.push((
                rng.gen_range(0.0..(W - 16.0)),
                rng.gen_range(0.0..(H - 16.0)),
                rng.gen_range(0.0..std::f32::consts::TAU),
                SheetRegion::new(rng.gen_range(0..2), 0, 16, 8, 11, 16).with_colormod([
                    rng.gen(),
                    rng.gen(),
                    rng.gen(),
                    *[0, 128, 255].choose(&mut rng).unwrap(),
                ]),
            ));
        }
    }
    fn render(&mut self, renderer: &mut Self::Renderer, _dt: f32) {
        for (x, y, rot, uv) in self.sprites.iter() {
            renderer.draw_sprite(
                0,
                Transform {
                    x: *x,
                    y: *y,
                    w: 44,
                    h: 64,
                    rot: *rot,
                },
                *uv,
            );
        }
    }
}

fn main() {
    app!(TestApp, "content").run(WindowBuilder::new(), Some((W as u32, H as u32)));
}
