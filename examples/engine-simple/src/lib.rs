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
    let instance = Arc::new(wgpu::Instance::default());
    let mut builder = Some(builder);
    let elp = winit::event_loop::EventLoop::new()?;

    let fut = async {
        let mut renderer = Some(
            Renderer::with_surface(1024, 768, 1024, 768, Arc::clone(&instance), None)
                .await
                .unwrap(),
        );
        let mut init: Option<(Engine, G)> = None;

        elp.run(move |event, target| {
            if let winit::event::Event::Resumed = event {
                if let Some(builder) = builder.take() {
                    let window = Arc::new(builder.build(target).unwrap());
                    frenderer::prepare_window(&window);
                    // instead of using Renderer::with_surface above
                    // we could have created a surface here with
                    // instance.create_surface(&window) and then a
                    // WGPU with WGPU::new(instance).await, then build
                    // the renderer using Renderer::with_gpu.
                    let mut engine = Engine {
                        window,
                        renderer: renderer.take().unwrap(),
                        input: Input::default(),
                        clock: Clock::new(1.0 / 60.0, 0.0002, 5),
                    };
                    let game = G::new(&mut engine);
                    init = Some((engine, game));
                }
            }
            if let Some((engine, game)) = init.as_mut() {
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
        })
        .unwrap();
    };
    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen_futures::spawn_local(fut);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        pollster::block_on(fut);
    }
    Ok(())
}
