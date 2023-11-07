use engine_ecs as engine;
use engine_ecs::{components::*, geom::*, Camera, SheetRegion};
use hecs::Entity;
use rand::Rng;
type Engine = engine::Engine<Game>;

// Components (markers)
struct Apple();
struct Guy();

// Bundles
#[derive(hecs::Bundle)]
struct WallBundle(Sprite, Transform, Solid, BoxCollision);
#[derive(hecs::Bundle)]
struct GuyBundle(Sprite, Transform, Pushable, BoxCollision, Physics, Guy);
#[derive(hecs::Bundle)]
struct AppleBundle(
    Sprite,
    Transform,
    SolidPushable,
    BoxCollision,
    Physics,
    Apple,
);
#[derive(hecs::Bundle)]
struct DecoBundle(Sprite, Transform);

const W: f32 = 320.0;
const H: f32 = 240.0;
const GUY_SPEED: f32 = 2.0;
const GUY_SIZE: Vec2 = Vec2 { x: 16.0, y: 16.0 };
const APPLE_SIZE: Vec2 = Vec2 { x: 16.0, y: 16.0 };
const APPLE_MAX: usize = 128;
const APPLE_INTERVAL: std::ops::Range<u32> = 1..10;
const WALL_UVS: SheetRegion = SheetRegion::new(0, 0, 480, 12, 8, 8);
const APPLE_SPEED_RANGE: std::ops::Range<f32> = (-2.0)..(-0.5);

struct Game {
    apple_timer: u32,
    score: u32,
    guy: Entity,
    spritesheet: engine::Spritesheet,
    font: engine::BitFont,
}

impl engine::Game for Game {
    const DT: f32 = 1.0 / 120.0;
    fn new(engine: &mut Engine) -> Self {
        engine.set_camera(Camera {
            screen_pos: [0.0, 0.0],
            screen_size: [W, H],
        });
        #[cfg(target_arch = "wasm32")]
        let sprite_img = {
            let img_bytes = include_bytes!("content/demo.png");
            image::load_from_memory_with_format(&img_bytes, image::ImageFormat::Png)
                .map_err(|e| e.to_string())
                .unwrap()
                .into_rgba8()
        };
        #[cfg(not(target_arch = "wasm32"))]
        let sprite_img = image::open("content/demo.png").unwrap().into_rgba8();
        let spritesheet = engine.add_spritesheet(&[&sprite_img], Some("demo spritesheet"));
        engine.spawn(DecoBundle(
            Sprite(spritesheet, SheetRegion::new(0, 0, 0, 16, 640, 480)),
            Transform {
                x: W / 2.0,
                y: H / 2.0,
                w: W as u16,
                h: H as u16,
                rot: 0.0,
            },
        ));
        let guy = engine.spawn(GuyBundle(
            Sprite(spritesheet, SheetRegion::new(0, 16, 480, 8, 16, 16)),
            Transform {
                x: W / 2.0,
                y: 24.0,
                w: GUY_SIZE.x as u16,
                h: GUY_SIZE.y as u16,
                rot: 0.0,
            },
            Pushable::default(),
            BoxCollision(AABB {
                center: Vec2::ZERO,
                size: GUY_SIZE,
            }),
            Physics { vel: Vec2::ZERO },
            Guy(),
        ));
        // floor
        make_wall(spritesheet, engine, W / 2.0, 8.0, W, 16.0);
        // left wall
        make_wall(spritesheet, engine, 8.0, H / 2.0, 16.0, H);
        // right wall
        make_wall(spritesheet, engine, W - 8.0, H / 2.0, 16.0, H);
        let font = engine.make_font(
            spritesheet,
            '0'..='9',
            SheetRegion::new(0, 0, 512, 0, 80, 8),
            10,
        );
        Game {
            apple_timer: 0,
            score: 0,
            font,
            spritesheet,
            guy,
        }
    }
    fn update(&mut self, engine: &mut Engine) {
        if engine.frame_number() % 600 == 0 {
            println!(
                "{:.6} : {:.6} --- {:.6} : {:.6} --- {:.6} : {:.6} --- {:.6}",
                engine.avg_sim_time(),
                engine.max_sim_time(),
                engine.avg_render_time(),
                engine.max_render_time(),
                engine.avg_net_time(),
                engine.max_net_time(),
                Self::DT
            );
        }
        let dir = engine.input.key_axis(engine::Key::Left, engine::Key::Right);
        engine
            .world()
            .query_one::<&mut Physics>(self.guy)
            .unwrap()
            .get()
            .unwrap()
            .vel = Vec2 {
            x: dir * GUY_SPEED,
            y: 0.0,
        };
        let mut rng = rand::thread_rng();
        let mut apple_count = 0;
        let mut to_remove = vec![];
        for (apple, (_, trf)) in engine.world().query::<(&Apple, &Transform)>().iter() {
            if trf.y < -8.0 {
                to_remove.push(apple);
            } else {
                apple_count += 1;
            }
        }
        for apple in to_remove {
            engine.despawn(apple).unwrap();
        }
        if self.apple_timer > 0 {
            self.apple_timer -= 1;
        } else if apple_count < APPLE_MAX {
            let _apple = engine.spawn(AppleBundle(
                Sprite(self.spritesheet, SheetRegion::new(0, 0, 496, 4, 16, 16)),
                Transform {
                    x: rng.gen_range(8.0..(W - 8.0)),
                    y: H + 8.0,
                    w: APPLE_SIZE.x as u16,
                    h: APPLE_SIZE.y as u16,
                    rot: 0.0,
                },
                SolidPushable::default(),
                BoxCollision(AABB {
                    center: Vec2::ZERO,
                    size: APPLE_SIZE,
                }),
                Physics {
                    vel: Vec2 {
                        x: 0.0,
                        y: rng.gen_range(APPLE_SPEED_RANGE),
                    },
                },
                Apple(),
            ));
            self.apple_timer = rng.gen_range(APPLE_INTERVAL);
        }
    }
    fn handle_collisions(
        &mut self,
        engine: &mut Engine,
        _contacts: impl Iterator<Item = engine::Contact>,
        triggers: impl Iterator<Item = engine::Contact>,
    ) {
        for engine::Contact(thing_a, thing_b, _amt) in triggers {
            let ent_a = engine.world().entity(thing_a).unwrap();
            let ent_b = engine.world().entity(thing_b).unwrap();
            if ent_a.has::<Apple>() && ent_b.has::<Guy>() {
                engine.despawn(thing_a).unwrap();
                self.score += 1;
            }
        }
    }
    fn render(&mut self, engine: &mut Engine) {
        engine.draw_string(
            &self.font,
            self.score.to_string(),
            Vec2 {
                x: 16.0,
                y: H - 16.0,
            },
            16.0,
        );
    }
}
fn main() {
    Engine::new(winit::window::WindowBuilder::new()).run();
}

fn make_wall(
    spritesheet: engine_ecs::Spritesheet,
    engine: &mut Engine,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> Entity {
    engine.spawn(WallBundle(
        Sprite(spritesheet, WALL_UVS),
        Transform {
            x,
            y,
            w: w as u16,
            h: h as u16,
            rot: 0.0,
        },
        Solid::default(),
        BoxCollision(AABB {
            center: Vec2::ZERO,
            size: Vec2 { x: w, y: h },
        }),
    ))
}
