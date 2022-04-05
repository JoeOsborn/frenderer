pub use winit::event::VirtualKeyCode as Key;

pub struct Input {
    now_keys: Box<[bool]>,
    prev_keys: Box<[bool]>,
}
impl Input {
    pub(crate) fn new() -> Self {
        Self {
            now_keys: vec![false; 255].into_boxed_slice(),
            prev_keys: vec![false; 255].into_boxed_slice(),
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
    pub(crate) fn next_frame(&mut self) {
        self.prev_keys.copy_from_slice(&self.now_keys);
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
}
