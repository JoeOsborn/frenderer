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
    let drv = frenderer::Driver::new(builder, Some((1024, 768)));
    drv.run_event_loop::<(), (Engine, G)>(
        move |window, renderer| {
            let mut engine = Engine {
                window,
                renderer,
                input: Input::default(),
                clock: Clock::new(1.0 / 60.0, 0.0002, 5),
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
        },
    )
}
