use frapp::assets_manager::asset::Png;
use frapp::frenderer::input::Input;
pub use frapp::frenderer::sprites::Camera2D;
use frapp::frenderer::*;
use frapp::*;
pub use frapp::{self, app, assets_manager, frenderer, winit, WindowBuilder};

pub struct ECSApp<G: Game> {
    #[allow(dead_code)]
    assets: AssetCache,
    world: hecs::World,
    camera: Camera2D,
    frame: usize,
    game: G,
}

pub struct Engine<'app> {
    assets: &'app mut AssetCache,
    pub renderer: &'app mut Immediate,
    world: &'app mut hecs::World,
    pub input: &'app Input,
    pub camera: Camera2D,
    pub frame: usize,
}
pub trait Game: Sized + 'static {
    fn new(engine: &mut Engine<'_>) -> Self;
    fn early_update(&mut self, engine: &mut Engine<'_>);
    fn late_update(&mut self, engine: &mut Engine<'_>);
    fn render(&mut self, engine: &mut Engine<'_>);
}

impl<G: Game> ECSApp<G> {
    fn do_collision(&mut self) {}
}

impl<G: Game> App for ECSApp<G> {
    type Renderer = Immediate;
    const DT: f32 = 1.0 / 60.0;
    fn new(renderer: &mut Self::Renderer, mut assets: AssetCache) -> Self {
        let mut world = hecs::World::new();
        let (w, h) = renderer.render_size();
        let camera = Camera2D {
            screen_pos: [0.0, 0.0],
            screen_size: [w as f32, h as f32],
        };
        let mut engine = Engine {
            assets: &mut assets,
            world: &mut world,
            input: &Input::default(),
            camera,
            frame: 0,
            renderer,
        };
        let game = G::new(&mut engine);
        let camera = engine.camera;
        ECSApp {
            assets,
            world,
            camera,
            frame: 0,
            game,
        }
    }
    fn update(&mut self, renderer: &mut Self::Renderer, input: &Input) {
        {
            let mut engine = Engine {
                assets: &mut self.assets,
                world: &mut self.world,
                input,
                camera: self.camera,
                frame: self.frame + 1,
                renderer,
            };
            self.game.early_update(&mut engine);
            self.camera = engine.camera;
            self.frame = engine.frame;
        }
        self.world
            .query_mut::<(&mut components::Transform, &mut components::Physics)>()
            .into_iter()
            .for_each(|(_ent, (trf, phys))| {
                phys.vel += phys.acc * Self::DT;
                let dp = phys.vel * Self::DT;
                trf.x += dp.x;
                trf.y += dp.y;
            });
        self.do_collision();
        {
            let mut engine = Engine {
                assets: &mut self.assets,
                world: &mut self.world,
                input,
                camera: self.camera,
                frame: self.frame + 1,
                renderer,
            };
            self.game.late_update(&mut engine);
            self.camera = engine.camera;
            self.frame = engine.frame;
        }
    }
    fn render(&mut self, renderer: &mut Self::Renderer, _dt: f32, input: &Input) {
        // TODO: draw each sprite and text
        use components::{Sprite, Text, Transform};
        self.world
            .query::<(&Sprite, &Transform)>()
            .iter()
            .for_each(|(_ent, (sprite, trf))| {
                renderer.draw_sprite(sprite.0 .0 as usize, *trf, sprite.1);
            });
        self.world
            .query::<(&Text, &Transform)>()
            .iter()
            .for_each(|(_ent, (txt, trf))| {
                let Text {
                    spritesheet,
                    font,
                    text,
                    screen_pos,
                    depth,
                    char_height,
                } = &txt;
                renderer.draw_text(
                    spritesheet.0 as usize,
                    font,
                    text,
                    [trf.x + screen_pos.x, trf.y + screen_pos.y],
                    *depth,
                    *char_height,
                );
            });
        let mut engine = Engine {
            assets: &mut self.assets,
            world: &mut self.world,
            input,
            camera: self.camera,
            frame: self.frame + 1,
            renderer,
        };
        self.game.render(&mut engine);
        self.camera = engine.camera;
    }
}
pub struct Spritesheet(u8);

pub mod geom;

/// This "Contact" represents an actual displacement that happened
pub struct Contact {
    pub disp: geom::Vec2,
    pub other: hecs::Entity,
    //pub data: u64,
}
mod grid;
pub mod level;

pub mod components {
    use super::*;
    use frapp::frenderer::bitfont::BitFont;
    pub use level::Level;
    pub use sprites::Transform;
    pub struct Physics {
        pub vel: geom::Vec2,
        pub acc: geom::Vec2,
    }
    pub struct Text {
        pub spritesheet: Spritesheet,
        pub font: BitFont<std::ops::Range<char>>,
        pub text: String,
        pub screen_pos: geom::Vec2,
        pub depth: u16,
        pub char_height: f32,
    }
    pub struct Sprite(pub Spritesheet, pub sprites::SheetRegion);
    pub struct Body(pub geom::Rect);
    pub struct Tilemap(pub level::Level);
    pub struct Movable(Vec<Contact>);
    impl Movable {
        pub fn contacts(&self) -> &[Contact] {
            &self.0
        }
    }
    pub struct Trigger(Vec<Contact>);
    impl Trigger {
        pub fn contacts(&self) -> &[Contact] {
            &self.0
        }
    }
}

impl Engine<'_> {
    pub fn world(&mut self) -> &mut hecs::World {
        self.world
    }
    pub fn spawn<B: hecs::DynamicBundle>(&mut self, b: B) -> hecs::Entity {
        let ent = self.world.spawn(b);
        // TODO: maybe add entity to collision world
        ent
    }
    pub fn despawn(&mut self, entity: hecs::Entity) -> Result<(), hecs::NoSuchEntity> {
        // TODO: remove entity from collision world
        self.world.despawn(entity)
    }
    pub fn set_camera(&mut self, camera: Camera2D) {
        self.camera = camera;
    }
    pub fn frame_number(&self) -> usize {
        self.frame
    }
    pub fn add_spritesheet(&mut self, imgs: &[&str], label: Option<&str>) -> Spritesheet {
        let imgs: Vec<_> = imgs
            .iter()
            .map(|&img| {
                let img = self
                    .assets
                    .load::<Png>(img)
                    .unwrap_or_else(|err| panic!("failed to load image {} : {}", img, err));
                img.read().0.to_rgba8()
            })
            .collect();
        let bytes: Vec<_> = imgs.iter().map(|img| img.as_raw().as_slice()).collect();
        let idx = self.renderer.sprite_group_add(
            &self.renderer.create_array_texture(
                &bytes,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                imgs[0].dimensions(),
                label,
            ),
            vec![crate::sprites::Transform::ZERO; 1024],
            vec![crate::sprites::SheetRegion::ZERO; 1024],
            self.camera,
        );
        assert!(idx <= 255, "too many sprite groups!");
        Spritesheet(idx as u8)
    }
}

// fn main() {
// app!(ECSApp<G>, "content").run(WindowBuilder::new(), Some((W as u32, H as u32)));
// }
