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
        use std::cell::OnceCell;
        enum InitPhase<G: Game> {
            WaitingOnResume(winit::window::WindowBuilder),
            WaitingOnEngine(Arc<winit::window::Window>, Rc<OnceCell<Engine>>),
            Ready(Arc<winit::window::Window>, Engine, G),
            Invalid,
        }
        use std::rc::Rc;
        use winit::event::Event;
        use winit::event_loop::EventLoop;
        frenderer::prepare_logging().unwrap();
        let elp = EventLoop::new().unwrap();
        let phase = std::cell::Cell::new(InitPhase::WaitingOnResume(builder));
        let instance = Arc::new(wgpu::Instance::default());
        elp.run(move |evt, tgt| {
            phase.set(match phase.replace(InitPhase::Invalid) {
                InitPhase::WaitingOnResume(builder) => {
                    if let Event::Resumed = evt {
                        let win = Arc::new(builder.build(tgt).unwrap());
                        frenderer::prepare_window(&win);
                        let surface = instance.create_surface(Arc::clone(&win)).unwrap();
                        let wsz = win.inner_size();
                        let engine_cell = Rc::new(OnceCell::default());
                        #[cfg(target_arch = "wasm32")]
                        {
                            wasm_bindgen_futures::spawn_local(Self::engine_async_init(
                                wsz,
                                Arc::clone(&win),
                                surface,
                                Arc::clone(&instance),
                                Rc::clone(&engine_cell),
                            ));
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            pollster::block_on(Self::engine_async_init(
                                wsz,
                                Arc::clone(&win),
                                surface,
                                Arc::clone(&instance),
                                Rc::clone(&engine_cell),
                            ));
                        }
                        InitPhase::WaitingOnEngine(win, engine_cell)
                    } else {
                        InitPhase::WaitingOnResume(builder)
                    }
                }
                InitPhase::WaitingOnEngine(window, engine_cell) => {
                    if Rc::strong_count(&engine_cell) == 1 {
                        if let Some(mut engine) =
                            Rc::into_inner(engine_cell).and_then(OnceCell::into_inner)
                        {
                            let game = G::new(&mut engine);
                            InitPhase::Ready(window, engine, game)
                        } else {
                            panic!("Rc has only one owner but engine is not yet initialized");
                        }
                    } else {
                        InitPhase::WaitingOnEngine(window, engine_cell)
                    }
                }
                InitPhase::Ready(window, mut engine, mut game) => {
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
                        &mut engine.clock,
                        &engine.window,
                        &evt,
                        tgt,
                        &mut engine.input,
                    ) {
                        frenderer::EventPhase::Run(steps) => {
                            for _ in 0..steps {
                                game.update(&mut engine);
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
                            game.render(&mut engine);
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
                    InitPhase::Ready(window, engine, game)
                }
                InitPhase::Invalid => {
                    panic!("unexpectedly reentrant event loop")
                }
            });
        })?;
        Ok(())
    }
    async fn engine_async_init(
        wsz: winit::dpi::PhysicalSize<u32>,
        window: Arc<winit::window::Window>,
        surface: wgpu::Surface<'static>,
        instance: Arc<wgpu::Instance>,
        engine: std::rc::Rc<std::cell::OnceCell<Engine>>,
    ) {
        let renderer = Renderer::with_surface(
            wsz.width, wsz.height, wsz.width, wsz.height, instance, surface,
        )
        .await
        .unwrap();

        engine
            .set(Self {
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
