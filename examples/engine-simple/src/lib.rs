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

pub struct Engine<'r, 'w> {
    pub renderer: &'r mut frenderer::Renderer,
    pub window: &'w winit::window::Window,
    inner: std::rc::Rc<std::cell::RefCell<EngineInner>>,
}

struct EngineInner {
    input: Input,
    clock: Clock,
}

impl<'r, 'w> Engine<'r, 'w> {
    pub fn input(&self) -> std::cell::Ref<Input> {
        std::cell::Ref::map(self.inner.borrow(), |i| &i.input)
    }
}
pub mod geom;

pub fn run<G: Game>(
    builder: winit::window::WindowBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::cell::RefCell;
    use std::rc::Rc;
    let drv = frenderer::Driver::new(builder, Some((1024, 768)));
    drv.run_event_loop::<(), (Rc<RefCell<EngineInner>>, G)>(
        move |window, renderer| {
            let inner = Rc::new(RefCell::new(EngineInner {
                input: Input::default(),
                clock: Clock::new(1.0 / 60.0, 0.0002, 5),
            }));
            let mut engine = Engine {
                window,
                renderer,
                inner: Rc::clone(&inner),
            };
            let game = G::new(&mut engine);
            (inner, game)
        },
        move |event, target, window, renderer, (inner, game)| {
            let (mut clock, mut input) =
                std::cell::RefMut::map_split(inner.borrow_mut(), |i| (&mut i.clock, &mut i.input));
            match renderer.handle_event(&mut clock, window, &event, target, &mut input) {
                EventPhase::Run(steps) => {
                    drop(clock);
                    drop(input);
                    let mut engine = Engine {
                        window,
                        renderer,
                        inner: Rc::clone(inner),
                    };
                    for _ in 0..steps {
                        game.update(&mut engine);
                        engine.inner.borrow_mut().input.next_frame();
                    }
                    game.render(&mut engine);
                    renderer.render();
                }
                EventPhase::Quit => {
                    target.exit();
                }
                EventPhase::Wait => {}
            }
        },
    )
}
