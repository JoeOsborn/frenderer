//! A wrapper for a current and previous input button/mouse state.

pub use winit::dpi::PhysicalPosition as MousePos;
pub use winit::event::VirtualKeyCode as Key;
use winit::event::{ElementState, Event, MouseButton, WindowEvent};

/// `Input` wraps a current and previous input state.  When window
/// events arrive from [`winit`], you should call
/// [`Input::process_input_event()`]; later (e.g. when handling
/// [`winit::event::Event::MainEventsCleared]), you can make queries
/// like [`Input::is_key_down()`], and when you've finished processing
/// events for a frame you can call [`Input::next_frame()`] to cycle
/// the new state to the old state.
pub struct Input {
    now_keys: Box<[bool]>,
    prev_keys: Box<[bool]>,
    now_mouse: Box<[bool]>,
    prev_mouse: Box<[bool]>,
    now_mouse_pos: MousePos<f64>,
    prev_mouse_pos: MousePos<f64>,
}
impl Default for Input {
    fn default() -> Self {
        Self {
            now_keys: vec![false; 255].into_boxed_slice(),
            prev_keys: vec![false; 255].into_boxed_slice(),
            now_mouse: vec![false; 16].into_boxed_slice(),
            prev_mouse: vec![false; 16].into_boxed_slice(),
            now_mouse_pos: MousePos { x: 0.0, y: 0.0 },
            prev_mouse_pos: MousePos { x: 0.0, y: 0.0 },
        }
    }
}
#[allow(dead_code)]
impl Input {
    /// Process a [`winit`] event and update the current keys/mouse position.
    pub fn process_input_event<T>(&mut self, ev: &Event<T>) {
        match *ev {
            // WindowEvent->KeyboardInput: Keyboard input!
            Event::WindowEvent {
                // Note this deeply nested pattern match
                event: WindowEvent::KeyboardInput { input: key_ev, .. },
                ..
            } => {
                self.handle_key_event(key_ev);
            }
            Event::WindowEvent {
                event: WindowEvent::MouseInput { state, button, .. },
                ..
            } => {
                self.handle_mouse_button(state, button);
            }
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => {
                self.handle_mouse_move(position);
            }
            _ => (),
        }
    }
    /// Is this key currently down?
    pub fn is_key_down(&self, kc: Key) -> bool {
        self.now_keys[kc as usize]
    }
    /// Is this key currently up?
    pub fn is_key_up(&self, kc: Key) -> bool {
        !self.now_keys[kc as usize]
    }
    /// Was this key just pressed on this frame?
    pub fn is_key_pressed(&self, kc: Key) -> bool {
        self.now_keys[kc as usize] && !self.prev_keys[kc as usize]
    }
    /// Was this key just released on this frame?
    pub fn is_key_released(&self, kc: Key) -> bool {
        !self.now_keys[kc as usize] && self.prev_keys[kc as usize]
    }
    /// Is this mouse button currently held?
    pub fn is_mouse_down(&self, button: MouseButton) -> bool {
        self.now_mouse[Self::mouse_button_to_usize(button)]
    }
    fn mouse_button_to_usize(button: MouseButton) -> usize {
        match button {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            MouseButton::Middle => 2,
            MouseButton::Other(n) => n as usize,
        }
    }
    /// Is this mouse button currently up?
    pub fn is_mouse_up(&self, mb: MouseButton) -> bool {
        !self.now_mouse[Self::mouse_button_to_usize(mb)]
    }
    /// Was this mouse button just pressed this frame?
    pub fn is_mouse_pressed(&self, mb: MouseButton) -> bool {
        self.now_mouse[Self::mouse_button_to_usize(mb)]
            && !self.prev_mouse[Self::mouse_button_to_usize(mb)]
    }
    /// Was this mouse button just released this frame?
    pub fn is_mouse_released(&self, mb: MouseButton) -> bool {
        !self.now_mouse[Self::mouse_button_to_usize(mb)]
            && self.prev_mouse[Self::mouse_button_to_usize(mb)]
    }
    /// Where is the mouse right now?
    pub fn mouse_pos(&self) -> MousePos<f64> {
        self.now_mouse_pos
    }
    /// How much has the mouse moved this frame?
    pub fn mouse_delta(&self) -> MousePos<f64> {
        MousePos {
            x: self.now_mouse_pos.x - self.prev_mouse_pos.x,
            y: self.now_mouse_pos.y - self.prev_mouse_pos.y,
        }
    }
    /// Given two keys (a negative and positive direction), produce a
    /// value between -1 and 1 based on which are currently held.
    pub fn key_axis(&self, down: Key, up: Key) -> f32 {
        (if self.is_key_down(down) { -1.0 } else { 0.0 })
            + (if self.is_key_down(up) { 1.0 } else { 0.0 })
    }
    /// Cycle current state to previous state.
    pub fn next_frame(&mut self) {
        self.prev_keys.copy_from_slice(&self.now_keys);
        self.prev_mouse.copy_from_slice(&self.now_mouse);
        self.prev_mouse_pos = self.now_mouse_pos;
    }
    fn handle_key_event(&mut self, ke: winit::event::KeyboardInput) {
        if let winit::event::KeyboardInput {
            virtual_keycode: Some(keycode),
            state,
            ..
        } = ke
        {
            match state {
                winit::event::ElementState::Pressed => {
                    self.now_keys[keycode as usize] = true;
                }
                winit::event::ElementState::Released => {
                    self.now_keys[keycode as usize] = false;
                }
            }
        }
    }
    fn handle_mouse_button(&mut self, state: ElementState, button: MouseButton) {
        let button = Self::mouse_button_to_usize(button);
        match state {
            ElementState::Pressed => {
                self.now_mouse[button] = true;
            }
            ElementState::Released => {
                self.now_mouse[button] = false;
            }
        }
    }
    fn handle_mouse_move(&mut self, position: MousePos<f64>) {
        self.now_mouse_pos = position;
    }
}
