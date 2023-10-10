//! [`Frenderer`] is the main user-facing type of this crate.  You can
//! make one using [`with_default_runtime()`] or provide your own
//! [`super::Runtime`] implementor via [`Frenderer::with_runtime()`].

use crate::{sprites::SpriteRenderer, WGPU};
use winit::event::{Event, WindowEvent};

/// A wrapper over GPU state and (for now) a sprite renderer.
pub struct Frenderer<RT: super::Runtime> {
    pub gpu: WGPU,
    pub sprites: SpriteRenderer,
    runtime: RT,
}

/// Initialize frenderer with default settings for the current target
/// architecture, including logging via `env_logger` on native or `console_log` on web.
/// On web, this also adds a canvas to the given window.  If you don't need all that behavior,
/// consider using your own [`super::Runtime`].
#[cfg(not(target_arch = "wasm32"))]
pub fn with_default_runtime(window: &winit::window::Window) -> Frenderer<impl super::Runtime> {
    env_logger::init();
    Frenderer::with_runtime(window, super::PollsterRuntime {})
}
#[cfg(target_arch = "wasm32")]
pub fn with_default_runtime(window: &winit::Window::Window) -> Frenderer<impl super::Runtime> {
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
    Frenderer::with_runtime(window, super::WebRuntime)
}

impl<RT: super::Runtime> Frenderer<RT> {
    /// Create a new Frenderer with the given window and runtime.
    pub fn with_runtime(window: &winit::window::Window, runtime: RT) -> Self {
        let gpu = runtime.run_future(WGPU::new(window));
        let sprites = SpriteRenderer::new(&gpu);
        Self {
            gpu,
            sprites,
            runtime,
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
    /// Create a [`wgpu::RenderPass`] and draw into it.  In the future
    /// this should take the [`wgpu::RenderPass`] or the queue/command
    /// encoder as a parameter and not present the surface on its own,
    /// so it can fit more smoothly into other rendering schemes.
    pub fn render(&self) {
        let frame = self
            .gpu
            .surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture");
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
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
                depth_stencil_attachment: None,
            });
            self.sprites.render(&mut rpass);
        }
        self.gpu.queue.submit(Some(encoder.finish()));
        frame.present();
    }
}
