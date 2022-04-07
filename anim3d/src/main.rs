#![allow(dead_code)]

use frenderer::animation::{AnimationSettings, AnimationState};
use frenderer::assets::AnimRef;
use frenderer::camera::Camera;
use frenderer::types::*;
use frenderer::{Engine, Key, Result, WindowSettings};
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
        for obj in self.things.iter_mut() {
            let yaw = if input.is_key_down(Key::Z) {
                (PI / 4.0) * (1.0 / 60.0)
            } else {
                0.0
            };
            let pitch = if input.is_key_down(Key::X) {
                (PI / 4.0) * (1.0 / 60.0)
            } else {
                0.0
            };
            let roll = if input.is_key_down(Key::C) {
                (PI / 4.0) * (1.0 / 60.0)
            } else {
                0.0
            };
            let dscale = if input.is_key_down(Key::Up) {
                1.0 / 60.0
            } else {
                0.0
            };
            obj.trf.rotation = Rotor3::from_euler_angles(roll, pitch, yaw) * obj.trf.rotation;
            obj.trf.scale += dscale;
            // dbg!(obj.trf.rotation);
            obj.tick_animation();
        }
        for s in self.sprites.iter_mut() {
            let yaw = if input.is_key_down(Key::A) {
                (PI / 4.0) * (1.0 / 60.0)
            } else {
                0.0
            };
            let pitch = if input.is_key_down(Key::S) {
                (PI / 4.0) * (1.0 / 60.0)
            } else {
                0.0
            };
            let roll = if input.is_key_down(Key::D) {
                (PI / 4.0) * (1.0 / 60.0)
            } else {
                0.0
            };
            let dscale = if input.is_key_down(Key::F) {
                1.0 / 60.0
            } else {
                0.0
            };
            s.trf.rotation = Rotor3::from_euler_angles(roll, pitch, yaw) * s.trf.rotation;
            s.size.x += dscale;
            s.size.y += dscale;
        }
        for m in self.flats.iter_mut() {
            let yaw = if input.is_key_down(Key::Q) {
                (PI / 4.0) * (1.0 / 60.0)
            } else {
                0.0
            };
            let pitch = if input.is_key_down(Key::W) {
                (PI / 4.0) * (1.0 / 60.0)
            } else {
                0.0
            };
            let roll = if input.is_key_down(Key::E) {
                (PI / 4.0) * (1.0 / 60.0)
            } else {
                0.0
            };
            let dscale = if input.is_key_down(Key::R) {
                1.0 / 60.0
            } else {
                0.0
            };
            m.trf.rotation = Rotor3::from_euler_angles(roll, pitch, yaw) * m.trf.rotation;
            m.trf.scale += dscale;
        }
        for m in self.textured.iter_mut() {
            let yaw = if input.is_key_down(Key::U) {
                (PI / 4.0) * (1.0 / 60.0)
            } else {
                0.0
            };
            let pitch = if input.is_key_down(Key::I) {
                (PI / 4.0) * (1.0 / 60.0)
            } else {
                0.0
            };
            let roll = if input.is_key_down(Key::O) {
                (PI / 4.0) * (1.0 / 60.0)
            } else {
                0.0
            };
            let dscale = if input.is_key_down(Key::P) {
                1.0 / 60.0
            } else {
                0.0
            };
            m.trf.rotation = Rotor3::from_euler_angles(roll, pitch, yaw) * m.trf.rotation;
            m.trf.scale += dscale;
        }
    }
    fn render(
        &mut self,
        _a: &mut frenderer::assets::Assets,
        rs: &mut frenderer::renderer::RenderState,
    ) {
        for (obj_i, obj) in self.things.iter_mut().enumerate() {
            rs.render_skinned(obj.model.clone(), obj.animation, obj.state, obj.trf, obj_i);
        }
        for (s_i, s) in self.sprites.iter_mut().enumerate() {
            rs.render_sprite(s.tex, s.cel, s.trf, s.size, s_i);
        }
        for (m_i, m) in self.flats.iter_mut().enumerate() {
            rs.render_flat(m.model.clone(), m.trf, m_i);
        }
        for (t_i, t) in self.textured.iter_mut().enumerate() {
            rs.render_textured(t.model.clone(), t.trf, t_i);
        }
    }
}
fn main() -> Result<()> {
    frenderer::color_eyre::install()?;

    let mut engine: Engine = Engine::new(WindowSettings::default(), DT);

    engine.set_camera(Camera::look_at(
        Vec3::new(0., 0., 100.),
        Vec3::new(0., 0., 0.),
        Vec3::new(0., 1., 0.),
    ));

    let marble_tex = engine.load_texture(std::path::Path::new("content/sphere-diffuse.jpg"))?;
    let marble_meshes = engine.load_textured(std::path::Path::new("content/sphere.obj"))?;
    let marble = engine.create_textured_model(marble_meshes, vec![marble_tex]);
    let floor_tex = engine.load_texture(std::path::Path::new("content/cube-diffuse.jpg"))?;
    let floor_meshes = engine.load_textured(std::path::Path::new("content/floor.obj"))?;
    let floor = engine.create_textured_model(floor_meshes, vec![floor_tex]);
    let king = engine.load_texture(std::path::Path::new("content/king.png"))?;
    let tex = engine.load_texture(std::path::Path::new("content/robot.png"))?;
    let meshes = engine.load_skinned(
        std::path::Path::new("content/characterSmall.fbx"),
        &["RootNode", "Root"],
    )?;
    let animation = engine.load_anim(
        std::path::Path::new("content/anim/run.fbx"),
        meshes[0],
        AnimationSettings { looping: true },
        "Root|Run",
    )?;
    assert_eq!(meshes.len(), 1);
    let model = engine.create_skinned_model(meshes, vec![tex]);
    let flat_model = engine.load_flat(std::path::Path::new("content/windmill.glb"))?;
    let world = World {
        things: vec![GameObject {
            trf: Similarity3::new(Vec3::new(-20.0, -15.0, -10.0), Rotor3::identity(), 0.1),
            model,
            animation,
            state: AnimationState { t: 0.0 },
        }],
        sprites: vec![Sprite {
            trf: Isometry3::new(Vec3::new(20.0, 5.0, -10.0), Rotor3::identity()),
            size: Vec2::new(16.0, 16.0),
            cel: Rect::new(0.5, 0.5, 0.5, 0.5),
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
