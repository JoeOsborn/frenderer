use assets_manager::asset::Png;
use std::sync::Arc;

use frenderer::{
    input,
    sprites::{Camera2D, SheetRegion, Transform},
    wgpu,
};
use rand::Rng;

fn main() {
    frenderer::with_default_runtime(winit::window::WindowBuilder::new(), Some((1024, 768)), run)
        .unwrap();
    // instead of the above, we could have created the wgpu device/adapter ourselves, made a frenderer::WGPU, and then made a frenderer with that and the window.
}

fn run(
    event_loop: winit::event_loop::EventLoop<()>,
    window: Arc<winit::window::Window>,
    mut frend: frenderer::Renderer,
) {
    let mut input = input::Input::default();

    #[cfg(not(target_arch = "wasm32"))]
    let source =
        assets_manager::source::FileSystem::new("content").expect("Couldn't load resources");
    #[cfg(target_arch = "wasm32")]
    let source = assets_manager::source::Embedded::from(assets_manager::source::embed!("content"));
    let cache = assets_manager::AssetCache::with_source(source);

    let sprite_img_handle = cache.load::<Png>("king").expect("Couldn't load king img");
    let sprite_img = sprite_img_handle.read().0.to_rgba8();

    let sprite_invert_img_handle = cache
        .load::<Png>("king_invert")
        .expect("Couldn't load inverse king");
    let sprite_invert_img = sprite_invert_img_handle.read().0.to_rgba8();

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
    frend.sprite_group_add(
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
            .map(|_n| SheetRegion::new(0, 0, 16, 0, 11, 16))
            .collect(),
        camera,
    );

    const DT: f32 = 1.0 / 60.0;
    const DT_FUDGE_AMOUNT: f32 = 0.0002;
    const DT_MAX: f32 = DT * 5.0;
    const TIME_SNAPS: [f32; 5] = [15.0, 30.0, 60.0, 120.0, 144.0];
    let mut acc = 0.0;
    let mut now = frenderer::clock::Instant::now();
    event_loop
        .run(move |event, target| {
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
                    frend.resize_surface(size.width, size.height);
                    window.request_redraw();
                }
                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    // Render prep
                    frend.sprite_group_set_camera(0, camera);
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
                    //             view: frend.depth_texture_view(),
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
                    now = frenderer::clock::Instant::now();
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
        })
        .unwrap();
}
