use std::sync::Arc;

pub use bytemuck::Zeroable;
pub use frenderer::{
    bitfont::BitFont,
    clock::Clock,
    input::{Input, Key},
};
use frenderer::{sprites::SpriteRenderer, FrendererEvents};
pub use frenderer::{
    sprites::{Camera2D as Camera, SheetRegion, Transform},
    wgpu, Renderer,
};
pub trait Game: Sized + 'static {
    fn new(engine: &mut Engine) -> Self;
    fn update(&mut self, engine: &mut Engine);
    fn render(&mut self, engine: &mut Engine);
}

pub struct Engine {
    pub renderer: Renderer,
    pub input: Input,
    camera: Camera,
    clock: Clock,
    window: Arc<winit::window::Window>,
    sprite_renderer: SpriteRenderer,
}

impl Engine {
    pub fn run<G: Game>(
        builder: winit::window::WindowBuilder,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let drv = frenderer::Driver::new(builder, Some((1024, 768)));
        drv.run_event_loop::<(), _>(
            move |window, renderer| {
                let mut engine = Self {
                    input: Input::default(),
                    camera: Camera {
                        screen_pos: [0.0, 0.0],
                        screen_size: window.inner_size().into(),
                    },
                    clock: Clock::new(1.0 / 60.0, 0.0002, 5),
                    sprite_renderer: SpriteRenderer::new(
                        &renderer.gpu,
                        renderer.config().view_formats[1].into(),
                        renderer.depth_texture().format(),
                    ),
                    window,
                    renderer,
                };
                let game = G::new(&mut engine);
                (engine, game)
            },
            move |event, target, (ref mut engine, ref mut game)| match engine.renderer.handle_event(
                &mut engine.clock,
                &engine.window,
                &event,
                target,
                &mut engine.input,
            ) {
                frenderer::EventPhase::Run(steps) => {
                    for _ in 0..steps {
                        game.update(engine);
                        engine.input.next_frame();
                    }
                    engine.pre_render();
                    game.render(engine);
                    engine.render();
                }
                frenderer::EventPhase::Quit => {
                    target.exit();
                }
                frenderer::EventPhase::Wait => {}
            },
        )?;
        Ok(())
    }
    fn pre_render(&mut self) {
        let surface_size = self.renderer.surface_size();
        if self.renderer.render_size() != surface_size {
            self.renderer.resize_render(surface_size.0, surface_size.1);
            self.window.request_redraw();
        }

        for group in 0..self.sprite_renderer.sprite_group_count() {
            self.sprite_renderer
                .resize_sprite_group(&self.renderer.gpu, group, 0);
            self.sprite_renderer.upload_sprites(
                &self.renderer.gpu,
                group,
                0..self.sprite_renderer.sprite_group_size(group),
            );
        }
    }
    fn render(&mut self) {
        // TODO this is actually not right if we want menus
        self.sprite_renderer
            .set_camera_all(&self.renderer.gpu, self.camera);
        for group in 0..self.sprite_renderer.sprite_group_count() {
            self.sprite_renderer.upload_sprites(
                &self.renderer.gpu,
                group,
                0..self.sprite_renderer.sprite_group_size(group),
            );
        }
        let Some((frame, view, mut encoder)) = self.renderer.render_setup() else {
            return;
        };
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: self.renderer.depth_texture_view(),
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            self.sprite_renderer.render(&mut rpass, ..);
        }
        self.renderer.render_finish(frame, encoder);
    }
}

impl Engine {
    fn ensure_spritegroup_size(&mut self, group: usize, count: usize) {
        if count > self.sprite_renderer.sprite_group_size(group) {
            // grow big enough to get enough capacity (and limit number of reallocations)
            self.sprite_renderer.resize_sprite_group(
                &self.renderer.gpu,
                group,
                count.next_power_of_two(),
            );
            // then shrink to the requested size
            self.sprite_renderer
                .resize_sprite_group(&self.renderer.gpu, group, count);
        }
    }
    pub fn set_camera(&mut self, camera: Camera) {
        self.camera = camera;
    }
    pub fn add_spritesheet(&mut self, img: image::RgbaImage, label: Option<&str>) -> Spritesheet {
        let ret = Spritesheet(self.sprite_renderer.add_sprite_group(
            &self.renderer.gpu,
            &self.renderer.create_array_texture(
                &[&img],
                wgpu::TextureFormat::Rgba8UnormSrgb,
                img.dimensions(),
                label,
            ),
            vec![Transform::zeroed(); 1024],
            vec![SheetRegion::zeroed(); 1024],
            self.camera,
        ));
        self.sprite_renderer
            .resize_sprite_group(&self.renderer.gpu, ret.0, 0);
        ret
    }
    pub fn draw_string(
        &mut self,
        spritesheet: Spritesheet,
        font: &BitFont,
        text: &str,
        pos: geom::Vec2,
        char_sz: f32,
    ) -> geom::Vec2 {
        let start = self.sprite_renderer.sprite_group_size(spritesheet.0);
        self.ensure_spritegroup_size(spritesheet.0, start + text.len());
        let (trfs, uvs) = self.sprite_renderer.get_sprites_mut(spritesheet.0);
        let trfs = &mut trfs[start..(start + text.len())];
        let uvs = &mut uvs[start..(start + text.len())];
        let corner = font.draw_text(trfs, uvs, text, pos.into(), 0, char_sz);
        corner.into()
    }
    pub fn draw_sprite(
        &mut self,
        spritesheet: Spritesheet,
        trf: impl Into<Transform>,
        uv: SheetRegion,
    ) {
        let start = self.sprite_renderer.sprite_group_size(spritesheet.0);
        self.ensure_spritegroup_size(spritesheet.0, start + 1);
        let (trfs, uvs) = self.sprite_renderer.get_sprites_mut(spritesheet.0);
        trfs[start] = trf.into();
        uvs[start] = uv;
    }
}

pub mod geom;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Spritesheet(usize);
