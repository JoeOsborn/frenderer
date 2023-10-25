use engine::geom::*;
use engine::Engine;

#[repr(u8)]
#[derive(Clone, Copy, Debug)]
enum GuyAnims {
    Idle = 0,
    Right,
    Left,
}
#[repr(u8)]
#[derive(Clone, Copy, Debug)]
enum AppleAnims {
    Idle = 0,
}
#[repr(u8)]
#[derive(Clone, Copy, Debug)]
enum WallAnims {
    Idle = 0,
}

#[derive(Default)]
struct GuyController {}

impl engine::EntityController for GuyController {}

#[derive(Default)]
struct AppleController {}
impl engine::EntityController for AppleController {}

fn main() {
    let mut engine = Engine::new(winit::window::WindowBuilder::new());

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
    let sprite_tex = engine.renderer.gpu.create_texture(
        &sprite_img,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        sprite_img.dimensions(),
        Some("spr-demo.png"),
    );

    let guy_type = engine
        .create_entity_type(GuyController::default())
        .spritesheet(sprites, IVec2 { x: 16, y: 16 })
        .sprite_size(IVec2 { x: 12, y: 16 })
        .animations([(0..=0, 1.0), (1..=2, 0.5), (3..=4, 0.5)])
        .size(Vec2 { x: 24.0, y: 32.0 })
        .collision(engine::CollisionFlags::MOVABLE)
        .gravity(Vec2 { x: 0.0, y: -0.1 })
        .build();
    let wall_type = engine
        .create_entity_type(engine::BasicController::default())
        .spritesheet(sprites, IVec2 { x: 16, y: 16 })
        .animations([(5..=5, 1.0)])
        .collision(engine::CollisionFlags::SOLID)
        .build();
    let apple_type = engine
        .create_entity_type(AppleController::default())
        .spritesheet(sprites, IVec2 { x: 16, y: 16 })
        .animations([(6..=6, 1.0)])
        .collision(engine::CollisionFlags::TRIGGER)
        .build();
    const W: f32 = 1024.0;
    const H: f32 = 768.0;
    engine.set_camera(Rect {
        pos: Vec2 { x: 0.0, y: 0.0 },
        size: Vec2 { x: W, y: H },
    });
    let player = engine.create_entity(
        guy_type,
        Vec2 {
            x: W / 2.0,
            y: 24.0,
        },
        (),
    );
    let mut floor = engine.create_entity(wall_type, Vec2 { x: W / 2.0, y: 8.0 }, ());
    floor.size = Vec2 {
        x: engine.camera().size.x,
        y: 16.0,
    };
    let mut left_wall = engine.create_entity(wall_type, Vec2 { x: 8.0, y: H / 2.0 }, ());
    left_wall.size = Vec2 { x: 16.0, y: H };
    let mut right_wall = engine.create_entity(
        wall_type,
        Vec2 {
            x: W - 8.0,
            y: H / 2.0,
        },
        (),
    );
    right_wall.size = Vec2 { x: 16.0, y: H };
    engine.run();
}
