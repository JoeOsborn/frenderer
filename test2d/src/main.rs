use frenderer::{input, GPUCamera, GPUSprite};
use rand::Rng;

fn main() {
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::Window::new(&event_loop).unwrap();
    let mut frend = frenderer::with_default_runtime(window);
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
    frend.sprites.add_sprite_group(
        &frend.gpu,
        sprite_tex,
        (0..10_000)
            .map(|_n| GPUSprite {
                screen_region: [
                    rng.gen_range(0.0..(camera.screen_size[0] - 16.0)),
                    rng.gen_range(0.0..(camera.screen_size[1] - 16.0)),
                    16.0,
                    16.0,
                ],
                sheet_region: [0.0, 0.5, 0.5, 0.5],
            })
            .collect(),
        camera,
    );
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
                // some number of times
                //update_game();
                camera.screen_pos[0] += 0.01;
                frend.sprites.set_camera_all(&frend.gpu, camera);
                input.next_frame();
                // ok now render
                let elapsed = frend.render();
                println!("{}", elapsed.as_secs_f32());
            }
            event => {
                frend.process_window_event(&event);
                input.process_input_event(&event);
            }
        }
    });
}
