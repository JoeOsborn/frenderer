use std::sync::Arc;

pub use bytemuck::Zeroable;
use frenderer::EventPhase;
pub use frenderer::{
    bitfont::BitFont,
    clock::Clock,
    input::{Input, Key},
};
pub use frenderer::{
    sprites::{Camera2D as Camera, SheetRegion, Transform},
    wgpu, Renderer,
};
use std::cell::OnceCell;
pub trait Game: Sized + 'static {
    fn new(engine: &mut Engine) -> Self;
    fn update(&mut self, engine: &mut Engine);
    fn render(&mut self, engine: &mut Engine);
}

pub struct Engine {
    pub renderer: frenderer::Renderer,
    pub window: std::sync::Arc<winit::window::Window>,
    pub input: Input,
    clock: frenderer::clock::Clock,
}
pub mod geom;

pub fn run<G: Game>(
    builder: winit::window::WindowBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    use frenderer::FrendererEvents;
    frenderer::prepare_logging()?;
    let mut builder = Some(builder);
    let elp = winit::event_loop::EventLoop::new()?;
    let mut init: Arc<OnceCell<(Engine, G)>> = Arc::new(OnceCell::new());
    elp.run(move |event, target| {
        if let winit::event::Event::Resumed = event {
            if let Some(builder) = builder.take() {
                let instance = Arc::new(wgpu::Instance::default());
                let window = Arc::new(builder.build(target).unwrap());
                frenderer::prepare_window(&window);
                let surface = instance.create_surface(Arc::clone(&window)).unwrap();
                let init = Arc::clone(&init);
                let window = Arc::clone(&window);
                let instance = Arc::clone(&instance);
                let fut = async move {
                    let renderer =
                        Renderer::with_surface(1024, 768, 1024, 768, instance, Some(surface))
                            .await
                            .unwrap();

                    let mut engine = Engine {
                        window,
                        renderer,
                        input: Input::default(),
                        clock: Clock::new(1.0 / 60.0, 0.0002, 5),
                    };
                    let game = G::new(&mut engine);
                    init.set((engine, game)).unwrap();
                };
                #[cfg(target_arch = "wasm32")]
                {
                    wasm_bindgen_futures::spawn_local(fut);
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    pollster::block_on(fut);
                }
            }
        }
        if let Some((engine, game)) = Arc::get_mut(&mut init).and_then(|c| c.get_mut()) {
            match engine.renderer.handle_event(
                &mut engine.clock,
                &engine.window,
                &event,
                target,
                &mut engine.input,
            ) {
                EventPhase::Run(steps) => {
                    for _ in 0..steps {
                        game.update(engine);
                        engine.input.next_frame();
                    }
                    game.render(engine);
                    engine.renderer.render();
                }
                EventPhase::Quit => {
                    target.exit();
                }
                EventPhase::Wait => {}
            }
        }
    })?;
    Ok(())
}
