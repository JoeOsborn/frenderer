pub use winit::dpi::PhysicalPosition as MousePos;
pub use winit::event::VirtualKeyCode as Key;
use winit::event::{ElementState, MouseButton};

pub struct Input {
    now_keys: Box<[bool]>,
    prev_keys: Box<[bool]>,
    now_mouse: Box<[bool]>,
    prev_mouse: Box<[bool]>,
    now_mouse_pos: MousePos<f64>,
    prev_mouse_pos: MousePos<f64>,
}
impl Input {
    pub(crate) fn new() -> Self {
        Self {
            now_keys: vec![false; 255].into_boxed_slice(),
            prev_keys: vec![false; 255].into_boxed_slice(),
            now_mouse: vec![false; 16].into_boxed_slice(),
            prev_mouse: vec![false; 16].into_boxed_slice(),
            now_mouse_pos: MousePos { x: 0.0, y: 0.0 },
            prev_mouse_pos: MousePos { x: 0.0, y: 0.0 },
        }
    }
    pub fn is_key_down(&self, kc: Key) -> bool {
        self.now_keys[kc as usize]
    }
    pub fn is_key_up(&self, kc: Key) -> bool {
        !self.now_keys[kc as usize]
    }
    pub fn is_key_pressed(&self, kc: Key) -> bool {
        self.now_keys[kc as usize] && !self.prev_keys[kc as usize]
    }
    pub fn is_key_released(&self, kc: Key) -> bool {
        !self.now_keys[kc as usize] && self.prev_keys[kc as usize]
    }
    pub fn is_mouse_down(&self, kc: Key) -> bool {
        self.now_mouse[kc as usize]
    }
    fn mouse_button_to_usize(button: MouseButton) -> usize {
        match button {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            MouseButton::Middle => 2,
            MouseButton::Other(n) => n as usize,
        }
    }
    pub fn is_mouse_up(&self, mb: MouseButton) -> bool {
        !self.now_mouse[Self::mouse_button_to_usize(mb)]
    }
    pub fn is_mouse_pressed(&self, mb: MouseButton) -> bool {
        self.now_mouse[Self::mouse_button_to_usize(mb)]
            && !self.prev_mouse[Self::mouse_button_to_usize(mb)]
    }
    pub fn is_mouse_released(&self, mb: MouseButton) -> bool {
        !self.now_mouse[Self::mouse_button_to_usize(mb)]
            && self.prev_mouse[Self::mouse_button_to_usize(mb)]
    }
    pub fn mouse_pos(&self) -> MousePos<f64> {
        self.now_mouse_pos
    }
    pub fn mouse_delta(&self) -> MousePos<f64> {
        MousePos {
            x: self.now_mouse_pos.x - self.prev_mouse_pos.x,
            y: self.now_mouse_pos.y - self.prev_mouse_pos.y,
        }
    }
    pub fn key_axis(&self, down: Key, up: Key) -> f32 {
        (if self.is_key_down(down) { -1.0 } else { 0.0 })
            + (if self.is_key_down(up) { 1.0 } else { 0.0 })
    }
    pub(crate) fn next_frame(&mut self) {
        self.prev_keys.copy_from_slice(&self.now_keys);
        self.prev_mouse.copy_from_slice(&self.now_mouse);
        self.prev_mouse_pos = self.now_mouse_pos;
    }
    pub(crate) fn handle_key_event(&mut self, ke: winit::event::KeyboardInput) {
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
    pub(crate) fn handle_mouse_button(&mut self, state: ElementState, button: MouseButton) {
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
    pub(crate) fn handle_mouse_move(&mut self, position: MousePos<f64>) {
        self.now_mouse_pos = position;
    }
}
