use std::error::Error;

use frenderer::{input, wgpu, Camera2D, SheetRegion, Transform};
use rand::Rng;

fn main() -> Result<(), Box<dyn Error>> {
    let event_loop = winit::event_loop::EventLoop::new()?;
    let window = std::sync::Arc::new(winit::window::Window::new(&event_loop)?);
    let mut frend = frenderer::with_default_runtime(window.clone())?;
    let mut input = input::Input::default();
    // init game code here
    #[cfg(target_arch = "wasm32")]
    let sprite_img = {
        let img_bytes = include_bytes!("content/king.png");
        image::load_from_memory_with_format(&img_bytes, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?
            .into_rgba8()
    };
    #[cfg(not(target_arch = "wasm32"))]
    let sprite_img = image::open("content/king.png")?.into_rgba8();

    #[cfg(target_arch = "wasm32")]
    let sprite_invert_img = {
        let img_bytes = include_bytes!("content/king_invert.png");
        image::load_from_memory_with_format(&img_bytes, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?
            .into_rgba8()
    };
    #[cfg(not(target_arch = "wasm32"))]
    let sprite_invert_img = image::open("content/king_invert.png")?.into_rgba8();

    let sprite_tex = frend.create_array_texture(
        &[&sprite_img, &sprite_invert_img],
        wgpu::TextureFormat::Rgba8UnormSrgb,
        sprite_img.dimensions(),
        Some("spr-king.png"),
    );
    let mut camera = Camera2D {
        screen_pos: [0.0, 0.0],
        screen_size: [1024.0, 768.0],
    };

    let mut rng = rand::thread_rng();
    const COUNT: usize = 100_000;
    frend.sprites.add_sprite_group(
        &frend.gpu,
        &sprite_tex,
        (0..COUNT)
            .map(|_n| Transform {
                x: rng.gen_range(0.0..(camera.screen_size[0] - 16.0)),
                y: rng.gen_range(0.0..(camera.screen_size[1] - 16.0)),
                w: 11,
                h: 16,
                rot: rng.gen_range(0.0..(std::f32::consts::TAU)),
            })
            .collect(),
        (0..COUNT)
            .map(|_n| SheetRegion::new(rng.gen_range(0..=1), 0, 16, 0, 11, 16))
            .collect(),
        camera,
    );

    const DT: f32 = 1.0 / 60.0;
    const DT_FUDGE_AMOUNT: f32 = 0.0002;
    const DT_MAX: f32 = DT * 5.0;
    const TIME_SNAPS: [f32; 5] = [15.0, 30.0, 60.0, 120.0, 144.0];
    let mut acc = 0.0;
    let mut now = std::time::Instant::now();
    Ok(event_loop.run(move |event, target| {
        use winit::event::{Event, WindowEvent};
        target.set_control_flow(winit::event_loop::ControlFlow::Poll);
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                target.exit();
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                frend.resize(size.width, size.height);
                window.request_redraw();
            }
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                // Render prep
                frend.sprites.set_camera_all(&frend.gpu, camera);
                // update sprite positions and sheet regions
                // ok now render.
                frend.render();
                // Or we could do this to integrate frenderer into a larger system.
                // (This first call isn't necessary if we make our own framebuffer/view and encoder)
                // let (frame, view, mut encoder) = frend.render_setup();
                // {
                //     // This is us manually making a renderpass
                //     let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                //         label: None,
                //         color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                //             view: &view,
                //             resolve_target: None,
                //             ops: frenderer::wgpu::Operations {
                //                 load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                //                 store: true,
                //             },
                //         })],
                //         depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                //             view: &frend.gpu.depth_texture_view,
                //             depth_ops: Some(wgpu::Operations {
                //                 load: wgpu::LoadOp::Clear(1.0),
                //                 store: true,
                //             }),
                //             stencil_ops: None,
                //         }),
                //     });
                //     // frend has render_into to do the actual rendering
                //     frend.render_into(&mut rpass);
                // }
                // // This just submits the command encoder and presents the frame, we wouldn't need it if we did that some other way.
                // frend.render_finish(frame, encoder);
            }
            Event::AboutToWait => {
                // compute elapsed time since last frame
                let mut elapsed = now.elapsed().as_secs_f32();
                println!("{elapsed}");
                // snap time to nearby vsync framerate
                TIME_SNAPS.iter().for_each(|s| {
                    if (elapsed - 1.0 / s).abs() < DT_FUDGE_AMOUNT {
                        elapsed = 1.0 / s;
                    }
                });
                // Death spiral prevention
                if elapsed > DT_MAX {
                    acc = 0.0;
                    elapsed = DT;
                }
                acc += elapsed;
                now = std::time::Instant::now();
                // While we have time to spend
                while acc >= DT {
                    // simulate a frame
                    acc -= DT;
                    println!("tick");
                    //update_game();
                    camera.screen_pos[0] += 0.01;
                    input.next_frame();
                }
                window.request_redraw();
            }
            event => {
                input.process_input_event(&event);
            }
        }
    })?)
}
