pub use bytemuck::Zeroable;
pub use frenderer::{
    input::{Input, Key},
    wgpu, Camera2D as Camera, Frenderer, SheetRegion, Transform,
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
    pub struct Physics {
        pub vel: crate::geom::Vec2,
    }
    pub struct Sprite(pub crate::Spritesheet, pub crate::SheetRegion);
}

pub struct Engine<G: Game> {
    pub renderer: Frenderer,
    world_: hecs::World,
    pub input: Input,
    camera: Camera,
    contacts: collision::Contacts,
    event_loop: Option<winit::event_loop::EventLoop<()>>,
    window: winit::window::Window,
    // Text drawing
    texts: Vec<TextDraw>,
    sim_times: std::collections::VecDeque<f32>,
    render_times: std::collections::VecDeque<f32>,
    net_times: std::collections::VecDeque<f32>,
    sim_frame: usize,
    _game: std::marker::PhantomData<G>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CharaID(
    u8, /* group */
    /* consider: generation, and matching generation on chara */
    u32, /* index within group */
);

impl<G: Game> Engine<G> {
    pub fn new(builder: winit::window::WindowBuilder) -> Self {
        let event_loop = winit::event_loop::EventLoop::new();
        let window = builder.build(&event_loop).unwrap();
        let renderer = frenderer::with_default_runtime(&window);
        let input = Input::default();
        let camera = Camera {
            screen_pos: [0.0, 0.0],
            screen_size: window.inner_size().into(),
        };
        let contacts = collision::Contacts::new();
        let world = hecs::World::new();
        Self {
            renderer,
            input,
            contacts,
            window,
            world_: world,
            sim_times: std::collections::VecDeque::new(),
            net_times: std::collections::VecDeque::new(),
            render_times: std::collections::VecDeque::new(),
            event_loop: Some(event_loop),
            camera,
            texts: Vec::with_capacity(128),
            sim_frame: 0,
            _game: std::marker::PhantomData,
        }
    }
    pub fn world(&self) -> &hecs::World {
        &self.world_
    }
    pub fn spawn<B: hecs::DynamicBundle>(&mut self, b: B) -> hecs::Entity {
        let ent = self.world_.spawn(b);
        // maybe add entity to collision world
        self.contacts.insert_entity(ent, &mut self.world_);
        ent
    }
    pub fn despawn(&mut self, entity: hecs::Entity) -> Result<(), hecs::NoSuchEntity> {
        // remove entity from collision world
        self.contacts.remove_entity(entity, &mut self.world_);
        self.world_.despawn(entity)
    }
    pub fn run(mut self) {
        let mut game = G::new(&mut self);
        const DT_FUDGE_AMOUNT: f32 = 0.0002;
        let dt_max: f32 = G::DT * 5.0;
        const TIME_SNAPS: [f32; 5] = [15.0, 30.0, 60.0, 120.0, 144.0];
        let mut acc = 0.0;
        let mut now = std::time::Instant::now();
        let mut displacements: Vec<Contact> = Vec::with_capacity(128);
        let mut triggers: Vec<Contact> = Vec::with_capacity(128);
        self.event_loop
            .take()
            .unwrap()
            .run(move |event, _, control_flow| {
                use winit::event::{Event, WindowEvent};
                control_flow.set_poll();
                match event {
                    Event::WindowEvent {
                        event: WindowEvent::CloseRequested,
                        ..
                    } => {
                        *control_flow = winit::event_loop::ControlFlow::Exit;
                    }
                    Event::MainEventsCleared => {
                        // compute elapsed time since last frame
                        let mut elapsed = now.elapsed().as_secs_f32();
                        while self.net_times.len() > 10 {
                            self.net_times.pop_front();
                        }
                        self.net_times.push_back(elapsed);
                        // println!("{elapsed}");
                        // snap time to nearby vsync framerate
                        TIME_SNAPS.iter().for_each(|s| {
                            if (elapsed - 1.0 / s).abs() < DT_FUDGE_AMOUNT {
                                elapsed = 1.0 / s;
                            }
                        });
                        // Death spiral prevention
                        if elapsed > dt_max {
                            acc = 0.0;
                            elapsed = G::DT;
                        }
                        acc += elapsed;
                        now = std::time::Instant::now();
                        // While we have time to spend
                        while acc >= G::DT {
                            let sim_time = std::time::Instant::now();
                            // simulate a frame
                            acc -= G::DT;
                            game.update(&mut self);
                            for (_e, (trf, phys)) in self
                                .world_
                                .query_mut::<(&mut Transform, &components::Physics)>()
                            {
                                trf.x += phys.vel.x;
                                trf.y += phys.vel.y;
                                // TODO could we call contacts.update_entity here?
                            }
                            self.contacts.frame_update_index(&mut self.world_);
                            for _iter in 0..COLLISION_STEPS {
                                self.contacts.do_collisions(&mut self.world_);
                                self.contacts.step_update_index(&mut self.world_);
                            }
                            self.contacts.gather_triggers();
                            // we can response to collision displacements and triggers of the frame all at once
                            displacements.append(&mut self.contacts.displacements);
                            triggers.append(&mut self.contacts.triggers);
                            game.handle_collisions(
                                &mut self,
                                displacements.drain(..),
                                triggers.drain(..),
                            );
                            displacements.clear();
                            triggers.clear();
                            // the handle_* functions might have moved things around, but we need accurate info for ad hoc queries during game::update next trip through the loop
                            self.contacts.step_update_index(&mut self.world_);
                            // Remove empty quadtree branches/grid cell chunks or rows
                            self.contacts.optimize_index(&mut self.world_);
                            self.input.next_frame();
                            while self.sim_times.len() > 10 {
                                self.sim_times.pop_front();
                            }
                            self.sim_times.push_back(sim_time.elapsed().as_secs_f32());
                            self.sim_frame += 1;
                        }
                        let render_now = std::time::Instant::now();
                        game.render(&mut self);
                        let chara_len = self
                            .world_
                            .query_mut::<(&Transform, &components::Sprite)>()
                            .into_iter()
                            .len();
                        let text_len: usize = self.texts.iter().map(|t| t.1.len()).sum();
                        self.ensure_spritegroup_size(0, chara_len + text_len);

                        let (trfs, uvs) = self.renderer.sprites.get_sprites_mut(0);

                        for ((_e, (trf, spr)), (out_trf, out_uv)) in self
                            .world_
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
                            let (count, _) = font.draw_text(
                                &mut self.renderer.sprites,
                                0,
                                sprite_idx,
                                text,
                                (*pos).into(),
                                *sz,
                            );
                            sprite_idx += count;
                        }
                        assert_eq!(sprite_idx, chara_len + text_len);
                        // TODO: this could be more efficient by only uploading charas which changed
                        self.renderer.sprites.upload_sprites(
                            &self.renderer.gpu,
                            0,
                            0..(chara_len + text_len),
                        );
                        // update sprites from charas
                        // update texts
                        self.renderer
                            .sprites
                            .set_camera_all(&self.renderer.gpu, self.camera);
                        self.renderer.sprites.resize_sprite_group(
                            &self.renderer.gpu,
                            0,
                            chara_len + text_len,
                        );
                        self.renderer.render();
                        self.texts.clear();
                        while self.render_times.len() > 10 {
                            self.render_times.pop_front();
                        }
                        self.render_times
                            .push_back(render_now.elapsed().as_secs_f32());
                        self.window.request_redraw();
                    }
                    event => {
                        if self.renderer.process_window_event(&event) {
                            self.window.request_redraw();
                        }
                        self.input.process_input_event(&event);
                    }
                }
            });
    }
    pub fn avg_render_time(&self) -> f32 {
        self.render_times.iter().sum::<f32>() / (self.render_times.len() as f32)
    }
    pub fn avg_net_time(&self) -> f32 {
        self.net_times.iter().sum::<f32>() / (self.net_times.len() as f32)
    }
    pub fn avg_sim_time(&self) -> f32 {
        self.sim_times.iter().sum::<f32>() / (self.sim_times.len() as f32)
    }
    pub fn max_render_time(&self) -> f32 {
        self.render_times
            .iter()
            .fold(0.0_f32, |seed, t| if seed < *t { *t } else { seed })
    }
    pub fn max_sim_time(&self) -> f32 {
        self.sim_times
            .iter()
            .fold(0.0_f32, |seed, t| if seed < *t { *t } else { seed })
    }
    pub fn max_net_time(&self) -> f32 {
        self.net_times
            .iter()
            .fold(0.0_f32, |seed, t| if seed < *t { *t } else { seed })
    }
    pub fn make_font<B: std::ops::RangeBounds<char>>(
        &mut self,
        spritesheet: Spritesheet,
        range: B,
        uv: SheetRegion,
        chars_per_row: u16,
    ) -> BitFont<B> {
        BitFont {
            font: frenderer::BitFont::with_sheet_region(range, uv, chars_per_row),
            _spritesheet: spritesheet,
        }
    }
    fn ensure_spritegroup_size(&mut self, group: usize, count: usize) {
        if count > self.renderer.sprites.sprite_group_size(group) {
            self.renderer.sprites.resize_sprite_group(
                &self.renderer.gpu,
                group,
                count.next_power_of_two(),
            );
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
        let idx = self.renderer.sprites.add_sprite_group(
            &self.renderer.gpu,
            &self.renderer.gpu.create_array_texture(
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
        self.texts
            .push(TextDraw(font.font.clone(), text, pos, char_sz));
    }
}
