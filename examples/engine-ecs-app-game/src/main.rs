use engine_ecs_app::geom::*;
use engine_ecs_app::rand::Rng;
use engine_ecs_app::*;
use frenderer::sprites::SheetRegion;

const MAX_HEALTH: u8 = 3;
const W: f32 = 320.0;
const H: f32 = 240.0;
const TILE_SZ: u16 = 16;
// pixels per second
const PLAYER_SPEED: f32 = 64.0;
const ENEMY_SPEED: f32 = 32.0;
const KNOCKBACK_SPEED: f32 = 128.0;

const ATTACK_MAX_TIME: f32 = 0.3;
const ATTACK_COOLDOWN_TIME: f32 = 0.1;
const KNOCKBACK_TIME: f32 = 0.25;

const PLAYER: [SheetRegion; 4] = [
    //n, e, s, w
    SheetRegion::rect(461 + 16 * 2, 39, 16, 16),
    SheetRegion::rect(461, 39, 16, 16),
    SheetRegion::rect(461 + 16 * 3, 39, 16, 16),
    SheetRegion::rect(461 + 16, 39, 16, 16),
];
const PLAYER_ATK: [SheetRegion; 4] = [
    //n, e, s, w
    SheetRegion::rect(428, 0, 16, 8), // offset by 8px in direction
    SheetRegion::rect(349, 22, 8, 16),
    SheetRegion::rect(162, 13, 16, 8),
    SheetRegion::rect(549, 17, 8, 16),
];
const ENEMY: [SheetRegion; 4] = [
    SheetRegion::rect(533 + 16 * 2, 39, 16, 16),
    SheetRegion::rect(533 + 16, 39, 16, 16),
    SheetRegion::rect(533, 39, 16, 16),
    SheetRegion::rect(533 + 16 * 3, 39, 16, 16),
];
const HEART: SheetRegion = SheetRegion::rect(525, 35, 8, 8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum Dir {
    N,
    E,
    S,
    W,
}
impl Dir {
    fn to_vec2(self) -> Vec2 {
        match self {
            Dir::N => Vec2 { x: 0.0, y: 1.0 },
            Dir::E => Vec2 { x: 1.0, y: 0.0 },
            Dir::S => Vec2 { x: 0.0, y: -1.0 },
            Dir::W => Vec2 { x: -1.0, y: 0.0 },
        }
    }
}

// components
use engine_ecs_app::components::*; // sprite, transform
                                   //struct Door(String, u16, u16);
struct Player {
    dir: Dir,
    attack_timer: f32,
    knockback_timer: f32,
    attack: Option<Entity>,
}
struct Enemy(Dir);
struct PlayerAttack(Dir);
struct Health(u8);
struct Heart(u8);

// bundles
//struct DoorBundle(Transform, Door);
#[derive(Bundle)]
struct PlayerBundle(Sprite, Transform, Physics, Player, Health, Body, Movable);
#[derive(Bundle)]
struct EnemyBundle(Sprite, Transform, Physics, Enemy, Body, Movable);
#[derive(Bundle)]
struct AttackBundle(Sprite, Transform, PlayerAttack, Trigger);
#[derive(Bundle)]
struct HeartBundle(Sprite, Transform, Heart);

struct MyGame {
    spritesheet: Spritesheet,
    player: Entity,
    level: Entity,
}
impl MyGame {
    fn enter_level(&mut self, start: Vec2, engine: &mut Engine) {
        let world = engine.world();
        // despawn all enemies
        let to_remove: Vec<_> = world
            .query_mut::<&Enemy>()
            .into_iter()
            .map(|(e, _)| e)
            .collect();
        for rem in to_remove {
            world.despawn(rem).unwrap();
        }
        let trf = world.query_one_mut::<&mut Transform>(self.player).unwrap();
        trf.x = start.x;
        trf.y = start.y;
        let starts = world.get::<&Level>(self.level).unwrap().starts().to_owned();
        for (etype, pos) in starts {
            match etype.name() {
                "door" => todo!("doors not supported"),
                "enemy" => {
                    world.spawn(EnemyBundle(
                        Sprite(self.spritesheet, ENEMY[Dir::S as usize]),
                        Transform {
                            x: pos.x,
                            y: pos.y,
                            w: TILE_SZ,
                            h: TILE_SZ,
                            rot: 0.0,
                        },
                        Physics::default(),
                        Enemy(Dir::S),
                        Body::new(Rect {
                            x: -(TILE_SZ as f32) / 2.0,
                            y: -(TILE_SZ as f32) / 2.0,
                            w: TILE_SZ,
                            h: TILE_SZ,
                        }),
                        Movable::default(),
                    ));
                }
                _ => (),
            }
        }
    }
}
impl Game for MyGame {
    fn new(engine: &mut Engine<'_>) -> Self {
        let spritesheet = engine.add_spritesheet(&["adventure"], Some("adventure spritesheet"));
        let level = Level::from_str(
            &engine
                .assets
                .load::<String>("adventure-l1")
                .expect("Couldn't access adventure-l1.txt")
                .read(),
            spritesheet,
            0,
        );
        for i in 0..MAX_HEALTH {
            engine.world().spawn(HeartBundle(
                Sprite(spritesheet, HEART),
                Transform {
                    x: 8.0 + i as f32 * 12.0,
                    y: H - 8.0,
                    w: 8,
                    h: 8,
                    rot: 0.0,
                },
                Heart(i),
            ));
        }
        let player_start = level
            .starts()
            .iter()
            .find(|(t, _)| t.name() == "player")
            .map(|(_, ploc)| *ploc)
            .expect("Start level doesn't put the player anywhere");
        let level = engine.world().spawn((level,));
        let player = PlayerBundle(
            Sprite(spritesheet, PLAYER[Dir::S as usize]),
            Transform {
                x: 0.0,
                y: 0.0,
                w: TILE_SZ,
                h: TILE_SZ,
                rot: 0.0,
            },
            Physics::default(),
            Player {
                dir: Dir::S,
                attack_timer: 0.0,
                knockback_timer: 0.0,
                attack: None,
            },
            Health(MAX_HEALTH),
            Body::new(Rect {
                x: -(TILE_SZ as f32) / 2.0,
                y: -(TILE_SZ as f32) / 2.0,
                w: TILE_SZ,
                h: TILE_SZ,
            }),
            Movable::default(),
        );
        let player = engine.world().spawn(player);
        let mut game = Self {
            spritesheet,
            level,
            player,
        };
        game.enter_level(player_start, engine);
        game
    }
    fn early_update(&mut self, engine: &mut Engine<'_>) {
        use frenderer::input::Key;
        let dt = engine.dt;
        let space = engine.input.is_key_pressed(Key::Space);
        // point characters in the right direction, handle inputs
        let mut dx = engine.input.key_axis(Key::ArrowLeft, Key::ArrowRight) * PLAYER_SPEED;
        // now down means -y and up means +y!  beware!
        let mut dy = engine.input.key_axis(Key::ArrowDown, Key::ArrowUp) * PLAYER_SPEED;
        let world = engine.world();
        {
            let Player {
                mut dir,
                mut attack_timer,
                mut knockback_timer,
                mut attack,
            } = *world.get::<&Player>(self.player).unwrap();
            if attack_timer > 0.0 {
                attack_timer -= dt;
            }
            if knockback_timer > 0.0 {
                knockback_timer -= dt;
            }

            let attacking = attack_timer > ATTACK_COOLDOWN_TIME;
            let knockback = knockback_timer > 0.0;
            if attacking {
                dx = 0.0;
                dy = 0.0;
            } else if knockback {
                let delta = dir.to_vec2();
                dx = -delta.x * KNOCKBACK_SPEED;
                dy = -delta.y * KNOCKBACK_SPEED;
            } else {
                if dx > 0.0 {
                    dir = Dir::E;
                }
                if dx < 0.0 {
                    dir = Dir::W;
                }
                if dy > 0.0 {
                    dir = Dir::N;
                }
                if dy < 0.0 {
                    dir = Dir::S;
                }
            }
            if let Some(atk) = attack {
                if attack_timer <= ATTACK_COOLDOWN_TIME {
                    attack = None;
                    world.despawn(atk).unwrap();
                }
            } else if attack_timer <= 0.0 && space {
                let delta = dir.to_vec2() * 8.0;
                let atk_trf = {
                    let pl_trf = world.get::<&Transform>(self.player).unwrap();
                    Transform {
                        x: pl_trf.x + delta.x,
                        y: pl_trf.y + delta.y,
                        w: TILE_SZ,
                        h: TILE_SZ,
                        rot: 0.0,
                    }
                };
                attack_timer = ATTACK_MAX_TIME;
                attack = Some(world.spawn(AttackBundle(
                    Sprite(self.spritesheet, PLAYER_ATK[dir as usize]),
                    atk_trf,
                    PlayerAttack(dir),
                    Trigger::new(Rect {
                        x: -(TILE_SZ as f32) / 2.0,
                        y: -(TILE_SZ as f32) / 2.0,
                        w: TILE_SZ,
                        h: TILE_SZ,
                    }),
                )))
            }
            world.get::<&mut Physics>(self.player).unwrap().vel = Vec2 { x: dx, y: dy };
            let mut pl_comp = world.get::<&mut Player>(self.player).unwrap();
            pl_comp.attack = attack;
            pl_comp.dir = dir;
            pl_comp.attack_timer = attack_timer;
            pl_comp.knockback_timer = knockback_timer;
        }
        let mut rng = rand::thread_rng();
        for (_enemy, (phys, Enemy(dir))) in world.query_mut::<(&mut Physics, &mut Enemy)>() {
            if rng.gen_bool(0.05) {
                *dir = match rng.gen_range(0..4) {
                    0 => Dir::N,
                    1 => Dir::E,
                    2 => Dir::S,
                    3 => Dir::W,
                    _ => panic!(),
                };
            }
            phys.vel = dir.to_vec2() * ENEMY_SPEED;
        }
    }
    fn late_update(&mut self, engine: &mut Engine<'_>) {
        {
            let (sprite, pl) = engine
                .world()
                .query_one_mut::<(&mut Sprite, &Player)>(self.player)
                .unwrap();
            sprite.1 = PLAYER[pl.dir as usize];
        }
        for (_, (sprite, Enemy(dir))) in engine.world().query_mut::<(&mut Sprite, &Enemy)>() {
            sprite.1 = ENEMY[*dir as usize];
        }

        let to_remove: Vec<_> = engine
            .world()
            .query_mut::<With<&Trigger, &PlayerAttack>>()
            .into_iter()
            .flat_map(|(_, trig)| trig.contacts())
            .copied()
            .collect();
        for enemy in to_remove {
            if engine.world().query_one_mut::<&Enemy>(enemy).is_ok() {
                println!("whammo");
                let _ = engine.world().despawn(enemy);
            }
        }
        {
            let world = engine.world();
            let mut q = world
                .query_one::<(&mut Player, &mut Health, &Movable)>(self.player)
                .unwrap();
            let (pl, Health(h), mov) = q.get().unwrap();
            for Contact { other, .. } in mov.contacts() {
                if world
                    .entity(*other)
                    .map(|e| e.has::<Enemy>())
                    .unwrap_or(false)
                    && pl.knockback_timer <= 0.0
                {
                    println!("ouch");
                    if *h == 0 {
                        panic!("game over!!");
                    }
                    *h -= 1;
                    pl.knockback_timer = KNOCKBACK_TIME;
                }
            }
            let h = *h;
            drop(q);
            for (_heart, (Heart(hid), trf)) in world.query_mut::<(&Heart, &mut Transform)>() {
                if *hid >= h {
                    *trf = Transform::ZERO;
                } else {
                    *trf = Transform {
                        x: 8.0 + (*hid as f32) * 12.0,
                        y: H - 8.0,
                        w: 8,
                        h: 8,
                        rot: 0.0,
                    }
                }
            }
        }
    }
    fn render(&mut self, _engine: &mut Engine<'_>) {}
}

fn main() {
    app!(ECSApp<MyGame>, "content").run(WindowBuilder::new(), Some((W as u32, H as u32)));
}
