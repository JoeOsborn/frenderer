use frenderer::{input, wgpu, GPUCamera, Region};
use rand::Rng;

fn main() {
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::Window::new(&event_loop).unwrap();
    let mut frend = frenderer::with_default_runtime(&window);
    let mut input = input::Input::default();
    // init game code here
    let (sprite_tex, _sprite_img) = frend.block_on(async {
        frend
            .gpu
            .load_texture(std::path::Path::new("content/king.png"), None)
            .await
            .expect("Couldn't load spritesheet texture")
    });
    let mut camera = GPUCamera {
        screen_pos: [0.0, 0.0],
        screen_size: [1024.0, 768.0],
    };

    let mut rng = rand::thread_rng();
    const COUNT: usize = 100_000;
    frend.sprites.add_sprite_group(
        &frend.gpu,
        sprite_tex,
        (0..COUNT)
            .map(|_n| Region {
                x: rng.gen_range(0.0..(camera.screen_size[0] - 16.0)),
                y: rng.gen_range(0.0..(camera.screen_size[1] - 16.0)),
                w: 16.0,
                h: 16.0,
            })
            .collect(),
        vec![
            Region {
                x: 0.0,
                y: 0.5,
                w: 0.5,
                h: 0.5
            };
            COUNT
        ],
        camera,
    );
    const DT: f32 = 1.0 / 60.0;
    const DT_FUDGE_AMOUNT: f32 = 0.0002;
    const DT_MAX: f32 = DT * 5.0;
    const TIME_SNAPS: [f32; 4] = [15.0, 30.0, 60.0, 120.0];
    let mut acc = 0.0;
    let mut now = std::time::Instant::now();
    event_loop.run(move |event, _, control_flow| {
        use winit::event::{Event, WindowEvent};
        control_flow.set_poll();
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = winit::event_loop::ControlFlow::Exit;
            }
            Event::MainEventsCleared => {
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
                // Render prep
                frend.sprites.set_camera_all(&frend.gpu, camera);
                // update sprite positions and sheet regions
                // ok now render.
                // We could just call frend.render();.
                // Or we could do this to integrate frenderer into a larger system.
                // (This first call isn't necessary if we make our own framebuffer/view and encoder)
                let (frame, view, mut encoder) = frend.render_setup();
                {
                    // This is us manually making a renderpass
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: None,
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: frenderer::wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: true,
                            },
                        })],
                        depth_stencil_attachment: None,
                    });
                    // frend has render_into to do the actual rendering
                    frend.render_into(&mut rpass);
                }
                // This just submits the command encoder and presents the frame, we wouldn't need it if we did that some other way.
                frend.render_finish(frame, encoder);
                window.request_redraw();
            }
            event => {
                if frend.process_window_event(&event) {
                    window.request_redraw();
                }
                input.process_input_event(&event);
            }
        }
    });
}
