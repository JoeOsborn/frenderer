use frenderer::{input, Camera3D, Transform3D};
use glam::*;
use rand::Rng;

fn main() {
    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::Window::new(&event_loop).unwrap();
    #[cfg(not(target_arch = "wasm32"))]
    let source = assets_manager::source::FileSystem::new("content").unwrap();
    #[cfg(target_arch = "wasm32")]
    let source = assets_manager::source::Embedded::from(source::embed!("content"));
    let cache = assets_manager::AssetCache::with_source(source);
    let mut frend = frenderer::with_default_runtime(&window);
    let mut input = input::Input::default();
    let fox = cache
        .load::<assets_manager::asset::Gltf>("khronos.Fox.glTF-Binary.Fox")
        .unwrap();

    let mut camera = Camera3D {
        translation: Vec3 {
            x: 0.0,
            y: 0.0,
            z: -100.0,
        }
        .into(),
        rotation: Quat::from_rotation_y(0.0).into(),
        // 90 degrees is typical
        fov: std::f32::consts::FRAC_PI_2,
        near: 10.0,
        far: 1000.0,
        aspect: 1024.0 / 768.0,
    };
    frend.meshes.set_camera(&frend.gpu, camera);

    let mut rng = rand::thread_rng();
    const COUNT: usize = 10;
    let fox = fox.read();
    let fox_img = fox.get_image_by_index(0);
    let fox_tex = frend.gpu.create_array_texture(
        &[&fox_img.to_rgba8()],
        frenderer::wgpu::TextureFormat::Rgba8Unorm,
        (fox_img.width(), fox_img.height()),
        Some("fox texture"),
    );
    let prim = fox
        .document
        .meshes()
        .next()
        .unwrap()
        .primitives()
        .next()
        .unwrap();
    let reader = prim.reader(|b| Some(fox.get_buffer_by_index(b.index())));
    let verts: Vec<_> = reader
        .read_positions()
        .unwrap()
        .zip(reader.read_tex_coords(0).unwrap().into_f32())
        .map(|(pos, tex)| frenderer::meshes::Vertex {
            position: pos.into(),
            uv: tex.into(),
            which: 0,
        })
        .collect();
    let vert_count = verts.len();
    let fox_mesh = frend.meshes.add_mesh_group(
        &frend.gpu,
        &fox_tex,
        verts,
        (0..vert_count as u32).collect(),
        vec![frenderer::meshes::MeshEntry {
            instance_count: COUNT as u32,
            submeshes: vec![frenderer::meshes::SubmeshEntry {
                vertex_base: 0,
                indices: 0..vert_count as u32,
            }],
        }],
    );
    for trf in frend.meshes.get_meshes_mut(fox_mesh, 0) {
        *trf = Transform3D {
            translation: Vec3 {
                x: rng.gen_range(-400.0..400.0),
                y: rng.gen_range(-300.0..300.0),
                z: rng.gen_range(100.0..500.0),
            }
            .into(),
            rotation: Quat::from_euler(
                EulerRot::XYZ,
                rng.gen_range(0.0..std::f32::consts::TAU),
                rng.gen_range(0.0..std::f32::consts::TAU),
                rng.gen_range(0.0..std::f32::consts::TAU),
            )
            .into(),
            scale: rng.gen_range(0.5..1.0),
        };
    }
    frend.meshes.upload_meshes(&frend.gpu, fox_mesh, 0, ..);
    const DT: f32 = 1.0 / 60.0;
    const DT_FUDGE_AMOUNT: f32 = 0.0002;
    const DT_MAX: f32 = DT * 5.0;
    const TIME_SNAPS: [f32; 5] = [15.0, 30.0, 60.0, 120.0, 144.0];
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
                    // rotate every fox a random amount
                    // for trf in frend.meshes.get_meshes_mut(fox_mesh, 0) {
                    //     trf.rotation = (Quat::from_array(trf.rotation)
                    //         * Quat::from_euler(
                    //             EulerRot::XYZ,
                    //             rng.gen_range(0.0..(std::f32::consts::TAU * DT)),
                    //             rng.gen_range(0.0..(std::f32::consts::TAU * DT)),
                    //             rng.gen_range(0.0..(std::f32::consts::TAU * DT)),
                    //         ))
                    //     .into();
                    // }
                    camera.translation[2] -= 100.0 * DT;
                    frend.meshes.upload_meshes(&frend.gpu, fox_mesh, 0, ..);
                    //println!("tick");
                    //update_game();
                    // camera.screen_pos[0] += 0.01;
                    input.next_frame();
                }
                // Render prep
                frend.meshes.set_camera(&frend.gpu, camera);
                // update sprite positions and sheet regions
                // ok now render.
                frend.render();
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
