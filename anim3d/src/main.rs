#![allow(dead_code)]

use frenderer::animation::{AnimationSettings, AnimationState};
use frenderer::assets::AnimRef;
use frenderer::camera::{Camera, Projection};
use frenderer::renderer::billboard::{BlendMode as FBlend, SingleRenderState as FBillboard};
use frenderer::renderer::flat::SingleRenderState as FFlat;
use frenderer::renderer::skinned::SingleRenderState as FSkinned;
use frenderer::renderer::sprites::SingleRenderState as FSprite;
use frenderer::renderer::textured::SingleRenderState as FTextured;
use frenderer::types::*;
use frenderer::{Engine, FrendererSettings, Key, Result, SpriteRendererSettings};
use std::rc::Rc;

const DT: f64 = 1.0 / 60.0;

struct GameObject {
    trf: Similarity3,
    model: Rc<frenderer::renderer::skinned::Model>,
    animation: AnimRef,
    state: AnimationState,
}
impl GameObject {
    fn tick_animation(&mut self) {
        self.state.tick(DT);
    }
}
struct Sprite {
    trf: Isometry3,
    tex: frenderer::assets::TextureRef,
    cel: Rect,
    size: Vec2,
}
struct World {
    camera: Camera,
    things: Vec<GameObject>,
    sprites: Vec<Sprite>,
    flats: Vec<Flat>,
    textured: Vec<Textured>,
}
struct Flat {
    trf: Similarity3,
    model: Rc<frenderer::renderer::flat::Model>,
}
struct Textured {
    trf: Similarity3,
    model: Rc<frenderer::renderer::textured::Model>,
}
impl frenderer::World for World {
    fn update(&mut self, input: &frenderer::Input, _assets: &mut frenderer::assets::Assets) {
        let yaw = input.key_axis(Key::Q, Key::W) * PI / 4.0 * DT as f32;
        let pitch = input.key_axis(Key::A, Key::S) * PI / 4.0 * DT as f32;
        let roll = input.key_axis(Key::Z, Key::X) * PI / 4.0 * DT as f32;
        let dscale = input.key_axis(Key::E, Key::R) * 1.0 * DT as f32;
        let rot = Rotor3::from_euler_angles(roll, pitch, yaw);
        for obj in self.things.iter_mut() {
            obj.trf.append_rotation(rot);
            obj.trf.scale = (obj.trf.scale + dscale).max(0.01);
            // dbg!(obj.trf.rotation);
            obj.tick_animation();
        }

        for s in self.sprites.iter_mut() {
            s.trf.append_rotation(rot);
            s.size.x += dscale;
            s.size.y += dscale;
        }
        for m in self.flats.iter_mut() {
            m.trf.append_rotation(rot);
            m.trf.scale += dscale;
        }
        for m in self.textured.iter_mut() {
            m.trf.append_rotation(rot);
            m.trf.scale += dscale;
        }
        let camera_drot = input.key_axis(Key::Left, Key::Right) * PI / 4.0 * DT as f32;
        self.camera
            .transform
            .prepend_rotation(Rotor3::from_rotation_xz(camera_drot));
    }
    fn render(
        &mut self,
        _a: &mut frenderer::assets::Assets,
        rs: &mut frenderer::renderer::RenderState,
    ) {
        rs.set_camera(self.camera);
        for (obj_i, obj) in self.things.iter_mut().enumerate() {
            rs.render_skinned(
                obj_i,
                obj.model.clone(),
                FSkinned::new(obj.animation, obj.state, obj.trf),
            );
        }
        for (s_i, s) in self.sprites.iter_mut().enumerate() {
            rs.render_sprite(s_i, s.tex, FSprite::new(s.cel, s.trf, s.size));
        }
        for (s_i, s) in self.sprites.iter_mut().enumerate() {
            use rand::prelude::*;
            let mut rng = rand::thread_rng();
            for i in 0..8 {
                let random_offset = Vec3::new(rng.gen(), rng.gen(), rng.gen());
                let random_rot: f32 = rng.gen::<f32>() * PI / 4.0 - PI / 8.0;
                let mut random_color: [u8; 4] = rng.gen::<[u8; 4]>();
                for c in random_color.iter_mut() {
                    *c = (*c).max(32);
                }
                rs.render_billboard(
                    s_i * 16 + i,
                    (s.tex, FBlend::Additive),
                    FBillboard::new(
                        s.cel,
                        s.trf.translation + random_offset,
                        random_rot,
                        s.size,
                        random_color,
                    ),
                );
            }
        }
        for (m_i, m) in self.flats.iter_mut().enumerate() {
            rs.render_flat(m_i, m.model.clone(), FFlat::new(m.trf));
        }
        for (t_i, t) in self.textured.iter_mut().enumerate() {
            rs.render_textured(t_i, t.model.clone(), FTextured::new(t.trf));
        }
    }
}
fn main() -> Result<()> {
    frenderer::color_eyre::install()?;

    let mut engine: Engine = Engine::new(
        FrendererSettings {
            sprite: SpriteRendererSettings {
                cull_back_faces: false,
                ..SpriteRendererSettings::default()
            },
            ..FrendererSettings::default()
        },
        DT,
    );

    let camera = Camera::look_at(
        Vec3::new(0.0, 0.0, 100.0),
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::unit_y(),
        Projection::Perspective { fov: PI / 2.0 },
    );

    // let camera = Camera::look_at(
    //     Vec3::new(0.0, 500.0, 0.0),
    //     Vec3::new(0.0, 0.0, 0.0),
    //     -Vec3::unit_z(),
    //     Projection::Orthographic {
    //         width: 1000.0,
    //         depth: 1000.0,
    //     },
    // );

    // let camera = Camera::from_transform(
    //     Similarity3::new(
    //         Vec3::new(0.0, 0.0, -500.0),
    //         Rotor3::from_rotation_yz(PI / 2.0),
    //         1.0,
    //     ),
    //     Projection::Orthographic {
    //         width: 1000.0,
    //         depth: 1000.0,
    //     },
    // );

    let marble_tex = engine
        .assets()
        .load_texture(std::path::Path::new("content/sphere-diffuse.jpg"))?;
    let marble_meshes = engine
        .assets()
        .load_textured(std::path::Path::new("content/sphere.obj"))?;
    let marble = engine
        .assets()
        .create_textured_model(marble_meshes, vec![marble_tex]);
    let floor_tex = engine
        .assets()
        .load_texture(std::path::Path::new("content/cube-diffuse.jpg"))?;
    let floor_meshes = engine
        .assets()
        .load_textured(std::path::Path::new("content/floor.obj"))?;
    let floor = engine
        .assets()
        .create_textured_model(floor_meshes, vec![floor_tex]);
    let king = engine
        .assets()
        .load_texture(std::path::Path::new("content/king.png"))?;
    let tex = engine
        .assets()
        .load_texture(std::path::Path::new("content/robot.png"))?;
    let meshes = engine.assets().load_skinned(
        std::path::Path::new("content/characterSmall.fbx"),
        &["RootNode", "Root"],
    )?;
    let animation = engine.assets().load_anim(
        std::path::Path::new("content/anim/run.fbx"),
        meshes[0],
        AnimationSettings { looping: true },
        "Root|Run",
    )?;
    assert_eq!(meshes.len(), 1);
    let model = engine.assets().create_skinned_model(meshes, vec![tex]);
    let flat_model = engine
        .assets()
        .load_flat(std::path::Path::new("content/windmill.glb"))?;
    let world = World {
        camera,
        things: vec![GameObject {
            trf: Similarity3::new(Vec3::new(-20.0, -15.0, -10.0), Rotor3::identity(), 0.1),
            model,
            animation,
            state: AnimationState { t: 0.0 },
        }],
        sprites: vec![Sprite {
            trf: Isometry3::new(Vec3::new(20.0, 5.0, -10.0), Rotor3::identity()),
            size: Vec2::new(16.0, 16.0),
            cel: Rect::new(0.5, 0.0, 0.5, 0.5),
            tex: king,
        }],
        flats: vec![Flat {
            trf: Similarity3::new(Vec3::new(0.0, 0.0, -10.0), Rotor3::identity(), 1.0),
            model: flat_model,
        }],
        textured: vec![
            Textured {
                trf: Similarity3::new(Vec3::new(0.0, 0.0, -10.0), Rotor3::identity(), 5.0),
                model: marble,
            },
            Textured {
                trf: Similarity3::new(Vec3::new(0.0, -25.0, 0.0), Rotor3::identity(), 10.0),
                model: floor,
            },
        ],
    };
    engine.play(world)
}
