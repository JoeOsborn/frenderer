pub use frenderer::{input::Input, Frenderer, GPUCamera, Region, Transform};
pub struct Engine<RT: frenderer::Runtime> {
    renderer: Frenderer<RT>,
    input: Input,
    event_loop: winit::event_loop::EventLoop<()>,
    window: winit::window::Window,
}

/// Initialize frenderer with default settings for the current target
/// architecture, including logging via `env_logger` on native or `console_log` on web.
/// On web, this also adds a canvas to the given window.  If you don't need all that behavior,
/// consider using your own [`super::Runtime`].
#[cfg(not(target_arch = "wasm32"))]
pub fn with_default_runtime(
    builder: winit::window::WindowBuilder,
) -> Engine<impl frenderer::Runtime> {
    let event_loop = winit::event_loop::EventLoop::new();
    let window = builder.build(&event_loop).unwrap();
    let renderer = frenderer::with_default_runtime(&window);
    Engine::with_renderer(window, event_loop, renderer)
}
#[cfg(target_arch = "wasm32")]
pub fn with_default_runtime(window: winit::window::WindowBuilder) -> Engine<impl super::Runtime> {
    let event_loop = winit::event_loop::EventLoop::new();
    let window = builder.build(&event_loop).unwrap();
    let renderer = frenderer::with_default_runtime(&window);
    Engine::with_renderer(window, event_loop, renderer)
}

impl<RT: frenderer::Runtime + 'static> Engine<RT> {
    pub fn with_renderer(
        window: winit::window::Window,
        event_loop: winit::event_loop::EventLoop<()>,
        renderer: Frenderer<RT>,
    ) -> Self {
        let input = Input::default();
        Self {
            renderer,
            input,
            window,
            event_loop,
        }
    }
    pub fn run(mut self) {
        const DT: f32 = 1.0 / 60.0;
        const DT_FUDGE_AMOUNT: f32 = 0.0002;
        const DT_MAX: f32 = DT * 5.0;
        const TIME_SNAPS: [f32; 5] = [15.0, 30.0, 60.0, 120.0, 144.0];
        let mut acc = 0.0;
        let mut now = std::time::Instant::now();
        self.event_loop.run(move |event, _, control_flow| {
            use winit::event::{Event, WindowEvent};
            control_flow.set_poll();
            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    *control_flow = winit::event_loop::ControlFlow::Exit;
                }
                Event::MainEventsCleared => {
                    // compute elapsed time since last frame
                    let mut elapsed = now.elapsed().as_secs_f32();
                    println!("{elapsed}");
                    // snap time to nearby vsync framerate
                    TIME_SNAPS.iter().for_each(|s| {
                        if (elapsed - 1.0 / s).abs() < DT_FUDGE_AMOUNT {
                            elapsed = 1.0 / s;
                        }
                    });
                    // Death spiral prevention
                    if elapsed > DT_MAX {
                        acc = 0.0;
                        elapsed = DT;
                    }
                    acc += elapsed;
                    now = std::time::Instant::now();
                    // While we have time to spend
                    while acc >= DT {
                        // simulate a frame
                        acc -= DT;
                        println!("tick");
                        //update_game();
                        self.input.next_frame();
                    }
                    // Render prep
                    //self.renderer.sprites.set_camera_all(&frend.gpu, camera);
                    // update sprite positions and sheet regions
                    // ok now render.
                    // We could just call frend.render().
                    self.renderer.render();
                    self.window.request_redraw();
                }
                event => {
                    if self.renderer.process_window_event(&event) {
                        self.window.request_redraw();
                    }
                    self.input.process_input_event(&event);
                }
            }
        });
    }
}
