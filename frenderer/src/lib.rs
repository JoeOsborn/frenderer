pub use color_eyre;
pub use color_eyre::eyre::Result;
pub mod animation;
pub mod assets;
pub mod camera;
mod engine;
pub use engine::{Engine, WindowSettings};
mod image;
mod input;
pub use input::{Input, Key, MousePos};
pub mod renderer;
pub mod types;
mod vulkan;

pub trait World {
    fn update(&mut self, inp: &input::Input, assets: &mut assets::Assets);
    fn render(&mut self, assets: &mut assets::Assets, render_state: &mut renderer::RenderState);
}
