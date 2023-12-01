use std::sync::Arc;

pub use bytemuck::Zeroable;
pub use frenderer::{
    input::{Input, Key},
    BitFont, Clock,
};
pub use frenderer::{wgpu, Camera2D as Camera, Renderer, SheetRegion, Transform};
use frenderer::{EventPhase, FrendererEvents};
pub trait Game: Sized + 'static {
    fn new(engine: &mut Engine) -> Self;
    fn update(&mut self, engine: &mut Engine);
    fn render(&mut self, engine: &mut Engine);
}

pub struct Engine {
    pub renderer: Renderer,
    pub input: Input,
    event_loop: Option<winit::event_loop::EventLoop<()>>,
    window: Arc<winit::window::Window>,
}

impl Engine {
    pub fn new(
        builder: winit::window::WindowBuilder,
    ) -> Result<Engine, Box<dyn std::error::Error>> {
        let event_loop = winit::event_loop::EventLoop::new()?;
        let window = Arc::new(builder.build(&event_loop)?);
        let renderer = frenderer::with_default_runtime(window.clone())?;
        let input = Input::default();
        Ok(Self {
            renderer,
            input,
            window,
            event_loop: Some(event_loop),
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
                EventPhase::Simulate(steps) => {
                    for _ in 0..steps {
                        game.update(&mut self);
                        self.input.next_frame();
                    }
                }
                EventPhase::Draw => {
                    game.render(&mut self);
                    self.renderer.render();
                }
                EventPhase::Quit => {
                    target.exit();
                }
                EventPhase::Wait => {}
            }
        })?)
    }
}
pub mod geom;
