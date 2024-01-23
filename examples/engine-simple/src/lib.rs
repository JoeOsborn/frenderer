use std::sync::Arc;

pub use bytemuck::Zeroable;
pub use frenderer::{
    bitfont::BitFont,
    clock::Clock,
    input::{Input, Key},
};
pub use frenderer::{
    sprites::{Camera2D as Camera, SheetRegion, Transform},
    wgpu, Renderer,
};
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
    pub fn run<G: Game>(
        builder: winit::window::WindowBuilder,
    ) -> Result<(), Box<dyn std::error::Error>> {
        frenderer::with_default_runtime(
            builder,
            Some((1024, 768)),
            |event_loop, window, renderer| {
                let input = Input::default();
                let this = Self {
                    renderer,
                    input,
                    window,
                    event_loop: Some(event_loop),
                };
                this.go::<G>().unwrap();
            },
        )
    }
    fn go<G: Game>(mut self) -> Result<(), Box<dyn std::error::Error>> {
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
                    self.window.request_redraw();
                }
                EventPhase::Draw => {
                    game.render(&mut self);
                    self.renderer.render();
                    self.window.request_redraw();
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
