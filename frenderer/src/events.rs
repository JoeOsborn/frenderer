/// Phase in the game event loop lifecycle
pub enum EventPhase {
    /// The game should simulate time forward by the given number of steps
    Simulate(usize),
    /// The game should do whatever it needs to draw onto the screen.  Typically the caller of [`FrendererEvents::handle_event`] should respond to this by calling `render` on the [`frenderer::frenderer::Renderer`].
    Draw,
    /// The game should terminate as quickly as possible and close the window.
    Quit,
    /// There's nothing in particular the game should do right now.
    Wait,
}
/// This extension trait is used under the `winit` feature to simplify event-loop handling.
pub trait FrendererEvents<T> {
    /// Call `handle_event` on your [`frenderer::frenderer::Renderer`]
    /// with a given [`frenderer::clock::Clock`] to let Frenderer
    /// figure out "the right thing to do" for the current `winit`
    /// event.  See [`frenderer::clock::Clock`] for details on the timestep computation.
    fn handle_event(
        &mut self,
        clock: &mut crate::Clock,
        window: &winit::window::Window,
        evt: &winit::event::Event<T>,
        target: &winit::event_loop::EventLoopWindowTarget<T>,
        input: &mut crate::input::Input,
    ) -> EventPhase;
}
impl<T> FrendererEvents<T> for crate::Renderer {
    fn handle_event(
        &mut self,
        clock: &mut crate::Clock,
        window: &winit::window::Window,
        evt: &winit::event::Event<T>,
        target: &winit::event_loop::EventLoopWindowTarget<T>,
        input: &mut crate::input::Input,
    ) -> EventPhase {
        use winit::event::{Event, WindowEvent};
        target.set_control_flow(winit::event_loop::ControlFlow::Poll);
        match evt {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => EventPhase::Quit,
            winit::event::Event::WindowEvent {
                event: winit::event::WindowEvent::Resized(size),
                ..
            } => {
                self.resize(size.width, size.height);
                window.request_redraw();
                EventPhase::Wait
            }
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                window.request_redraw();
                EventPhase::Draw
            }
            Event::AboutToWait => {
                let steps = clock.tick();
                if steps > 0 {
                    EventPhase::Simulate(steps)
                } else {
                    EventPhase::Wait
                }
            }
            event => {
                input.process_input_event(event);
                EventPhase::Wait
            }
        }
    }
}
