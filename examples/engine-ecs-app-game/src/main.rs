use engine_ecs_app::geom::*;
use engine_ecs_app::*;
use frenderer::sprites::SheetRegion;

const W: f32 = 1024.0;
const H: f32 = 768.0;
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
struct Door(String, u16, u16);
struct Player(Dir, f32);
struct Enemy(Dir);
struct PlayerAttack(Dir, Rect);
struct Knockback(f32);
struct Health(u8);

// bundles
struct DoorBundle(Transform, Door);
struct PlayerBundle(Sprite, Transform, Player, Knockback, Health, Body, Movable);
struct EnemyBundle(Sprite, Transform, Enemy, Body, Movable);
struct AttackBundle(Sprite, Transform, PlayerAttack, Trigger);
struct HeartBundle(Sprite, Transform);

struct MyGame {
    spritesheet: Spritesheet,
    level: hecs::Entity,
}
impl MyGame {
    fn enter_level(&mut self, start: Vec2, engine: &mut Engine) {
        // despawn all enemies
        for (enemy, _) in engine.world().query_mut::<&Enemy>() {
            engine.world().despawn(enemy).unwrap();
        }
        for (etype, pos) in engine.world().get::<&Level>(self.level).unwrap().starts() {
            match etype.name() {
                "door" => todo!("doors not supported"),
                "enemy" => {
                    engine.world().spawn(EnemyBundle(
                        Sprite(self.spritesheet, ENEMY[Dir::S as usize]),
                        Transform {
                            x: pos.x,
                            y: pos.y,
                            w: TILE_SZ,
                            h: TILE_SZ,
                            rot: 0.0,
                        },
                        Enemy(Dir::S),
                        Body::new(Rect {
                            x: 0.0,
                            y: 0.0,
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
                .load::<String>("level1")
                .expect("Couldn't access level1.txt")
                .read(),
            spritesheet,
            0,
        );
        let player_start = level
            .starts()
            .iter()
            .find(|(t, _)| t.name() == "player")
            .map(|(_, ploc)| ploc)
            .expect("Start level doesn't put the player anywhere");
        let level = engine.world().spawn(level);
        engine.world().spawn(PlayerBundle(
            Sprite(spritesheet, PLAYER[Dir::S as usize]),
            Transform {
                x: 0.0,
                y: 0.0,
                w: TILE_SZ,
                h: TILE_SZ,
                rot: 0.0,
            },
            Player(Dir::S, 0.0),
            Knockback(0.0),
            Health(3),
            Body::new(Rect {
                x: 0.0,
                y: 0.0,
                w: TILE_SZ,
                h: TILE_SZ,
            }),
            Movable::default(),
        ));
        let game = Self { spritesheet, level };
        game.enter_level(*player_start, engine);
        game
    }
    fn early_update(&mut self, engine: &mut Engine<'_>) {
        // point characters in the right direction, handle inputs
        todo!()
    }
    fn late_update(&mut self, engine: &mut Engine<'_>) {
        // make all the animations right
        todo!()
    }
    fn render(&mut self, engine: &mut Engine<'_>) {}
}

fn main() {
    app!(ECSApp<MyGame>, "content").run(WindowBuilder::new(), Some((W as u32, H as u32)));
}
