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
