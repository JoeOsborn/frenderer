use std::sync::Arc;

use assets_manager::asset::Gltf;
use frenderer::{
    input::{self, Key},
    meshes::{Camera3D, MeshGroup, Transform3D},
};
use rand::Rng;
use ultraviolet::*;

//mod obj_loader;

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
        assets_manager::source::FileSystem::new("content").expect("Couldn't build asset source");
    #[cfg(target_arch = "wasm32")]
    let source = assets_manager::source::Embedded::from(assets_manager::source::embed!("content"));
    let cache = assets_manager::AssetCache::with_source(source);
    let fox = cache
        .load::<assets_manager::asset::Gltf>("khronos.Fox.glTF-Binary.Fox")
        .expect("Couldn't get fox asset");
    let raccoon = cache
        .load::<assets_manager::asset::Gltf>("low_poly_raccoon.scene")
        .expect("Couldn't get raccoon asset");

    let mut camera = Camera3D {
        translation: Vec3 {
            x: 0.0,
            y: 0.0,
            z: -10.0,
        }
        .into(),
        rotation: Rotor3::from_rotation_xz(0.0).into_quaternion_array(),
        // 90 degrees is typical
        fov: std::f32::consts::FRAC_PI_2,
        near: 1.0,
        far: 1000.0,
        aspect: 1024.0 / 768.0,
    };
    frend.mesh_set_camera(camera);
    frend.flat_set_camera(camera);

    let mut rng = rand::thread_rng();
    const COUNT: usize = 100;
    let fox = load_gltf_single_textured(&mut frend, &fox.read(), COUNT as u32);
    for trf in frend.meshes_mut(fox, 0, ..) {
        *trf = Transform3D {
            translation: Vec3 {
                x: rng.gen_range(-80.0..80.0),
                y: rng.gen_range(-60.0..60.0),
                z: rng.gen_range(-50.0..50.0),
            }
            .into(),
            rotation: Rotor3::from_euler_angles(
                rng.gen_range(0.0..std::f32::consts::TAU),
                rng.gen_range(0.0..std::f32::consts::TAU),
                rng.gen_range(0.0..std::f32::consts::TAU),
            )
            .into_quaternion_array(),
            scale: rng.gen_range(0.01..0.10),
        };
    }
    let raccoon = load_gltf_flat(&mut frend, &raccoon.read(), COUNT as u32);
    for trf in frend.flats_mut(raccoon, 0, ..) {
        *trf = Transform3D {
            translation: Vec3 {
                x: rng.gen_range(-80.0..80.0),
                y: rng.gen_range(-60.0..60.0),
                z: rng.gen_range(-50.0..50.0),
            }
            .into(),
            rotation: Rotor3::from_euler_angles(
                rng.gen_range(0.0..std::f32::consts::TAU),
                rng.gen_range(0.0..std::f32::consts::TAU),
                rng.gen_range(0.0..std::f32::consts::TAU),
            )
            .into_quaternion_array(),
            scale: rng.gen_range(3.0..6.0),
        };
    }

    const DT: f32 = 1.0 / 60.0;
    const DT_FUDGE_AMOUNT: f32 = 0.0002;
    const DT_MAX: f32 = DT * 5.0;
    const TIME_SNAPS: [f32; 5] = [15.0, 30.0, 60.0, 120.0, 144.0];
    let mut acc = 0.0;
    let mut now = frenderer::clock::Instant::now();
    event_loop
        .run(move |event, target| {
            use winit::event::{Event, WindowEvent};
            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    target.exit();
                }
                Event::WindowEvent {
                    event: winit::event::WindowEvent::RedrawRequested,
                    ..
                } => {
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
                    now = frenderer::clock::Instant::now();
                    // While we have time to spend
                    while acc >= DT {
                        // simulate a frame
                        acc -= DT;
                        // rotate every fox a random amount
                        for trf in frend.meshes_mut(fox, 0, ..) {
                            trf.rotation = (Rotor3::from_quaternion_array(trf.rotation)
                                * Rotor3::from_euler_angles(
                                    rng.gen_range(0.0..(std::f32::consts::TAU * DT)),
                                    rng.gen_range(0.0..(std::f32::consts::TAU * DT)),
                                    rng.gen_range(0.0..(std::f32::consts::TAU * DT)),
                                ))
                            .into_quaternion_array();
                            trf.translation[1] += 5.0 * DT;
                        }
                        let (mx, my): (f32, f32) = input.mouse_delta().into();
                        let mut rot = Rotor3::from_quaternion_array(camera.rotation)
                            * Rotor3::from_rotation_xz(mx * std::f32::consts::FRAC_PI_4 * DT)
                            * Rotor3::from_rotation_yz(my * std::f32::consts::FRAC_PI_4 * DT);
                        rot.normalize();
                        camera.rotation = rot.into_quaternion_array();
                        let dx = input.key_axis(Key::KeyA, Key::KeyD);
                        let dz = input.key_axis(Key::KeyW, Key::KeyS);
                        let mut dir = Vec3 {
                            x: dx,
                            y: 0.0,
                            z: dz,
                        };
                        let here = if dir.mag_sq() > 0.0 {
                            dir.normalize();
                            Vec3::from(camera.translation) + rot * dir * 80.0 * DT
                        } else {
                            Vec3::from(camera.translation)
                        };
                        camera.translation = here.into();
                        //println!("tick");
                        //update_game();
                        // camera.screen_pos[0] += 0.01;
                        input.next_frame();
                    }
                    // Render prep
                    frend.mesh_set_camera(camera);
                    frend.flat_set_camera(camera);
                    // update sprite positions and sheet regions
                    // ok now render.
                    frend.render();
                    window.request_redraw();
                }
                Event::WindowEvent {
                    event: winit::event::WindowEvent::Resized(size),
                    ..
                } => {
                    if !frend.gpu.is_web() {
                        frend.resize_surface(size.width, size.height);
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

fn load_gltf_single_textured(
    frend: &mut frenderer::Renderer,
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

    let tex = frend.create_array_texture(
        &[&img.to_rgba8()],
        frenderer::wgpu::TextureFormat::Rgba8Unorm,
        (img.width(), img.height()),
        None,
    );
    frend.mesh_group_add(
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

fn load_gltf_flat(frend: &mut frenderer::Renderer, asset: &Gltf, instance_count: u32) -> MeshGroup {
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
            let vtx_base_supported = !(frend.gpu.is_gl() || frend.gpu.is_web());
            match reader.read_indices() {
                None if vtx_base_supported => indices.extend(0..(verts.len() - vtx_old_len) as u32),
                Some(index_reader) if vtx_base_supported => indices.extend(index_reader.into_u32()),
                None => indices.extend((vtx_old_len as u32)..verts.len() as u32),
                Some(index_reader) => {
                    indices.extend(index_reader.into_u32());
                    if indices[idx_old_len..]
                        .iter()
                        .any(|&idx| idx < vtx_old_len as u32)
                    {
                        for idx in indices[idx_old_len..].iter_mut() {
                            *idx += vtx_old_len as u32;
                        }
                    }
                }
            };
            let vertex_base = if vtx_base_supported {
                vtx_old_len as i32
            } else {
                0
            };
            entry.submeshes.push(frenderer::meshes::SubmeshData {
                indices: idx_old_len as u32..(indices.len() as u32),
                vertex_base,
            })
        }
        assert!(!entry.submeshes.is_empty());
        entries.push(entry);
    }
    frend.flat_group_add(&mats, verts, indices, entries)
}
