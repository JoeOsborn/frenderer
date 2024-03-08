pub use assets_manager;
pub use frenderer;
use frenderer::clock::{Clock, Instant};
use frenderer::input::Input;
use frenderer::Frenderer;
pub use frenderer::FrendererEvents;
use frenderer::{Driver, EventPhase};
pub use winit::{self, window::WindowBuilder};

/// `frapp` exposes an alias for [assets_manager::AssetCache] that uses a different source depending on whether we're targeting native or web.
#[cfg(not(target_arch = "wasm32"))]
pub type AssetCache = assets_manager::AssetCache<assets_manager::source::FileSystem>;
#[cfg(target_arch = "wasm32")]
pub type AssetCache = assets_manager::AssetCache<assets_manager::source::Embedded>;

/// App is the main public trait of `frapp`.  Implementors get a defined new/update/render lifecycle with a choice of frenderer renderers (either [frenderer::Renderer] or [frenderer::Immediate]).
pub trait App {
    /// Target delta-time for simulation
    const DT: f32;
    /// The renderer type to use
    type Renderer: frenderer::Frenderer;
    /// Initialize the app
    fn new(renderer: &mut Self::Renderer, assets: AssetCache) -> Self;
    /// Update (called every DT seconds)
    fn update(&mut self, renderer: &mut Self::Renderer, input: &mut Input);
    /// Render (called once per present cycle)
    fn render(&mut self, renderer: &mut Self::Renderer, dt: f32);
}

use std::marker::PhantomData;

/// AppDriver is public, but should only be created from the [app] macro.
pub struct AppDriver<A: App + 'static>
where
    A::Renderer: From<frenderer::Renderer> + FrendererEvents<()>,
{
    cache: AssetCache,
    _phantom: PhantomData<A>,
}

impl<A: App + 'static> AppDriver<A>
where
    A::Renderer: From<frenderer::Renderer> + FrendererEvents<()>,
{
    /// Calling [run] hands off control to `winit` and `frenderer`.
    pub fn run(self, builder: winit::window::WindowBuilder, render_dims: Option<(u32, u32)>) {
        let drv = Driver::new(builder, render_dims);
        let mut clock = Clock::new(A::DT, 0.0002, 5);
        let mut last_render = Instant::now();
        drv.run_event_loop::<(), _>(
            move |window, renderer| {
                let input = Input::default();
                let mut rend: A::Renderer = renderer.into();
                let app = A::new(&mut rend, self.cache);
                (window, app, rend, input)
            },
            move |event, target, (window, ref mut app, ref mut renderer, ref mut input)| {
                match renderer.handle_event(&mut clock, &window, &event, target, input) {
                    EventPhase::Run(steps) => {
                        for _ in 0..steps {
                            app.update(renderer, input);
                            input.next_frame();
                        }
                        app.render(renderer, last_render.elapsed().as_secs_f32());
                        last_render = Instant::now();
                        renderer.render();
                    }
                    EventPhase::Quit => {
                        target.exit();
                    }
                    EventPhase::Wait => {}
                }
            },
        )
        .unwrap()
    }
    /// New is public for its use in a macro, it isn't super helpful to call it directly.
    pub fn new(cache: AssetCache) -> Self {
        Self {
            cache,
            _phantom: std::marker::PhantomData,
        }
    }
}
/// `app!` takes an implementor of [App] and a path to a content folder and sets up an [AppDriver] on which [AppDriver::run] can be called to start the program.
#[macro_export]
macro_rules! app {
    ($et:ty, $content:literal) => {{
        #[cfg(not(target_arch = "wasm32"))]
        let source =
            assets_manager::source::FileSystem::new($content).expect("Couldn't load resources");
        #[cfg(target_arch = "wasm32")]
        let source =
            assets_manager::source::Embedded::from(assets_manager::source::embed!($content));
        let cache = assets_manager::AssetCache::with_source(source);
        AppDriver::<$et>::new(cache)
    }};
}
