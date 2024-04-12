use frapp::assets_manager::asset::Png;
use frapp::frenderer::input::Input;
pub use frapp::frenderer::sprites::Camera2D;
use frapp::frenderer::*;
use frapp::*;
pub use frapp::{self, app, assets_manager, frenderer, winit, WindowBuilder};
pub use hecs::*;
pub use rand;
pub struct ECSApp<G: Game> {
    #[allow(dead_code)]
    assets: AssetCache,
    world: hecs::World,
    camera: Camera2D,
    frame: usize,
    game: G,
}

pub struct Engine<'app> {
    pub assets: &'app mut AssetCache,
    pub renderer: &'app mut Immediate,
    world: &'app mut hecs::World,
    pub input: &'app Input,
    pub camera: Camera2D,
    pub frame: usize,
    pub dt: f32,
}
pub trait Game: Sized + 'static {
    fn new(engine: &mut Engine<'_>) -> Self;
    fn early_update(&mut self, engine: &mut Engine<'_>);
    fn late_update(&mut self, engine: &mut Engine<'_>);
    fn render(&mut self, engine: &mut Engine<'_>);
}

impl<G: Game> ECSApp<G> {
    fn do_collision(&mut self) {
        use components::{Body, Level, Movable, Transform, Trigger};
        use geom::{Rect, Vec2};
        for (_e, mov) in self.world.query_mut::<&mut Movable>() {
            mov.0.clear();
        }
        for (_e, trigger) in self.world.query_mut::<&mut Trigger>() {
            trigger.contacts.clear();
        }

        let ent_rects: Vec<(hecs::Entity, Rect)> = self
            .world
            .query_mut::<(&Transform, &Body)>()
            .into_iter()
            .map(|(e, (t, b))| (e, b.rect + Vec2 { x: t.x, y: t.y }))
            .collect();
        let mut ent_ent: Vec<(hecs::Entity, usize, hecs::Entity, usize, f32)> = vec![];
        for (idx, (i, ei)) in ent_rects.iter().enumerate() {
            if ei.is_empty() {
                continue;
            }
            for (j, ej) in ent_rects[(idx + 1)..].iter() {
                if ej.is_empty() {
                    continue;
                }
                if let Some(ov) = ei.overlap(*ej) {
                    ent_ent.push((*i, 0, *j, 0, ov.mag_sq()));
                }
            }
        }
        for (lid, lev) in self.world.query_mut::<&Level>() {
            for (i, ei) in ent_rects.iter() {
                if ei.is_empty() {
                    continue;
                }
                for (ti, tr, _td) in lev.tiles_within(*ei).filter(|(_ti, _tr, td)| td.solid) {
                    if let Some(ov) = ei.overlap(tr) {
                        ent_ent.push((*i, 0, lid, ti, ov.mag_sq()));
                    }
                }
            }
        }
        ent_ent.sort_by(|a, b| b.4.partial_cmp(&a.4).unwrap());
        for (e1, _, e2, e2i, _) in ent_ent.drain(..) {
            assert_ne!(e1, e2);
            // assumption: e1 is a regular collision entity
            let r1 = {
                let (&Transform { x, y, .. }, &Body { rect: r }) =
                    self.world.query_one_mut::<(&Transform, &Body)>(e1).unwrap();
                r + Vec2 { x, y }
            };
            // assumption: e2 is either a level or a regular collision entity
            let r2 = {
                if let Ok(lev) = self.world.query_one_mut::<&Level>(e2) {
                    lev.tile_rect_for_index(e2i).unwrap()
                } else {
                    let (&Transform { x, y, .. }, &Body { rect: r }) =
                        self.world.query_one_mut::<(&Transform, &Body)>(e2).unwrap();
                    r + Vec2 { x, y }
                }
            };
            let mut disp = r2.overlap(r1).unwrap_or(Vec2::ZERO);
            if disp.x < disp.y {
                disp.y = 0.0;
            } else {
                disp.x = 0.0;
            }
            let r1mid = r1.center();
            let r2mid = r2.center();
            if r1mid.x < r2mid.x {
                disp.x *= -1.0;
            }
            if r1mid.y < r2mid.y {
                disp.y *= -1.0;
            }
            match (
                self.world.get::<&mut Movable>(e1),
                self.world.get::<&mut Movable>(e2),
            ) {
                (Err(_), Err(_)) => println!("Neither entity {e1:?} {e2:?} is movable!"),
                (Ok(mut mov1), Ok(mut mov2)) => {
                    mov1.0.push(Contact {
                        disp: disp * 0.5,
                        other: e2,
                    });
                    mov2.0.push(Contact {
                        disp: disp * -0.5,
                        other: e1,
                    });
                    let mut trf1 = self.world.get::<&mut Transform>(e1).unwrap();
                    trf1.x += disp.x * 0.5;
                    trf1.y += disp.y * 0.5;
                    let mut trf2 = self.world.get::<&mut Transform>(e2).unwrap();
                    trf2.x -= disp.x * 0.5;
                    trf2.y -= disp.y * 0.5;
                }
                (Ok(mut mov1), Err(_)) => {
                    mov1.0.push(Contact { disp, other: e2 });
                    let mut trf1 = self.world.get::<&mut Transform>(e1).unwrap();
                    trf1.x += disp.x;
                    trf1.y += disp.y;
                }
                (Err(_), Ok(mut mov2)) => {
                    mov2.0.push(Contact {
                        disp: disp * -1.0,
                        other: e1,
                    });
                    let mut trf2 = self.world.get::<&mut Transform>(e2).unwrap();
                    trf2.x -= disp.x;
                    trf2.y -= disp.y;
                }
            }
            use components::Physics;
            for (_ent, (mov, phys)) in self.world.query_mut::<(&Movable, &mut Physics)>() {
                for col in mov.0.iter() {
                    if col.disp.x.abs() > std::f32::EPSILON {
                        phys.vel.x = 0.0;
                    }
                    if col.disp.y.abs() > std::f32::EPSILON {
                        phys.vel.y = 0.0;
                    }
                }
            }
        }
        // now do triggers: body v trigger
        for (ei, (trfi, bodyi)) in self.world.query::<(&Transform, &Body)>().iter() {
            let ri = bodyi.rect
                + Vec2 {
                    x: trfi.x,
                    y: trfi.y,
                };
            for (ej, (trfj, triggerj)) in self.world.query::<(&Transform, &mut Trigger)>().iter() {
                if ei == ej {
                    continue;
                }
                let rj = triggerj.rect
                    + Vec2 {
                        x: trfj.x,
                        y: trfj.y,
                    };
                if ri.overlap(rj).is_some() {
                    triggerj.contacts.push(ei);
                }
            }
        }
    }
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
            dt: 0.0,
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
                dt: Self::DT,
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
                dt: Self::DT,
                camera: self.camera,
                frame: self.frame + 1,
                renderer,
            };
            self.game.late_update(&mut engine);
            self.camera = engine.camera;
            self.frame = engine.frame;
        }
    }
    fn render(&mut self, renderer: &mut Self::Renderer, dt: f32, input: &Input) {
        use components::{Level, Sprite, Text, Transform};
        self.world
            .query_mut::<&Level>()
            .into_iter()
            .for_each(|(_ent, level)| {
                level.render_immediate(renderer);
            });
        self.world
            .query_mut::<(&Sprite, &Transform)>()
            .into_iter()
            .for_each(|(_ent, (Sprite(sheet, uvs), trf))| {
                renderer.draw_sprite(sheet.0 as usize, *trf, *uvs);
            });
        self.world
            .query_mut::<(&Text, &Transform)>()
            .into_iter()
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
            dt,
        };
        self.game.render(&mut engine);
        self.camera = engine.camera;
    }
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    #[derive(Default)]
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
    pub struct Body {
        pub rect: geom::Rect,
        // If we wanted to track the body in an acceleration structure we would need
        // to remember where it used to be
        // old_rect: geom::Rect,
        // old_transform: Transform,
    }
    impl Body {
        pub fn new(rect: geom::Rect) -> Self {
            Self {
                rect,
                // old_rect: rect,
                // old_transform: Transform::ZERO,
            }
        }
    }
    pub struct Tilemap(pub level::Level);
    #[derive(Default)]
    pub struct Movable(pub(crate) Vec<Contact>);
    impl Movable {
        pub fn contacts(&self) -> &[Contact] {
            &self.0
        }
    }
    pub struct Trigger {
        pub rect: geom::Rect,
        pub(crate) contacts: Vec<hecs::Entity>,
    }
    impl Trigger {
        pub fn new(rect: geom::Rect) -> Self {
            Self {
                rect,
                contacts: vec![],
                // old_rect: rect,
                // old_transform: Transform::ZERO,
            }
        }
        pub fn contacts(&self) -> &[hecs::Entity] {
            &self.contacts
        }
    }
}

impl Engine<'_> {
    pub fn world(&mut self) -> &mut hecs::World {
        self.world
    }
    pub fn spawn<B: hecs::DynamicBundle>(&mut self, b: B) -> hecs::Entity {
        self.world.spawn(b)
    }
    pub fn despawn(&mut self, entity: hecs::Entity) -> Result<(), hecs::NoSuchEntity> {
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
            1024,
            self.camera,
        );
        assert!(idx <= 255, "too many sprite groups!");
        Spritesheet(idx as u8)
    }
}

// fn main() {
// app!(ECSApp<G>, "content").run(WindowBuilder::new(), Some((W as u32, H as u32)));
// }
