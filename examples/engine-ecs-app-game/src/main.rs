use engine_ecs_app::geom::*;
use engine_ecs_app::*;
use frenderer::sprites::SheetRegion;

const W: f32 = 1024.0;
const H: f32 = 768.0;

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
struct Player(Dir);
struct Enemy(Dir);
struct PlayerAttack(Dir, f32, Rect);
struct Knockback(f32);
struct Health(u8);

// bundles
struct DoorBundle(Transform, Door);
struct PlayerBundle(Sprite, Transform, Player, Knockback, Health, Body, Movable);
struct EnemyBundle(Sprite, Transform, Enemy, Body, Movable);
struct AttackBundle(Sprite, Transform, PlayerAttack, Body, Trigger);
struct HeartBundle(Sprite, Transform);

struct MyGame {
    spritesheet: Spritesheet,
}
impl Game for MyGame {
    fn new(engine: &mut Engine<'_>) -> Self {
        let spritesheet = engine.add_spritesheet(&["demo"], Some("demo spritesheet"));
        Self { spritesheet }
    }

    fn early_update(&mut self, engine: &mut Engine<'_>) {
        todo!()
    }
    fn late_update(&mut self, engine: &mut Engine<'_>) {
        todo!()
    }
    fn render(&mut self, engine: &mut Engine<'_>) {}
}

fn main() {
    app!(ECSApp<MyGame>, "content").run(WindowBuilder::new(), Some((W as u32, H as u32)));
}
