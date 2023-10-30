use engine_charas as engine;
use engine_charas::{geom::*, Camera};
use rand::Rng;
type Engine = engine::Engine<Game>;

const W: f32 = 320.0;
const H: f32 = 240.0;
const GUY_SPEED: f32 = 4.0;
const GUY_SIZE: Vec2 = Vec2 { x: 16.0, y: 16.0 };
const APPLE_SIZE: Vec2 = Vec2 { x: 16.0, y: 16.0 };

const WALL_UVS: Rect = Rect {
    corner: Vec2 {
        x: 0.0,
        y: 480.0 / 1024.0,
    },
    size: Vec2 {
        x: 8.0 / 1024.0,
        y: 8.0 / 1024.0,
    },
};
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum CharaTag {
    Wall,
    Guy,
    Apple,
    Deco,
}

impl engine::TagType for CharaTag {}

struct Game {
    apple_timer: u32,
    score: u32,
    guy: engine::CharaID,
    spritesheet: engine::Spritesheet,
    font: engine::BitFont,
}

impl engine::Game for Game {
    type Tag = CharaTag;
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
        let spritesheet = engine.add_spritesheet(sprite_img, Some("demo spritesheet"));
        engine.make_chara(
            spritesheet,
            CharaTag::Deco,
            AABB {
                center: Vec2 {
                    x: W / 2.0,
                    y: H / 2.0,
                },
                size: Vec2 { x: W, y: H },
            },
            Rect {
                corner: Vec2 { x: 0.0, y: 0.0 },
                size: Vec2 {
                    x: 640.0 / 1024.0,
                    y: 480.0 / 1024.0,
                },
            },
            engine::Collision::none(),
        );
        let guy = engine.make_chara(
            spritesheet,
            CharaTag::Guy,
            AABB {
                center: Vec2 {
                    x: W / 2.0,
                    y: 24.0,
                },
                size: GUY_SIZE,
            },
            Rect {
                corner: Vec2 {
                    x: 16.0 / 1024.0,
                    y: 480.0 / 1024.0,
                },
                size: Vec2 {
                    x: 16.0 / 1024.0,
                    y: 16.0 / 1024.0,
                },
            },
            engine::Collision::movable(),
        );
        // floor
        engine.make_chara(
            spritesheet,
            CharaTag::Wall,
            AABB {
                center: Vec2 { x: W / 2.0, y: 8.0 },
                size: Vec2 { x: W, y: 16.0 },
            },
            WALL_UVS,
            engine::Collision::solid(),
        );
        // left wall
        engine.make_chara(
            spritesheet,
            CharaTag::Wall,
            AABB {
                center: Vec2 { x: 8.0, y: H / 2.0 },
                size: Vec2 { x: 16.0, y: H },
            },
            WALL_UVS,
            engine::Collision::solid(),
        );
        // right wall
        engine.make_chara(
            spritesheet,
            CharaTag::Wall,
            AABB {
                center: Vec2 {
                    x: W - 8.0,
                    y: H / 2.0,
                },
                size: Vec2 { x: 16.0, y: H },
            },
            WALL_UVS,
            engine::Collision::solid(),
        );
        let font = engine.make_font(
            spritesheet,
            '0'..='9',
            Rect {
                corner: Vec2 {
                    x: 0.0,
                    y: 512.0 / 1024.0,
                },
                size: Vec2 {
                    x: 80.0 / 1024.0,
                    y: 8.0 / 1024.0,
                },
            },
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
        let dir = engine.input.key_axis(engine::Key::Left, engine::Key::Right);
        engine.chara_mut(self.guy).set_vel(Vec2 {
            x: dir * GUY_SPEED,
            y: 0.0,
        });
        let mut rng = rand::thread_rng();
        if self.apple_timer > 0 {
            self.apple_timer -= 1;
        } else if engine.charas_by_tag(CharaTag::Apple).count() < 8 {
            let apple = engine.recycle_chara(
                self.spritesheet,
                CharaTag::Apple,
                AABB {
                    center: Vec2 {
                        x: rng.gen_range(8.0..(W - 8.0)),
                        y: H + 8.0,
                    },
                    size: APPLE_SIZE,
                },
                Rect {
                    corner: Vec2 {
                        x: 0.0,
                        y: 496.0 / 1024.0,
                    },
                    size: Vec2 {
                        x: 16.0 / 1024.0,
                        y: 16.0 / 1024.0,
                    },
                },
                engine::Collision::trigger(),
            );
            engine.chara_mut(apple).set_vel(Vec2 {
                x: 0.0,
                y: rng.gen_range((-4.0)..(-1.0)),
            });
            self.apple_timer = rng.gen_range(30..90);
        }
        let mut to_kill = vec![];
        for (id, chara) in engine.charas_by_tag_mut(CharaTag::Apple) {
            if chara.pos().y < -8.0 {
                to_kill.push(id);
            }
        }
        to_kill.into_iter().for_each(|k| engine.kill_chara(k));
    }
    fn handle_collisions(
        &mut self,
        _engine: &mut Engine,
        _contacts: impl Iterator<Item = engine::Contact<CharaTag>>,
    ) {
        // do nothing
    }
    fn handle_triggers(
        &mut self,
        engine: &mut Engine,
        triggers: impl Iterator<Item = engine::Contact<CharaTag>>,
    ) {
        for engine::Contact(thing_a, tag_a, thing_b, tag_b, amt) in triggers {
            match (tag_a, tag_b) {
                (CharaTag::Guy, CharaTag::Apple) => {
                    engine.kill_chara(thing_b);
                    self.score += 1;
                }
                // Apple, Guy will never happen because of the ordering of Guy and Apple in the enum
                _ => (),
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
