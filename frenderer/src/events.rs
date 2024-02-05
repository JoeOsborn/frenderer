//! This extension trait simplifies the connection between winit's
//! event loop stages and a game rendering/simulation lifecycle.

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

pub struct Driver {
    builder: Option<winit::window::WindowBuilder>,
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
    pub fn new(builder: winit::window::WindowBuilder, render_size: Option<(u32, u32)>) -> Self {
        Self {
            builder: Some(builder),
            render_size,
        }
    }
    pub fn run_event_loop<T: 'static + std::fmt::Debug, U: 'static>(
        mut self,
        init_cb: impl FnOnce(&winit::window::Window, &mut crate::Renderer) -> U + 'static,
        mut handler: impl FnMut(
                winit::event::Event<T>,
                &winit::event_loop::EventLoopWindowTarget<T>,
                &winit::window::Window,
                &mut crate::Renderer,
                &mut U,
            ) + 'static,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::sync::Arc;
        use winit::event_loop::EventLoop;
        prepare_logging()?;
        let event_loop: EventLoop<T> =
            winit::event_loop::EventLoopBuilder::with_user_event().build()?;
        #[allow(clippy::type_complexity)]
        let mut future: Option<
            std::pin::Pin<
                Box<
                    dyn std::future::Future<
                        Output = Result<crate::Renderer, Box<dyn std::error::Error>>,
                    >,
                >,
            >,
        > = None;
        let mut window: Option<Arc<winit::window::Window>> = None;
        let mut frenderer = None;
        let instance = Arc::new(wgpu::Instance::default());

        let waker = Arc::new(NoopWaker()).into();
        let mut init_cb = Some(init_cb);
        let mut userdata: Option<U> = None;
        let cb = move |event, target: &winit::event_loop::EventLoopWindowTarget<_>| {
            target.set_control_flow(winit::event_loop::ControlFlow::Wait);
            if let Some(f) = future.as_mut() {
                let mut cx = std::task::Context::from_waker(&waker);
                if let std::task::Poll::Ready(frend) = f.as_mut().poll(&mut cx) {
                    future = None;
                    frenderer = Some(frend.unwrap());
                    userdata = Some(init_cb.take().unwrap()(
                        window.as_ref().unwrap(),
                        frenderer.as_mut().unwrap(),
                    ));
                } else {
                    // schedule again
                    target.set_control_flow(winit::event_loop::ControlFlow::Poll);
                }
            } else if let Some(window) = window.as_ref() {
                handler(
                    event,
                    target,
                    window,
                    frenderer.as_mut().unwrap(),
                    userdata.as_mut().unwrap(),
                );
            } else if let winit::event::Event::Resumed = event {
                window = Some(Arc::new(
                    self.builder.take().unwrap().build(target).unwrap(),
                ));
                prepare_window(window.as_ref().unwrap());
                let surface = instance
                    .create_surface(Arc::clone(window.as_ref().unwrap()))
                    .unwrap();
                let wsz = window.as_ref().unwrap().inner_size();
                let sz = self.render_size.unwrap_or((wsz.width, wsz.height));
                future = Some(Box::pin(crate::Renderer::with_surface(
                    sz.0,
                    sz.1,
                    wsz.width,
                    wsz.height,
                    Arc::clone(&instance),
                    surface,
                )));
            } else {
                // do nothing, wait for resume or poll
            }
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
