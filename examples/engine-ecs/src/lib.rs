use std::sync::Arc;

pub use bytemuck::Zeroable;
pub use frenderer::input::{Input, Key};
use frenderer::{clock::Clock, EventPhase, FrendererEvents};
pub use frenderer::{
    sprites::{Camera2D as Camera, SheetRegion, Transform},
    wgpu, Renderer,
};
pub use hecs;
mod gfx;
pub use gfx::{BitFont, Spritesheet};

mod game;
pub use game::Game;
mod collision;
pub use collision::Contact;

const COLLISION_STEPS: usize = 5;
use gfx::TextDraw;

pub mod geom;

pub mod components {
    pub use super::Transform;
    pub use crate::collision::{BoxCollision, Pushable, Solid, SolidPushable, Trigger};
    #[derive(Default)]
    pub struct Physics {
        pub vel: crate::geom::Vec2,
        pub acc: crate::geom::Vec2,
    }
    pub struct Sprite(pub crate::Spritesheet, pub crate::SheetRegion);
}

pub struct Engine<G: Game> {
    pub renderer: Renderer,
    world: hecs::World,
    pub input: Input,
    camera: Camera,
    contacts: collision::Contacts,
    window: Arc<winit::window::Window>,
    texts: Vec<TextDraw>,
    sim_frame: usize,
    clock: Clock,
    _game: std::marker::PhantomData<G>,
}

impl<G: Game> Engine<G> {
    pub fn run(builder: winit::window::WindowBuilder) -> Result<(), Box<dyn std::error::Error>> {
        let drv = frenderer::Driver::new(builder, Some((1024, 768)));
        drv.run_event_loop::<(), _>(
            move |window, renderer| {
                let input = Input::default();
                let camera = Camera {
                    screen_pos: [0.0, 0.0],
                    screen_size: window.inner_size().into(),
                };
                let contacts = collision::Contacts::new();
                let world = hecs::World::new();
                let mut this = Self {
                    renderer,
                    input,
                    contacts,
                    window,
                    clock: Clock::new(1.0 / 60.0, 0.0002, 5),
                    world,
                    camera,
                    texts: Vec::with_capacity(128),
                    sim_frame: 0,
                    _game: std::marker::PhantomData,
                };

                let displacements: Vec<Contact> = Vec::with_capacity(128);
                let triggers: Vec<Contact> = Vec::with_capacity(128);

                let game = G::new(&mut this);
                (this, game, displacements, triggers)
            },
            move |event, target, (ref mut engine, ref mut game, ref mut displacements, ref mut triggers)| engine.run_step(&event, target, game, displacements, triggers)
    )
    }
    pub fn world(&self) -> &hecs::World {
        &self.world
    }
    pub fn spawn<B: hecs::DynamicBundle>(&mut self, b: B) -> hecs::Entity {
        let ent = self.world.spawn(b);
        // maybe add entity to collision world
        self.contacts.insert_entity(ent, &mut self.world);
        ent
    }
    pub fn despawn(&mut self, entity: hecs::Entity) -> Result<(), hecs::NoSuchEntity> {
        // remove entity from collision world
        self.contacts.remove_entity(entity, &mut self.world);
        self.world.despawn(entity)
    }
    fn run_step(
        &mut self,
        event: &winit::event::Event<()>,
        target: &winit::event_loop::EventLoopWindowTarget<()>,
        game: &mut G,
        displacements: &mut Vec<Contact>,
        triggers: &mut Vec<Contact>,
    ) {
        match self.renderer.handle_event(
            &mut self.clock,
            &self.window,
            event,
            target,
            &mut self.input,
        ) {
            EventPhase::Run(steps) => {
                for _ in 0..steps {
                    game.update(self);
                    for (_e, (trf, phys)) in self
                        .world
                        .query_mut::<(&mut Transform, &mut components::Physics)>()
                    {
                        phys.vel += phys.acc * G::DT;
                        trf.x += phys.vel.x * G::DT;
                        trf.y += phys.vel.y * G::DT;
                        // TODO could we call contacts.update_entity here?
                    }
                    self.contacts.frame_update_index(&mut self.world);
                    for _iter in 0..COLLISION_STEPS {
                        self.contacts.do_collisions(&mut self.world);
                        self.contacts.step_update_index(&mut self.world);
                    }
                    // any displacement should clear velocity in the opposing direction
                    for Contact(e1, _e2, v) in self.contacts.displacements.iter() {
                        // if e1 is pushable, maybe reset its vel
                        // we also might have a contact for e2, e1 for the opposite push
                        if let Ok(phys) = self.world.query_one_mut::<&mut components::Physics>(*e1)
                        {
                            if v.x.abs() > std::f32::EPSILON && v.x.signum() != phys.vel.x.signum()
                            {
                                phys.vel.x = 0.0;
                            }
                            if v.y.abs() > std::f32::EPSILON && v.y.signum() != phys.vel.y.signum()
                            {
                                phys.vel.y = 0.0;
                            }
                        }
                    }
                    self.contacts.gather_triggers();
                    // we can respond to collision displacements and triggers of the frame all at once
                    displacements.append(&mut self.contacts.displacements);
                    triggers.append(&mut self.contacts.triggers);
                    game.handle_collisions(self, displacements.drain(..), triggers.drain(..));
                    displacements.clear();
                    triggers.clear();
                    // the handle_* functions might have moved things around, but we need accurate info for ad hoc queries during game::update next trip through the loop
                    self.contacts.step_update_index(&mut self.world);
                    // Remove empty quadtree branches/grid cell chunks or rows
                    self.contacts.optimize_index(&mut self.world);
                    self.input.next_frame();
                }
                game.render(self);
                let chara_len = self
                    .world
                    .query_mut::<(&Transform, &components::Sprite)>()
                    .into_iter()
                    .len();
                let text_len: usize = self.texts.iter().map(|t| t.1.len()).sum();
                self.ensure_spritegroup_size(0, chara_len + text_len);

                let (trfs, uvs) = self.renderer.sprites_mut(0, 0..(chara_len + text_len));

                for ((_e, (trf, spr)), (out_trf, out_uv)) in self
                    .world
                    .query_mut::<(&Transform, &mut components::Sprite)>()
                    .into_iter()
                    .zip(trfs.iter_mut().zip(uvs.iter_mut()))
                {
                    *out_trf = *trf;
                    *out_uv = spr.1;
                }
                // iterate through texts and draw each one
                let mut sprite_idx = chara_len;
                for TextDraw(font, text, pos, sz) in self.texts.iter() {
                    let (_corner, used) = font.draw_text(
                        &mut trfs[chara_len..(chara_len + text_len)],
                        &mut uvs[chara_len..(chara_len + text_len)],
                        text,
                        (*pos).into(),
                        0,
                        *sz,
                    );
                    sprite_idx += used;
                }
                assert_eq!(sprite_idx, chara_len + text_len);
                self.renderer.sprite_group_set_camera(0, self.camera);
                self.renderer.sprite_group_resize(0, chara_len + text_len);
                self.renderer.render();
                self.texts.clear();
            }
            EventPhase::Quit => {
                target.exit();
            }
            EventPhase::Wait => {}
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn make_font<B: std::ops::RangeBounds<char>>(
        &mut self,
        spritesheet: Spritesheet,
        range: B,
        uv: SheetRegion,
        char_w: u16,
        char_h: u16,
        padding_x: u16,
        padding_y: u16,
    ) -> BitFont {
        BitFont {
            font: frenderer::bitfont::BitFont::with_sheet_region(
                range, uv, char_w, char_h, padding_x, padding_y,
            ),
            _spritesheet: spritesheet,
        }
    }
    fn ensure_spritegroup_size(&mut self, group: usize, count: usize) {
        if count > self.renderer.sprite_group_size(group) {
            self.renderer
                .sprite_group_resize(group, count.next_power_of_two());
        }
    }
    pub fn set_camera(&mut self, camera: Camera) {
        self.camera = camera;
    }
    pub fn frame_number(&self) -> usize {
        self.sim_frame
    }
    pub fn add_spritesheet(
        &mut self,
        imgs: &[&image::RgbaImage],
        label: Option<&str>,
    ) -> Spritesheet {
        let img_bytes: Vec<_> = imgs.iter().map(|img| img.as_raw().as_slice()).collect();
        let idx = self.renderer.sprite_group_add(
            &self.renderer.create_array_texture(
                &img_bytes,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                imgs[0].dimensions(),
                label,
            ),
            vec![Transform::zeroed(); 1024],
            vec![SheetRegion::zeroed(); 1024],
            self.camera,
        );
        assert!(idx == 0, "We only support one spritesheet for now");
        Spritesheet(idx)
    }
    pub fn draw_string(&mut self, font: &BitFont, text: String, pos: geom::Vec2, char_sz: f32) {
        self.texts.push(TextDraw(font.font, text, pos, char_sz));
    }
}
