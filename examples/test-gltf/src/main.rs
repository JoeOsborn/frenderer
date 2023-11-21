use assets_manager::asset::Gltf;
use frenderer::{input, meshes::MeshGroup, Camera3D, Transform3D};
use rand::Rng;
use ultraviolet::*;
use winit::event::VirtualKeyCode;

mod obj_loader;

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
    let raccoon = cache
        .load::<assets_manager::asset::Gltf>("low_poly_raccoon.scene")
        .unwrap();

    let mut camera = Camera3D {
        translation: Vec3 {
            x: 0.0,
            y: 0.0,
            z: -100.0,
        }
        .into(),
        rotation: Rotor3::from_rotation_xz(0.0).into_quaternion_array(),
        // 90 degrees is typical
        fov: std::f32::consts::FRAC_PI_2,
        near: 10.0,
        far: 1000.0,
        aspect: 1024.0 / 768.0,
    };
    frend.meshes.set_camera(&frend.gpu, camera);
    frend.flats.set_camera(&frend.gpu, camera);

    let mut rng = rand::thread_rng();
    const COUNT: usize = 100;
    let fox = load_gltf_single_textured(&mut frend, &fox.read(), COUNT as u32);
    for trf in frend.meshes.get_meshes_mut(fox, 0) {
        *trf = Transform3D {
            translation: Vec3 {
                x: rng.gen_range(-800.0..800.0),
                y: rng.gen_range(-600.0..600.0),
                z: rng.gen_range(-500.0..-100.0),
            }
            .into(),
            rotation: Rotor3::from_euler_angles(
                rng.gen_range(0.0..std::f32::consts::TAU),
                rng.gen_range(0.0..std::f32::consts::TAU),
                rng.gen_range(0.0..std::f32::consts::TAU),
            )
            .into_quaternion_array(),
            scale: rng.gen_range(0.5..1.0),
        };
    }
    frend.meshes.upload_meshes(&frend.gpu, fox, 0, ..);
    let raccoon = load_gltf_flat(&mut frend, &raccoon.read(), COUNT as u32);
    for trf in frend.flats.get_meshes_mut(raccoon, 0) {
        *trf = Transform3D {
            translation: Vec3 {
                x: rng.gen_range(-800.0..800.0),
                y: rng.gen_range(-600.0..600.0),
                z: rng.gen_range(-500.0..-100.0),
            }
            .into(),
            rotation: Rotor3::from_euler_angles(
                rng.gen_range(0.0..std::f32::consts::TAU),
                rng.gen_range(0.0..std::f32::consts::TAU),
                rng.gen_range(0.0..std::f32::consts::TAU),
            )
            .into_quaternion_array(),
            scale: rng.gen_range(24.0..32.0),
        };
    }
    frend.flats.upload_meshes(&frend.gpu, raccoon, 0, ..);

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
                // println!("{elapsed}");
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
                    for trf in frend.meshes.get_meshes_mut(fox, 0) {
                        trf.rotation = (Rotor3::from_quaternion_array(trf.rotation)
                            * Rotor3::from_euler_angles(
                                rng.gen_range(0.0..(std::f32::consts::TAU * DT)),
                                rng.gen_range(0.0..(std::f32::consts::TAU * DT)),
                                rng.gen_range(0.0..(std::f32::consts::TAU * DT)),
                            ))
                        .into_quaternion_array();
                        trf.translation[1] += 50.0 * DT;
                    }
                    let (mx, _my): (f32, f32) = input.mouse_delta().into();
                    let mut rot = Rotor3::from_quaternion_array(camera.rotation)
                        * Rotor3::from_rotation_xz(mx * std::f32::consts::FRAC_PI_4 * DT);
                    // let mut rot = Rotor3::from_quaternion_array(camera.rotation)
                    //     * (Rotor3::from_rotation_xz(
                    //         std::f32::consts::FRAC_PI_2
                    //             * if input.is_key_pressed(VirtualKeyCode::R) {
                    //                 1.0
                    //             } else {
                    //                 0.0
                    //             },
                    //     ));
                    rot.normalize();
                    camera.rotation = rot.into_quaternion_array();
                    let dx = input.key_axis(VirtualKeyCode::A, VirtualKeyCode::D);
                    let dz = input.key_axis(VirtualKeyCode::W, VirtualKeyCode::S);
                    let mut dir = Vec3 {
                        x: dx,
                        y: 0.0,
                        z: dz,
                    };
                    let here = if dir.mag_sq() > 0.0 {
                        dir.normalize();
                        Vec3::from(camera.translation) + rot * dir * 200.0 * DT
                    } else {
                        Vec3::from(camera.translation)
                    };
                    dbg!(rot.into_angle_plane().0);
                    dbg!(dir, here);
                    camera.translation = here.into();
                    frend.meshes.upload_meshes(&frend.gpu, fox, 0, ..);
                    //println!("tick");
                    //update_game();
                    // camera.screen_pos[0] += 0.01;
                    input.next_frame();
                }
                // Render prep
                frend.meshes.set_camera(&frend.gpu, camera);
                frend.flats.set_camera(&frend.gpu, camera);
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

fn load_gltf_single_textured(
    frend: &mut frenderer::Frenderer,
    asset: &Gltf,
    instance_count: u32,
) -> MeshGroup {
    let img = asset.get_image_by_index(0);
    let prim = asset
        .document
        .meshes()
        .next()
        .unwrap()
        .primitives()
        .next()
        .unwrap();
    let reader = prim.reader(|b| Some(asset.get_buffer_by_index(b.index())));
    let verts: Vec<_> = reader
        .read_positions()
        .unwrap()
        .zip(reader.read_tex_coords(0).unwrap().into_f32())
        .map(|(position, uv)| frenderer::meshes::Vertex::new(position, uv, 0))
        .collect();
    let vert_count = verts.len();

    let tex = frend.gpu.create_array_texture(
        &[&img.to_rgba8()],
        frenderer::wgpu::TextureFormat::Rgba8Unorm,
        (img.width(), img.height()),
        None,
    );
    frend.meshes.add_mesh_group(
        &frend.gpu,
        &tex,
        verts,
        (0..vert_count as u32).collect(),
        vec![frenderer::meshes::MeshEntry {
            instance_count,
            submeshes: vec![frenderer::meshes::SubmeshEntry {
                vertex_base: 0,
                indices: 0..vert_count as u32,
            }],
        }],
    )
}

fn load_gltf_flat(
    frend: &mut frenderer::Frenderer,
    asset: &Gltf,
    instance_count: u32,
) -> MeshGroup {
    let mut mats: Vec<_> = asset
        .document
        .materials()
        .map(|m| m.pbr_metallic_roughness().base_color_factor())
        .collect();
    if mats.is_empty() {
        mats.push([1.0, 0.0, 0.0, 1.0]);
    }
    let mut verts = Vec::with_capacity(1024);
    let mut indices = Vec::with_capacity(1024);
    let mut entries = Vec::with_capacity(1);
    for mesh in asset.document.meshes() {
        let mut entry = frenderer::meshes::MeshEntry {
            instance_count,
            submeshes: Vec::with_capacity(1),
        };
        for prim in mesh.primitives() {
            let reader = prim.reader(|b| Some(asset.get_buffer(&b)));
            let vtx_old_len = verts.len();
            assert_eq!(prim.mode(), gltf::mesh::Mode::Triangles);
            verts.extend(reader.read_positions().unwrap().map(|position| {
                frenderer::meshes::FlatVertex::new(
                    position,
                    prim.material().index().unwrap_or(0) as u32,
                )
            }));
            let idx_old_len = indices.len();
            match reader.read_indices() {
                None => indices.extend(0..(verts.len() - vtx_old_len) as u32),
                Some(index_reader) => indices.extend(index_reader.into_u32()),
            };
            entry.submeshes.push(frenderer::meshes::SubmeshData {
                indices: idx_old_len as u32..(indices.len() as u32),
                vertex_base: vtx_old_len as i32,
            })
        }
        assert!(!entry.submeshes.is_empty());
        entries.push(entry);
    }
    frend
        .flats
        .add_mesh_group(&frend.gpu, &mats, verts, indices, entries)
}
