//! A friendly, modular renderer built with WGPU.
//!
//! Frenderer can be used in three ways (not mutually exclusive):
//! 1. As a collection of standalone rendering strategies for sprites, textured meshes, and flat-colored meshes.
//! 2. As a cross-platform wrapper over WGPU initialization and state management, offering a convenient `render()` function.
//! 3. As an application framework (with the `winit` feature) to give reasonable defaults for game simulation and rendering lifecycle.
//!
//! The entry point for frenderer will depend on how it's being used;
//! for use case (1), you can initialize a [`WGPU`] struct yourself
//! with an adapter, device, and queue, and proceed to use the
//! built-in [`SpriteRenderer`], [`MeshRenderer`], [`FlatRenderer`],
//! or [`colorgeo::Colorgeo`] color-geometry postprocessing transform
//! with your own renderpass. In use case (2), you can initialize a
//! [`Renderer`] with a given runtime, size, and GPU surface, and call
//! [`Renderer::render`] to handle all the drawing.  Finally, in use
//! case (3), you'll use [`clock::Clock`], the extension trait in
//! [`events::FrendererEvents`], and the [`input::Input`] struct to
//! simplify your game loop's lifecycle.
//!
//! frenderer is highly modular, especially in case (1); in
//! particular, frenderer does not take control of the event loop from
//! winit or exclusively own the WGPU instance, device, and adapter.
//! Typical usage will call [`frenderer::with_default_runtime()`] to
//! set up frenderer, call e.g.
//! [`SpriteRenderer::add_sprite_group()`] on the resulting
//! [`frenderer::Renderer`] value, and eventually call
//! [`SpriteRenderer::upload_sprites()`] and
//! [`frenderer::Renderer::render`] to draw.

mod gpu;
pub use gpu::WGPU;
pub use wgpu;

mod sprites;
pub use sprites::{Camera2D, SheetRegion, SpriteRenderer, Transform};
pub mod meshes;
pub use meshes::{Camera3D, Transform3D};
pub mod colorgeo;
pub mod frenderer;
pub use frenderer::*;

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

#[cfg(feature = "winit")]
mod events;
#[cfg(feature = "winit")]
pub mod input;
#[cfg(feature = "winit")]
pub use events::*;

mod bitfont;
pub use bitfont::BitFont;

mod clock;
pub use clock::Clock;
pub use clock::Instant;
