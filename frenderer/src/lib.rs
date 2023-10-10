//! A friendly renderer built with WGPU.
//!
//! Frenderer currently manages a [`wgpu::Instance`] and associated
//! types, initializing a custom [SpriteRenderer] based on
//! storage buffers (for native) and instance buffers (for WebGL).
//!
//! It also provides a convenience type [`input::Input`] for
//! processing user input and a utility function for loading a texture
//! (from disk or from a relative URL).
//!
//! Except for the WGPU initialization, frenderer is fully modular; in
//! particular, it does not take control of the event loop.  Typical
//! usage will call [`frenderer::with_default_runtime()`]
//! to install frenderer inside a [`winit::window::Window`], call
//! [`sprites::SpriteRenderer::add_sprite_group()`] on the resulting
//! [`frenderer::Frenderer`] value, and eventually call
//! [`frenderer::Frenderer::process_window_event()`],
//! [`sprites::SpriteRenderer::upload_sprites()`], and
//! [`frenderer::Frenderer::render`] to draw.
//!
//! In the future, calling code will have the option of constructing
//! its own [`wgpu::RenderPass`] to provide to frenderer's rendering
//! subroutine.  More types of renderers including 3D renderers will
//! also be provided.

pub mod input;

/// Whether storage buffers should be used (currently only WebGL uses instanced drawing instead)
#[cfg(not(feature = "webgl"))]
pub(crate) const USE_STORAGE: bool = true;
#[cfg(feature = "webgl")]
pub(crate) const USE_STORAGE: bool = false;

mod gpu;
pub use gpu::WGPU;

mod sprites;
pub use sprites::{GPUCamera, GPUSprite, SpriteRenderer};

/// A runtime for frenderer; mainly wraps an async runtime, but also sets up logging, etc.
/// In the future it might be responsible for setting up WGPU/providing a rendering context as well.
pub trait Runtime {
    /// Run a future to completion, blocking until finished.
    fn run_future<F: std::future::Future>(&self, f: F) -> F::Output;
}
#[cfg(not(target_arch = "wasm32"))]
/// A runtime using [`pollster`] for native builds
struct PollsterRuntime();
#[cfg(not(target_arch = "wasm32"))]
impl Runtime for PollsterRuntime {
    fn run_future<F: std::future::Future>(&self, f: F) -> F::Output {
        pollster::block_on(f)
    }
}
#[cfg(target_arch = "wasm32")]
/// A runtime using [`wasm_bindgen_futures`] for web builds
struct WebRuntime();
#[cfg(target_arch = "wasm32")]
impl Runtime for WebRuntime {
    fn run_future<F: std::future::Future>(&self, f: F) -> F::Output {
        wasm_bindgen_futures::spawn_local(f)
    }
}
pub mod frenderer;
pub use frenderer::*;
