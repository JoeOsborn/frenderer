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
    window: Arc<winit::window::Window>,
    sprite_renderer: SpriteRenderer,
}

impl Engine {
    pub fn run<G: Game>(
        builder: winit::window::WindowBuilder,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::cell::OnceCell;
        use winit::event::Event;
        use winit::event_loop::EventLoop;
        frenderer::prepare_logging().unwrap();
        let elp = EventLoop::new().unwrap();
        let instance = Arc::new(wgpu::Instance::default());
        let mut engine: OnceCell<Self> = OnceCell::new();
        let window: OnceCell<Arc<winit::window::Window>> = OnceCell::new();
        let mut builder = Some(builder);
        let mut clock = Clock::new(1.0 / 60.0, 0.0002, 5);
        let mut game: OnceCell<G> = OnceCell::new();
        elp.run(move |evt, tgt| {
            if window.get().is_none() {
                if let Event::Resumed = evt {
                    let win = Arc::new(builder.take().unwrap().build(tgt).unwrap());
                    frenderer::prepare_window(&win);
                    let surface = instance.create_surface(Arc::clone(&win)).unwrap();
                    let wsz = win.inner_size();
                    window.set(Arc::clone(&win)).unwrap();
                    #[cfg(target_arch = "wasm32")]
                    {
                        wasm_bindgen_futures::spawn(Self::engine_async_init(
                            wsz,
                            win,
                            surface,
                            &instance,
                            &mut engine,
                        ));
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        pollster::block_on(Self::engine_async_init(
                            wsz,
                            win,
                            surface,
                            &instance,
                            &mut engine,
                        ));
                    }
                }
            } else if let Some(engine) = engine.get_mut() {
                if let Some(game) = game.get_mut() {
                    if let winit::event::Event::WindowEvent {
                        event: winit::event::WindowEvent::Resized(size),
                        ..
                    } = evt
                    {
                        if !engine.renderer.gpu.is_web() {
                            engine.renderer.resize_render(size.width, size.height);
                            engine.window.request_redraw();
                        }
                    }
                    match engine.renderer.handle_event(
                        &mut clock,
                        &engine.window,
                        &evt,
                        tgt,
                        &mut engine.input,
                    ) {
                        frenderer::EventPhase::Run(steps) => {
                            for _ in 0..steps {
                                game.update(engine);
                                engine.input.next_frame();
                            }
                            for group in 0..engine.sprite_renderer.sprite_group_count() {
                                engine.sprite_renderer.resize_sprite_group(
                                    &engine.renderer.gpu,
                                    group,
                                    0,
                                );
                                engine.sprite_renderer.upload_sprites(
                                    &engine.renderer.gpu,
                                    group,
                                    0..engine.sprite_renderer.sprite_group_size(group),
                                );
                            }
                            game.render(engine);
                            // TODO this is actually not right if we want menus
                            engine
                                .sprite_renderer
                                .set_camera_all(&engine.renderer.gpu, engine.camera);
                            for group in 0..engine.sprite_renderer.sprite_group_count() {
                                engine.sprite_renderer.upload_sprites(
                                    &engine.renderer.gpu,
                                    group,
                                    0..engine.sprite_renderer.sprite_group_size(group),
                                );
                            }
                            engine.render();
                        }
                        frenderer::EventPhase::Quit => {
                            tgt.exit();
                        }
                        frenderer::EventPhase::Wait => {}
                    };
                } else {
                    game.set(G::new(engine))
                        .unwrap_or_else(|_| panic!("Couldn't initialize game"));
                }
            } else {
                // just waiting
            }
        })?;
        Ok(())
    }
    async fn engine_async_init(
        wsz: winit::dpi::PhysicalSize<u32>,
        win: Arc<winit::window::Window>,
        surface: wgpu::Surface<'static>,
        instance: &Arc<wgpu::Instance>,
        engine: &mut std::cell::OnceCell<Engine>,
    ) {
        let renderer = Renderer::with_surface(
            wsz.width,
            wsz.height,
            wsz.width,
            wsz.height,
            Arc::clone(instance),
            surface,
        )
        .await
        .unwrap();

        engine
            .set(Self {
                input: Input::default(),
                camera: Camera {
                    screen_pos: [0.0, 0.0],
                    screen_size: win.inner_size().into(),
                },
                sprite_renderer: SpriteRenderer::new(
                    &renderer.gpu,
                    renderer.config().view_formats[1].into(),
                    renderer.depth_texture().format(),
                ),
                window: win,
                renderer,
            })
            .unwrap_or_else(|_| panic!("Couldn't set engine cell"));
    }
    fn render(&mut self) {
        let (frame, view, mut encoder) = self.renderer.render_setup();
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
