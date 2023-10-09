pub mod input;

#[cfg(not(feature = "webgl"))]
pub(crate) const USE_STORAGE: bool = true;
#[cfg(feature = "webgl")]
pub(crate) const USE_STORAGE: bool = false;

mod gpu;
pub use gpu::WGPU;

mod sprites;
pub use sprites::{GPUCamera, GPUSprite};

pub trait Runtime {
    fn run_future<F: std::future::Future>(&self, f: F) -> F::Output;
}
#[cfg(not(target_arch = "wasm32"))]
struct PollsterRuntime();
#[cfg(not(target_arch = "wasm32"))]
impl Runtime for PollsterRuntime {
    fn run_future<F: std::future::Future>(&self, f: F) -> F::Output {
        pollster::block_on(f)
    }
}
#[cfg(target_arch = "wasm32")]
struct WebRuntime();
#[cfg(target_arch = "wasm32")]
impl Runtime for WebRuntime {
    fn run_future<F: std::future::Future>(&self, f: F) -> F::Output {
        wasm_bindgen_futures::spawn_local(f)
    }
}
pub mod frenderer;
pub use frenderer::*;
