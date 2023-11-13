use assets_manager::source::Source;
use frenderer::{input, Camera3D, Transform3D};
use rand::Rng;
use ultraviolet::*;

struct Obj {
    pub models: Vec<tobj::Model>,
    pub materials: Vec<tobj::Material>,
}
#[allow(clippy::upper_case_acronyms)]
struct TMTL(tobj::MTLLoadResult);
struct MtlLoader();
impl assets_manager::Asset for TMTL {
    type Loader = MtlLoader;
    const EXTENSION: &'static str = "mtl";
}
impl assets_manager::loader::Loader<TMTL> for MtlLoader {
    fn load(
        content: std::borrow::Cow<[u8]>,
        _ext: &str,
    ) -> Result<TMTL, assets_manager::BoxedError> {
        let mut reader = std::io::BufReader::new(content.as_ref());
        Ok(TMTL(tobj::load_mtl_buf(&mut reader)))
    }
}

impl Obj {
    fn textures(&self) -> impl Iterator<Item = &String> {
        self.materials.iter().flat_map(|mtl| {
            mtl.ambient_texture
                .iter()
                .chain(mtl.diffuse_texture.iter())
                .chain(mtl.specular_texture.iter())
                .chain(mtl.normal_texture.iter())
                .chain(mtl.shininess_texture.iter())
                .chain(mtl.dissolve_texture.iter())
        })
    }
}
fn path_to_id(base_id: &str, path: &std::path::Path) -> (String, String) {
    let ext = path
        .extension()
        .map(|osstr| osstr.to_string_lossy())
        .unwrap_or_default();
    let path = path.with_extension("");
    let mut path_id = String::with_capacity(path.capacity() + base_id.len() + 1);
    if base_id.is_empty() {
        path_id += base_id;
        path_id += ".";
    }
    let mut comps = path.components().peekable();
    while let Some(comp) = comps.next() {
        path_id += &comp.as_os_str().to_string_lossy();
        if comps.peek().is_some() {
            path_id += ".";
        }
    }
    (path_id, ext.into_owned())
}
impl assets_manager::Compound for Obj {
    fn load(
        cache: assets_manager::AnyCache,
        id: &assets_manager::SharedString,
    ) -> Result<Self, assets_manager::BoxedError> {
        let source = cache.raw_source();
        let base_id = match id.rfind('.') {
            Some(index) => &id[..index],
            None => "",
        };
        let obj_bytes = source.read(id, "obj")?;
        let (models, materials) = tobj::load_obj_buf(
            &mut std::io::BufReader::new(obj_bytes.as_ref()),
            &tobj::LoadOptions {
                single_index: true,
                ..Default::default()
            },
            |path| {
                let (path_id, _ext) = path_to_id(base_id, path);
                cache.load::<TMTL>(&path_id).unwrap().read().0.clone()
            },
        )?;
        let mut obj = Obj {
            models,
            materials: materials.map_err(assets_manager::BoxedError::from)?,
        };
        // replace paths in texture names with asset IDs
        for mtl in obj.materials.iter_mut() {
            mtl.ambient_texture
                .iter_mut()
                .chain(mtl.diffuse_texture.iter_mut())
                .chain(mtl.specular_texture.iter_mut())
                .chain(mtl.normal_texture.iter_mut())
                .chain(mtl.shininess_texture.iter_mut())
                .chain(mtl.dissolve_texture.iter_mut())
                .try_for_each(|tex| {
                    let (path_id, _ext) = path_to_id(base_id, std::path::Path::new(tex));
                    // also load the image to mark it as a dependency
                    let ret = cache
                        .load::<assets_manager::asset::Png>(&path_id)
                        .map(|_| ());
                    *tex = path_id;
                    ret
                })?;
        }
        Ok(obj)
    }
}

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

    let camera = Camera3D {
        translation: Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
        .into(),
        rotation: Rotor3::identity().into_quaternion_array(),
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
        .map(|(position, uv)| frenderer::meshes::Vertex {
            position,
            uv,
            which: 0,
        })
        .collect();
    let vert_count = verts.len();

    /* TODO: also load an obj and show how that could be added to the
     * existing verts and indices and textures, to illustrate that you
     * can load multiple models into one group */

    let fox_tex = frend.gpu.create_array_texture(
        &[&fox_img.to_rgba8()],
        frenderer::wgpu::TextureFormat::Rgba8Unorm,
        (fox_img.width(), fox_img.height()),
        Some("fox texture"),
    );
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
                    for trf in frend.meshes.get_meshes_mut(fox_mesh, 0) {
                        trf.rotation = (Rotor3::from_quaternion_array(trf.rotation)
                            * Rotor3::from_euler_angles(
                                rng.gen_range(0.0..(std::f32::consts::TAU * DT)),
                                rng.gen_range(0.0..(std::f32::consts::TAU * DT)),
                                rng.gen_range(0.0..(std::f32::consts::TAU * DT)),
                            ))
                        .into_quaternion_array();
                        trf.translation[1] += 50.0 * DT;
                    }
                    // camera.translation[2] -= 100.0 * DT;
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
