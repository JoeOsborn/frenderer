pub use bytemuck::Zeroable;
pub use frenderer::{
    input::{Input, Key},
    wgpu, Frenderer, GPUCamera as Camera, SheetRegion, Transform,
};
pub use hecs;
mod gfx;
pub use gfx::{BitFont, Spritesheet};

mod game;
pub use game::Game;
mod collision;
pub use collision::Contact;

const COLLISION_STEPS: usize = 3;
use gfx::TextDraw;

pub mod geom;

pub mod components {
    pub use crate::collision::{BoxCollision, Pushable, Solid, SolidPushable, Trigger};
    pub use frenderer::Transform;
    pub struct Physics {
        pub vel: crate::geom::Vec2,
    }
    pub struct Sprite(pub crate::Spritesheet, pub crate::SheetRegion);
}

pub struct Engine<G: Game> {
    pub renderer: Frenderer,
    pub world: hecs::World,
    pub input: Input,
    camera: Camera,
    event_loop: Option<winit::event_loop::EventLoop<()>>,
    window: winit::window::Window,
    // Text drawing
    texts: Vec<TextDraw>,
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
        let world = hecs::World::new();
        Self {
            renderer,
            input,
            window,
            world,
            event_loop: Some(event_loop),
            camera,
            texts: Vec::with_capacity(128),
            _game: std::marker::PhantomData,
        }
    }
    pub fn run(mut self) {
        let mut game = G::new(&mut self);
        const DT: f32 = 1.0 / 60.0;
        const DT_FUDGE_AMOUNT: f32 = 0.0002;
        const DT_MAX: f32 = DT * 5.0;
        const TIME_SNAPS: [f32; 5] = [15.0, 30.0, 60.0, 120.0, 144.0];
        let mut contacts = collision::Contacts::new();
        let mut acc = 0.0;
        let mut now = std::time::Instant::now();
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
                        // println!("{elapsed}");
                        // snap time to nearby vsync framerate
                        TIME_SNAPS.iter().for_each(|s| {
                            if (elapsed - 1.0 / s).abs() < DT_FUDGE_AMOUNT {
                                elapsed = 1.0 / s;
                            }
                        });
                        // Death spiral prevention
                        if elapsed > DT_MAX {
                            acc = 0.0;
                            elapsed = DT;
                        }
                        acc += elapsed;
                        now = std::time::Instant::now();
                        // While we have time to spend
                        while acc >= DT {
                            // simulate a frame
                            acc -= DT;
                            game.update(&mut self);
                            for (_e, (trf, phys)) in self
                                .world
                                .query_mut::<(&mut Transform, &components::Physics)>()
                            {
                                trf.x += phys.vel.x;
                                trf.y += phys.vel.y;
                            }
                            // If we had visibility into all trf, collision shape updates, and entity insertion/deletion we could avoid the need for this
                            contacts.remake_index(&mut self.world);
                            for _iter in 0..COLLISION_STEPS {
                                contacts.do_collisions(&mut self.world);
                                game.handle_collisions(&mut self, contacts.displacements.drain(..));
                                contacts.update_index(&mut self.world);
                            }
                            // we can reuse the last index for gathering triggers
                            contacts.gather_triggers();
                            game.handle_triggers(&mut self, contacts.triggers.drain(..));
                            // Remove empty quadtree branches/grid cell chunks or rows
                            contacts.shrink_index(&mut self.world);
                            self.input.next_frame();
                        }
                        game.render(&mut self);
                        let chara_len = self
                            .world
                            .query_mut::<(&Transform, &components::Sprite)>()
                            .into_iter()
                            .len();
                        let text_len: usize = self.texts.iter().map(|t| t.1.len()).sum();
                        self.ensure_spritegroup_size(0, chara_len + text_len);

                        let (trfs, uvs) = self.renderer.sprites.get_sprites_mut(0);

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
