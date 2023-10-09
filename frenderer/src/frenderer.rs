use crate::{sprites::SpriteRenderer, WGPU};
use winit::event::{Event, WindowEvent};

pub struct Frenderer<RT: super::Runtime> {
    pub gpu: WGPU,
    pub sprites: SpriteRenderer,
    pub window: winit::window::Window,
    runtime: RT,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn with_default_runtime(window: winit::window::Window) -> Frenderer<impl super::Runtime> {
    env_logger::init();
    Frenderer::with_runtime(window, super::PollsterRuntime {})
}
#[cfg(target_arch = "wasm32")]
pub fn with_default_runtime(window: winit::Window::Window) -> Frenderer<impl super::Runtime> {
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
    pub fn with_runtime(window: winit::window::Window, runtime: RT) -> Self {
        let gpu = runtime.run_future(WGPU::new(&window));
        let sprites = SpriteRenderer::new(&gpu);
        Self {
            gpu,
            sprites,
            window,
            runtime,
        }
    }
    pub fn block_on<F: std::future::Future>(&self, f: F) -> F::Output {
        self.runtime.run_future(f)
    }
    pub fn process_window_event<T>(&mut self, ev: &Event<T>) -> winit::event_loop::ControlFlow {
        match *ev {
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                self.gpu.resize(size);
                // On macos the window needs to be redrawn manually after resizing
                self.window.request_redraw();
                winit::event_loop::ControlFlow::Poll
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => winit::event_loop::ControlFlow::Exit,
            _ => winit::event_loop::ControlFlow::Poll,
        }
    }
    pub fn render(&self) -> std::time::Duration {
        let start = std::time::Instant::now();
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
        self.window.request_redraw();
        start.elapsed()
    }
}
