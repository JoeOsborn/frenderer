//! [`Renderer`] is the main user-facing type of this crate.  You can
//! make one using [`with_default_runtime()`] or provide your own
//! [`super::Runtime`] implementor via [`Renderer::with_runtime()`].

use crate::{sprites::SpriteRenderer, WGPU};
use winit::event::{Event, WindowEvent};

pub use crate::meshes::MeshRenderer;
/// A wrapper over GPU state and (for now) a sprite renderer.
pub struct Renderer<RT: super::Runtime> {
    pub gpu: WGPU,
    pub sprites: SpriteRenderer,
    pub meshes: MeshRenderer,
    runtime: RT,
}

/// Initialize frenderer with default settings for the current target
/// architecture, including logging via `env_logger` on native or `console_log` on web.
/// On web, this also adds a canvas to the given window.  If you don't need all that behavior,
/// consider using your own [`super::Runtime`].
#[cfg(not(target_arch = "wasm32"))]
pub fn with_default_runtime(window: &winit::window::Window) -> super::Frenderer {
    env_logger::init();
    Renderer::with_runtime(window, super::PollsterRuntime(0))
}
#[cfg(target_arch = "wasm32")]
pub fn with_default_runtime(window: &winit::window::Window) -> super::Frenderer {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Trace).expect("could not initialize logger");
    use winit::platform::web::WindowExtWebSys;
    // On wasm, append the canvas to the document body
    web_sys::window()
        .and_then(|win| win.document())
        .and_then(|doc| doc.body())
        .and_then(|body| {
            body.append_child(&web_sys::Element::from(window.canvas()))
                .ok()
        })
        .expect("couldn't append canvas to document body");
    Renderer::with_runtime(window, super::WebRuntime(0))
}

impl<RT: super::Runtime> Renderer<RT> {
    /// Create a new Renderer with the given window and runtime.
    pub fn with_runtime(window: &winit::window::Window, runtime: RT) -> Self {
        let gpu = runtime.run_future(WGPU::new(window));
        let sprites = SpriteRenderer::new(&gpu);
        let meshes = MeshRenderer::new(&gpu);
        Self {
            gpu,
            sprites,
            runtime,
            meshes,
        }
    }
    /// Run a future to completion.  Convenience method to wrap the runtime's executor.
    pub fn block_on<F: std::future::Future>(&self, f: F) -> F::Output {
        self.runtime.run_future(f)
    }
    /// Process a window event
    /// (e.g. [`winit::event::WindowEvent::Resized`]).  Will resize
    /// the surface or perform other renderer-appropriate actions.
    /// Returns `true` if the window should be redrawn.
    pub fn process_window_event<T>(&mut self, ev: &Event<T>) -> bool {
        match *ev {
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                self.gpu.resize(size);
                true
            }
            _ => false,
        }
    }
    /// Acquire the next frame, create a [`wgpu::RenderPass`], draw
    /// into it, and submit the encoder.
    pub fn render(&self) {
        let (frame, view, mut encoder) = self.render_setup();
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.gpu.depth_texture_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });
            self.render_into(&mut rpass);
        }
        self.render_finish(frame, encoder);
    }
    /// Renders all the frenderer stuff into a given
    /// [`wgpu::RenderPass`].  Just does rendering, no encoder
    /// submitting or frame acquire/present.
    pub fn render_into<'s, 'pass>(&'s self, rpass: &mut wgpu::RenderPass<'pass>)
    where
        's: 'pass,
    {
        self.meshes.render(rpass, ..);
        self.sprites.render(rpass, ..);
    }
    /// Convenience method for acquiring a surface texture, view, and
    /// command encoder
    pub fn render_setup(
        &self,
    ) -> (
        wgpu::SurfaceTexture,
        wgpu::TextureView,
        wgpu::CommandEncoder,
    ) {
        let frame = self
            .gpu
            .surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture");
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        (frame, view, encoder)
    }
    /// Convenience method for submitting a command encoder and
    /// presenting the swapchain image.
    pub fn render_finish(&self, frame: wgpu::SurfaceTexture, encoder: wgpu::CommandEncoder) {
        self.gpu.queue.submit(Some(encoder.finish()));
        frame.present();
    }
}
