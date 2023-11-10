pub use bytemuck::Zeroable;
pub use frenderer::{
    input::{Input, Key},
    wgpu, BitFont, Camera2D as Camera, Frenderer, SheetRegion, Transform,
};
pub trait Game: Sized + 'static {
    fn new(engine: &mut Engine) -> Self;
    fn update(&mut self, engine: &mut Engine);
    fn render(&mut self, engine: &mut Engine);
}

pub struct Engine {
    pub renderer: Frenderer,
    pub input: Input,
    camera: Camera,
    event_loop: Option<winit::event_loop::EventLoop<()>>,
    window: winit::window::Window,
    sprite_counts: Vec<usize>,
}

impl Engine {
    pub fn new(builder: winit::window::WindowBuilder) -> Self {
        let event_loop = winit::event_loop::EventLoop::new();
        let window = builder.build(&event_loop).unwrap();
        let renderer = frenderer::with_default_runtime(&window);
        let input = Input::default();
        let camera = Camera {
            screen_pos: [0.0, 0.0],
            screen_size: window.inner_size().into(),
        };
        Self {
            renderer,
            input,
            window,
            event_loop: Some(event_loop),
            camera,
            sprite_counts: vec![],
        }
    }
    pub fn run<G: Game>(mut self) {
        let mut game = G::new(&mut self);
        const DT: f32 = 1.0 / 60.0;
        const DT_FUDGE_AMOUNT: f32 = 0.0002;
        const DT_MAX: f32 = DT * 5.0;
        const TIME_SNAPS: [f32; 5] = [15.0, 30.0, 60.0, 120.0, 144.0];
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
                            self.input.next_frame();
                        }
                        self.sprite_counts.fill(0);
                        game.render(&mut self);
                        for (idx, &count) in self.sprite_counts.iter().enumerate() {
                            self.renderer.sprites.resize_sprite_group(
                                &self.renderer.gpu,
                                idx,
                                count,
                            );
                            self.renderer
                                .sprites
                                .upload_sprites(&self.renderer.gpu, idx, 0..count);
                        }
                        self.renderer
                            .sprites
                            .set_camera_all(&self.renderer.gpu, self.camera);
                        self.renderer.render();
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
}

impl Engine {
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
    pub fn add_spritesheet(&mut self, img: image::RgbaImage, label: Option<&str>) -> Spritesheet {
        self.sprite_counts.push(0);
        Spritesheet(self.renderer.sprites.add_sprite_group(
            &self.renderer.gpu,
            &self.renderer.gpu.create_texture(
                &img,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                img.dimensions(),
                label,
            ),
            vec![Transform::zeroed(); 1024],
            vec![SheetRegion::zeroed(); 1024],
            self.camera,
        ))
    }
    pub fn draw_string(
        &mut self,
        spritesheet: Spritesheet,
        font: &BitFont,
        text: &str,
        pos: geom::Vec2,
        char_sz: f32,
    ) -> geom::Vec2 {
        self.ensure_spritegroup_size(
            spritesheet.0,
            self.sprite_counts[spritesheet.0] + text.len(),
        );
        let (drawn, corner) = font.draw_text(
            &mut self.renderer.sprites,
            spritesheet.0,
            self.sprite_counts[spritesheet.0],
            text,
            pos.into(),
            char_sz,
        );
        self.sprite_counts[spritesheet.0] += drawn;
        corner.into()
    }
    pub fn draw_sprite(
        &mut self,
        spritesheet: Spritesheet,
        trf: impl Into<Transform>,
        uv: SheetRegion,
    ) {
        self.ensure_spritegroup_size(spritesheet.0, self.sprite_counts[spritesheet.0] + 1);
        let (trfs, uvs) = self.renderer.sprites.get_sprites_mut(spritesheet.0);
        trfs[self.sprite_counts[spritesheet.0]] = trf.into();
        uvs[self.sprite_counts[spritesheet.0]] = uv;
        self.sprite_counts[spritesheet.0] += 1;
    }
}

pub mod geom;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Spritesheet(usize);
