pub enum EventPhase {
    Simulate(usize),
    Draw,
    Quit,
    Wait,
}
pub trait FrendererEvents<T> {
    fn handle_event(
        &mut self,
        clock: &mut crate::Clock,
        window: &winit::window::Window,
        evt: &winit::event::Event<T>,
        target: &winit::event_loop::EventLoopWindowTarget<T>,
        input: &mut crate::input::Input,
    ) -> EventPhase;
}
impl<RT: super::Runtime, T> FrendererEvents<T> for crate::Renderer<RT> {
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
