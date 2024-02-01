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
//! built-in [`sprites::SpriteRenderer`], [`meshes::MeshRenderer`], [`meshes::FlatRenderer`],
//! or [`colorgeo::ColorGeo`] color-geometry postprocessing transform
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
//! set up frenderer, call e.g.  [`Renderer::sprite_group_add()`] on
//! the produced [`frenderer::Renderer`] value, and eventually call
//! [`frenderer::Renderer::sprites_mut()`] or
//! [`frenderer::Renderer::sprite_group_resize()`] to modify the
//! sprite data and [`frenderer::Renderer::render`] to draw.
//!
//! The 3D rendering facilities of frenderer are pretty basic at the
//! moment, with simple perspective cameras and unlit textured or
//! flat-colored meshes.  As in the sprite renderer, the overriding
//! performance concern has been to minimize pipeline state changes
//! and draw calls using features like instanced rendering, storage
//! buffers (where available), array textures, and packing multiple
//! meshes into a single buffer.
//!
//! Frenderer works in retained mode, but the "engine-immediate"
//! example shows how an immediate-mode render API could be built on
//! top of it.

mod gpu;
pub use gpu::WGPU;
pub use wgpu;

pub mod colorgeo;
pub mod frenderer;
pub mod meshes;
pub mod sprites;
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

pub mod bitfont;
pub mod nineslice;

pub mod clock;
