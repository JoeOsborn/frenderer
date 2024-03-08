pub use assets_manager::{self, AnyCache};
pub use frenderer::*;

pub trait App {
    const DT: f32;
    type Renderer = frenderer::Renderer;
    fn new(assets: AnyCache<'static>) -> Self;
    fn update(&mut self, renderer: &mut Self::Renderer, input: &mut Input);
    fn render(&mut self, renderer: &mut Self::Renderer, dt: f32);
}

use std::marker::PhantomData;

pub struct AppDriver<A> {
    cache: AnyCache<'static>,
    _phantom: PhantomData<A>,
}

impl<A: App> AppDriver<A> {
    pub fn run(self, builder: winit::window::WindowBuilder, render_dims: Option<(u32, u32)>) {
        let drv = frenderer::Driver::new(builder, render_dims);
        let clock = Clock::new(A::DT, 0.0002, 5);
        let mut last_render = Instant::now();
        drv.run_event_loop::<(), _>(
            move |window, renderer| {
                let input = Input::default();
                let mut rend = renderer.into();
                let app = A::new(&mut rend, self.cache);
                (window, app, clock, rend, input)
            },
            move |event, target, (window, ref mut app, ref mut clock, ref mut renderer, ref mut input)| {
                match renderer.handle_event(
                    &mut clock,
                    &window,
                    &event,
                    target,
                    &mut input,
                ) {
                    EventPhase::Run(steps) => {
                        for _ in 0..steps {
                            app.update(&mut self, &mut input);
                            input.next_frame();
                        }
                        app.render(&mut self, last_render.elapsed().as_secs_f32());
                        last_render = frenderer::time::Instant::now();
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
}

macro_rules! app {
    ($et:ty, $content:lit) => {
        #[cfg(not(target_arch = "wasm32"))]
        let source =
            assets_manager::source::FileSystem::new($content).expect("Couldn't load resources");
        #[cfg(target_arch = "wasm32")]
        let source =
            assets_manager::source::Embedded::from(assets_manager::source::embed!($content));
        let cache = assets_manager::AssetCache::with_source(source).into_any_cache();
        AppDriver {
            cache,
            _phantom: PhantomData,
        }
    };
}
