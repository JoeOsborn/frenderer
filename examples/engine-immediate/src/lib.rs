use std::sync::Arc;

pub use bytemuck::Zeroable;
use frenderer::FrendererEvents;
pub use frenderer::{
    input::{Input, Key},
    BitFont, Clock,
};
pub use frenderer::{wgpu, Camera2D as Camera, Frenderer, SheetRegion, Transform};
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
    window: Arc<winit::window::Window>,
    sprite_counts: Vec<usize>,
}

impl Engine {
    pub fn new(builder: winit::window::WindowBuilder) -> Result<Self, Box<dyn std::error::Error>> {
        let event_loop = winit::event_loop::EventLoop::new()?;
        let window = Arc::new(builder.build(&event_loop)?);
        let renderer = frenderer::with_default_runtime(window.clone())?;
        let input = Input::default();
        let camera = Camera {
            screen_pos: [0.0, 0.0],
            screen_size: window.inner_size().into(),
        };
        Ok(Self {
            renderer,
            input,
            window,
            event_loop: Some(event_loop),
            camera,
            sprite_counts: vec![],
        })
    }
    pub fn run<G: Game>(mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut clock = Clock::new(1.0 / 60.0, 0.0002, 5);
        let mut game = G::new(&mut self);
        Ok(self.event_loop.take().unwrap().run(move |event, target| {
            match self.renderer.handle_event(
                &mut clock,
                &self.window,
                &event,
                target,
                &mut self.input,
            ) {
                frenderer::EventPhase::Simulate(steps) => {
                    for _ in 0..steps {
                        game.update(&mut self);
                        self.input.next_frame();
                    }
                }
                frenderer::EventPhase::Draw => {
                    self.sprite_counts.fill(0);
                    game.render(&mut self);
                    for (idx, &count) in self.sprite_counts.iter().enumerate() {
                        self.renderer
                            .sprites
                            .resize_sprite_group(&self.renderer.gpu, idx, count);
                        self.renderer
                            .sprites
                            .upload_sprites(&self.renderer.gpu, idx, 0..count);
                    }
                    self.renderer
                        .sprites
                        .set_camera_all(&self.renderer.gpu, self.camera);
                    self.renderer.render();
                }
                frenderer::EventPhase::Quit => {
                    target.exit();
                }
                frenderer::EventPhase::Wait => {}
            }
        })?)
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
            &self.renderer.create_texture(
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
