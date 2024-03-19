use assets_manager::asset::Png;

use frenderer::{
    input,
    sprites::{SheetRegion, Transform},
    wgpu,
};
use rand::{seq::SliceRandom, Rng};

const COUNT: usize = 100;
const W: f32 = 1024.0;
const H: f32 = 768.0;

fn init_data<S: assets_manager::source::Source>(
    frend: &mut frenderer::Immediate,
    cache: &assets_manager::AssetCache<S>,
) {
    let sprite_img_handle = cache.load::<Png>("king").expect("Couldn't load king img");
    let sprite_img = sprite_img_handle.read().0.to_rgba8();

    let sprite_invert_img_handle = cache
        .load::<Png>("king_invert")
        .expect("Couldn't load inverse king");
    let sprite_invert_img = sprite_invert_img_handle.read().0.to_rgba8();

    let sprite_tex = frend.create_array_texture(
        &[&sprite_img, &sprite_invert_img],
        wgpu::TextureFormat::Rgba8UnormSrgb,
        sprite_img.dimensions(),
        Some("spr-king.png"),
    );

    let mut rng = rand::thread_rng();
    frend.sprite_group_add(
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
        frenderer::sprites::Camera2D {
            screen_pos: [0.0, 0.0],
            screen_size: [W, H],
        },
    );
}

fn main() {
    let mut input = input::Input::default();

    #[cfg(not(target_arch = "wasm32"))]
    let source =
        assets_manager::source::FileSystem::new("content").expect("Couldn't load resources");
    #[cfg(target_arch = "wasm32")]
    let source = assets_manager::source::Embedded::from(assets_manager::source::embed!("content"));
    let cache = assets_manager::AssetCache::with_source(source);

    let drv = frenderer::Driver::new(
        winit::window::WindowBuilder::new()
            .with_title("test")
            .with_inner_size(winit::dpi::LogicalSize::new(W, H)),
        Some((W as u32, H as u32)),
    );
    let mut sprites: Vec<_> = {
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

    const DT: f32 = 1.0 / 60.0;
    let mut clock = frenderer::clock::Clock::new(DT, DT * 0.01, 8);
    drv.run_event_loop::<(), _>(
        move |win, frend| {
            let mut frend = frenderer::Immediate::new(frend);
            init_data(&mut frend, &cache);
            (win, frend)
        },
        move |event, target, (window, frend)| {
            use frenderer::{EventPhase, FrendererEvents};
            match frend.handle_event(&mut clock, window, &event, target, &mut input) {
                EventPhase::Run(steps) => {
                    let mut rng = rand::thread_rng();
                    for _step in 0..steps {
                        for (x, y, rot, _gfx) in sprites.iter_mut() {
                            *x += rng.gen_range((-1.0)..1.0);
                            *y += rng.gen_range((-1.0)..1.0);
                            *rot += rng.gen_range((-0.05)..0.05);
                        }
                        if rng.gen_bool(0.05) && !sprites.is_empty() {
                            sprites.swap_remove(rng.gen_range(0..sprites.len()));
                        }
                        if rng.gen_bool(0.01) {
                            sprites.push((
                                rng.gen_range(0.0..(W - 16.0)),
                                rng.gen_range(0.0..(H - 16.0)),
                                rng.gen_range(0.0..std::f32::consts::TAU),
                                SheetRegion::new(rng.gen_range(0..2), 0, 16, 8, 11, 16)
                                    .with_colormod([
                                        rng.gen(),
                                        rng.gen(),
                                        rng.gen(),
                                        *[0, 128, 255].choose(&mut rng).unwrap(),
                                    ]),
                            ));
                        }
                    }
                    for (x, y, rot, uv) in sprites.iter() {
                        frend.draw_sprite(
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
                    // ok now render.
                    {
                        let nine_stretched =
                            frenderer::nineslice::NineSlice::with_corner_edge_center(
                                frenderer::nineslice::CornerSlice {
                                    w: 8.0,
                                    h: 8.0,
                                    region: SheetRegion::rect(0, 0, 8, 8),
                                },
                                frenderer::nineslice::Slice {
                                    w: 2.0,
                                    h: 16.0,
                                    region: SheetRegion::rect(0, 0, 2, 16).with_depth(1),
                                    repeat: frenderer::nineslice::Repeat::Stretch,
                                },
                                frenderer::nineslice::Slice {
                                    w: 16.0,
                                    h: 2.0,
                                    region: SheetRegion::rect(0, 0, 16, 2).with_depth(1),
                                    repeat: frenderer::nineslice::Repeat::Stretch,
                                },
                                frenderer::nineslice::Slice {
                                    w: 16.0,
                                    h: 16.0,
                                    region: SheetRegion::rect(16, 0, 16, 16).with_depth(2),
                                    repeat: frenderer::nineslice::Repeat::Stretch,
                                },
                            );
                        let nine_tiled = frenderer::nineslice::NineSlice::with_corner_edge_center(
                            frenderer::nineslice::CornerSlice {
                                w: 8.0,
                                h: 8.0,
                                region: SheetRegion::rect(0, 0, 8, 8),
                            },
                            frenderer::nineslice::Slice {
                                w: 2.0,
                                h: 16.0,
                                region: SheetRegion::rect(0, 0, 2, 16).with_depth(1),
                                repeat: frenderer::nineslice::Repeat::Tile,
                            },
                            frenderer::nineslice::Slice {
                                w: 16.0,
                                h: 2.0,
                                region: SheetRegion::rect(0, 0, 16, 2).with_depth(1),
                                repeat: frenderer::nineslice::Repeat::Tile,
                            },
                            frenderer::nineslice::Slice {
                                w: 16.0,
                                h: 16.0,
                                region: SheetRegion::rect(16, 0, 16, 16).with_depth(2),
                                repeat: frenderer::nineslice::Repeat::Tile,
                            },
                        );
                        frend.draw_nineslice(0, &nine_stretched, 10.0, 20.0, 160.0, 112.0, 0);
                        frend.draw_nineslice(0, &nine_tiled, 400.0, 500.0, 160.0, 112.0, 0);
                    }
                    frend.render();
                }
                EventPhase::Quit => {
                    target.exit();
                }
                EventPhase::Wait => {}
            }
        },
    )
    .unwrap();
}
