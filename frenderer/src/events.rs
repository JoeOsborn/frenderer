//! The winit feature enables two useful helpers.  First is an
//! extension trait [`FrendererEvents`] that simplifies the connection
//! between winit's event loop stages and a game rendering/simulation
//! lifecycle.  Second is the [`Driver`] struct that manages winit's
//! event loop and initializes both a window and the graphics context
//! once the proper winit events have arrived.

/// Phase in the game event loop
pub enum EventPhase {
    /// The game should simulate time forward by the given number of steps and then render.  Typically the caller of [`FrendererEvents::handle_event`] should respond to this by calling `render` on the [`crate::frenderer::Renderer`].
    Run(usize),
    /// The game should terminate as quickly as possible and close the window.
    Quit,
    /// There's nothing in particular the game should do right now.
    Wait,
}

/// This extension trait is used under the `winit` feature to simplify event-loop handling.
pub trait FrendererEvents<T> {
    /// Call `handle_event` on your [`crate::frenderer::Renderer`]
    /// with a given [`crate::clock::Clock`] to let Frenderer
    /// figure out "the right thing to do" for the current `winit`
    /// event.  See [`crate::clock::Clock`] for details on the timestep computation.
    fn handle_event(
        &mut self,
        clock: &mut crate::clock::Clock,
        window: &winit::window::Window,
        evt: &winit::event::Event<T>,
        target: &winit::event_loop::EventLoopWindowTarget<T>,
        input: &mut crate::input::Input,
    ) -> EventPhase;
}
impl<T> FrendererEvents<T> for crate::Renderer {
    fn handle_event(
        &mut self,
        clock: &mut crate::clock::Clock,
        window: &winit::window::Window,
        evt: &winit::event::Event<T>,
        _target: &winit::event_loop::EventLoopWindowTarget<T>,
        input: &mut crate::input::Input,
    ) -> EventPhase {
        use winit::event::{Event, WindowEvent};
        match evt {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => EventPhase::Quit,
            winit::event::Event::WindowEvent {
                event: winit::event::WindowEvent::Resized(size),
                ..
            } => {
                // For some reason on web this causes repeated increases in size.
                if !self.gpu.is_web() {
                    self.resize_surface(size.width, size.height);
                }
                window.request_redraw();
                EventPhase::Wait
            }
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                let steps = clock.tick();
                window.request_redraw();
                EventPhase::Run(steps)
            }
            event => {
                input.process_input_event(event);
                EventPhase::Wait
            }
        }
    }
}
/// Driver takes ownership of winit's event loop and creates a window and graphics context when possible.
pub struct Driver {
    builder: winit::window::WindowBuilder,
    render_size: Option<(u32, u32)>,
}
#[cfg(all(target_arch = "wasm32", feature = "winit"))]
pub mod web_error {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum RuntimeError {
        NoCanvas,
        NoDocument,
        NoBody,
    }
    impl std::fmt::Display for RuntimeError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            <Self as std::fmt::Debug>::fmt(self, f)
        }
    }
    impl std::error::Error for RuntimeError {}
}

struct NoopWaker();
impl std::task::Wake for NoopWaker {
    fn wake(self: std::sync::Arc<Self>) {
        //nop
    }
}

impl Driver {
    /// Create a [`Driver`] with the given window builder and render target size (if absent, will use the window's inner size instead).
    pub fn new(builder: winit::window::WindowBuilder, render_size: Option<(u32, u32)>) -> Self {
        Self {
            builder,
            render_size,
        }
    }
    /// Kick off the event loop. Once the driver receives the
    /// [`winit::event::Event::Resumed`] event, it will initialize
    /// Frenderer and call `init_cb` with the window and renderer.
    /// This callback may return an application state object or
    /// userdata which will be passed as the final argument to
    /// `handler`, which will be called for every winit event /after/
    /// `init_cb` has been called.  If you don't want `run_event_loop`
    /// to own your application-specific data, you could instead store
    /// such data yourself in internally mutable types such as
    /// [`std::cell::OnceCell`] and evaluate `init_cb` for its side
    /// effects.
    ///
    /// Example:
    /// ```
    ///    drv.run_event_loop::<(), _>(
    ///      move |window, mut frend| {
    ///        let mut camera = Camera2D {
    ///          screen_pos: [0.0, 0.0],
    ///          screen_size: [1024.0, 768.0],
    ///        };
    ///        init_data(&mut frend, &mut camera);
    ///        (window, camera, frend)
    ///      },
    ///      move |event, target, (window, camera, frend)| {
    ///        // handle the winit event here, and maybe do some rendering!
    ///      });
    /// ```
    pub fn run_event_loop<T: 'static, U: 'static>(
        self,
        init_cb: impl FnOnce(std::sync::Arc<winit::window::Window>, crate::Renderer) -> U + 'static,
        mut handler: impl FnMut(winit::event::Event<T>, &winit::event_loop::EventLoopWindowTarget<T>, &mut U)
            + 'static,
    ) -> Result<(), Box<dyn std::error::Error>> {
        enum DriverState<U: 'static> {
            WaitingForResume(winit::window::WindowBuilder),
            PollingFuture(
                Arc<winit::window::Window>,
                #[allow(clippy::type_complexity)]
                std::pin::Pin<
                    Box<
                        dyn std::future::Future<
                            Output = Result<crate::Renderer, Box<dyn std::error::Error>>,
                        >,
                    >,
                >,
            ),
            Running(U),
            // This is just used as a temporary value
            InsideLoop,
        }
        use std::sync::Arc;
        use winit::event_loop::EventLoop;
        let Self {
            builder,
            render_size,
        } = self;
        prepare_logging()?;
        let event_loop: EventLoop<T> =
            winit::event_loop::EventLoopBuilder::with_user_event().build()?;
        let instance = Arc::new(wgpu::Instance::default());
        let waker = Arc::new(NoopWaker()).into();
        let mut init_cb = Some(init_cb);
        let driver_state = std::cell::Cell::new(DriverState::WaitingForResume(builder));
        let cb = move |event, target: &winit::event_loop::EventLoopWindowTarget<_>| {
            target.set_control_flow(winit::event_loop::ControlFlow::Wait);
            driver_state.set(match driver_state.replace(DriverState::InsideLoop) {
                DriverState::WaitingForResume(builder) => {
                    if let winit::event::Event::Resumed = event {
                        let window = Arc::new(builder.build(target).unwrap());
                        prepare_window(&window);
                        let surface = instance.create_surface(Arc::clone(&window)).unwrap();
                        let wsz = window.inner_size();
                        let sz = render_size.unwrap_or((wsz.width, wsz.height));
                        let future = Box::pin(crate::Renderer::with_surface(
                            sz.0,
                            sz.1,
                            wsz.width,
                            wsz.height,
                            Arc::clone(&instance),
                            surface,
                        ));
                        DriverState::PollingFuture(window, future)
                    } else {
                        DriverState::WaitingForResume(builder)
                    }
                }
                DriverState::PollingFuture(window, mut future) => {
                    let mut cx = std::task::Context::from_waker(&waker);
                    if let std::task::Poll::Ready(frend) = future.as_mut().poll(&mut cx) {
                        let frenderer = frend.unwrap();
                        let userdata = init_cb.take().unwrap()(Arc::clone(&window), frenderer);
                        DriverState::Running(userdata)
                    } else {
                        // schedule again
                        target.set_control_flow(winit::event_loop::ControlFlow::Poll);
                        DriverState::PollingFuture(window, future)
                    }
                }
                DriverState::Running(mut userdata) => {
                    handler(event, target, &mut userdata);
                    DriverState::Running(userdata)
                }
                DriverState::InsideLoop => {
                    panic!("driver state loop unexpectedly reentrant");
                }
            });
        };
        #[cfg(not(target_arch = "wasm32"))]
        {
            Ok(event_loop.run(cb)?)
        }
        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::EventLoopExtWebSys;
            Ok(event_loop.spawn(cb))
        }
    }
}

/// If you don't use [`Driver`], it may still be convenient to call
/// `prepare_window` to set up a window in a cross-platform way
/// (e.g. on web, it will add the window's canvas to the HTML
/// document).
#[allow(unused_variables)]
pub fn prepare_window(window: &winit::window::Window) {
    #[cfg(target_arch = "wasm32")]
    {
        use self::web_error::RuntimeError;
        use crate::wgpu::web_sys;
        use winit::platform::web::WindowExtWebSys;
        let doc = web_sys::window()
            .ok_or(RuntimeError::NoBody)
            .unwrap()
            .document()
            .unwrap();
        let canvas = window.canvas().ok_or(RuntimeError::NoCanvas).unwrap();
        doc.body()
            .ok_or(RuntimeError::NoBody)
            .unwrap()
            .append_child(&canvas)
            .unwrap();
    }
}

/// If you don't use [`Driver`], it may still be convenient to call
/// `prepare_logging` to set up `env_logger` or `console_log`
/// appropriately for the platform.
pub fn prepare_logging() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();
        Ok(())
    }
    #[cfg(target_arch = "wasm32")]
    {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        console_log::init_with_level(log::Level::Warn)?;
        Ok(())
    }
}
