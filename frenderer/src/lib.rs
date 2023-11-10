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
//! usage will call [`frenderer::with_default_runtime()`] to install
//! frenderer inside a [`winit::window::Window`], call
//! [`sprites::SpriteRenderer::add_sprite_group()`] on the resulting
//! [`frenderer::Renderer`] value, and eventually call
//! [`frenderer::Renderer::process_window_event()`],
//! [`sprites::SpriteRenderer::upload_sprites()`], and
//! [`frenderer::Renderer::render`] or
//! [`frenderer::Renderer::render_into`] to draw.
//!
//! In the future, more types of renderers including 3D renderers will
//! also be provided.

pub mod input;

/// Whether storage buffers should be used (currently only WebGL uses instance buffers instead)
#[cfg(not(feature = "webgl"))]
pub(crate) const USE_STORAGE: bool = true;
#[cfg(feature = "webgl")]
pub(crate) const USE_STORAGE: bool = false;

mod gpu;
pub use gpu::WGPU;
pub use wgpu;

mod sprites;
pub use sprites::{Camera2D, SheetRegion, SpriteRenderer, Transform};
pub mod meshes;
pub use meshes::{Camera3D, Transform3D};

/// A runtime for frenderer; mainly wraps an async runtime, but also sets up logging, etc.
/// In the future it might be responsible for setting up WGPU/providing a rendering context as well.
pub trait Runtime {
    /// Run a future to completion, blocking until finished.
    fn run_future<F: std::future::Future>(&self, f: F) -> F::Output;
}
#[cfg(not(target_arch = "wasm32"))]
/// A runtime using [`pollster`] for native builds
pub struct PollsterRuntime(u8);
#[cfg(not(target_arch = "wasm32"))]
impl Runtime for PollsterRuntime {
    fn run_future<F: std::future::Future>(&self, f: F) -> F::Output {
        pollster::block_on(f)
    }
}
#[cfg(target_arch = "wasm32")]
/// A runtime using [`wasm_bindgen_futures`] for web builds
pub struct WebRuntime(u8);
#[cfg(target_arch = "wasm32")]
impl Runtime for WebRuntime {
    fn run_future<F: std::future::Future>(&self, f: F) -> F::Output {
        wasm_bindgen_futures::spawn_local(f)
    }
}
pub mod frenderer;
pub use frenderer::*;
#[cfg(not(target_arch = "wasm32"))]
pub type Frenderer = Renderer<PollsterRuntime>;
#[cfg(target_arch = "wasm32")]
pub type Frenderer = Renderer<WebRuntime>;
pub mod bitfont;
pub use bitfont::BitFont;

fn range<R: std::ops::RangeBounds<usize>>(r: R, hi: usize) -> std::ops::Range<usize> {
    let low = match r.start_bound() {
        std::ops::Bound::Included(&x) => x,
        std::ops::Bound::Excluded(&x) => x + 1,
        std::ops::Bound::Unbounded => 0,
    };
    let high = match r.end_bound() {
        std::ops::Bound::Included(&x) => x + 1,
        std::ops::Bound::Excluded(&x) => x,
        std::ops::Bound::Unbounded => hi,
    };
    low..high
}
