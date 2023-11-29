pub mod bitfont;
use std::time::Instant;

pub use bitfont::BitFont;
pub mod input;

const TIME_SNAPS: [f32; 5] = [15.0, 30.0, 60.0, 120.0, 144.0];

pub struct Clock {
    acc: f32,
    dt: f32,
    fudge_amount: f32,
    max_frames_per_tick: usize,
    last_t: Instant,
}

impl Clock {
    pub fn new(dt: f32, fudge_amount: f32, max_frames_per_tick: usize) -> Self {
        Self {
            acc: 0.0,
            dt,
            fudge_amount,
            max_frames_per_tick,
            last_t: Instant::now(),
        }
    }
    pub fn set_now(&mut self, instant: Instant) {
        self.last_t = instant;
    }
    pub fn tick(&mut self) -> usize {
        // compute elapsed time since last frame
        let mut elapsed = self.last_t.elapsed().as_secs_f32();
        // println!("{elapsed}");
        // snap time to nearby vsync framerate
        TIME_SNAPS.iter().for_each(|s| {
            if (elapsed - 1.0 / s).abs() < self.fudge_amount {
                elapsed = 1.0 / s;
            }
        });
        // Death spiral prevention
        if elapsed > (self.max_frames_per_tick as f32 * self.dt) {
            self.acc = 0.0;
            elapsed = self.dt;
        }
        self.acc += elapsed;
        self.last_t = std::time::Instant::now();
        // While we have time to spend

        let steps = (self.acc / self.dt) as usize;
        self.acc -= steps as f32 * self.dt;
        steps
    }
}

pub enum EventPhase {
    Simulate(usize),
    Draw,
    Quit,
    Wait,
}

pub fn handle_event<RT: frenderer::Runtime, T>(
    clock: &mut Clock,
    window: &winit::window::Window,
    evt: &winit::event::Event<T>,
    target: &winit::event_loop::EventLoopWindowTarget<T>,
    input: &mut input::Input,
    renderer: &mut frenderer::Renderer<RT>,
) -> EventPhase {
    use winit::event::{Event, WindowEvent};
    target.set_control_flow(winit::event_loop::ControlFlow::Poll);
    match evt {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => EventPhase::Quit,
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
            if renderer.process_window_event(&event) {
                window.request_redraw();
            }
            input.process_input_event(&event);
            EventPhase::Wait
        }
    }
}
