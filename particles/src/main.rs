use std::cell::RefCell;
use std::rc::Rc;

use frenderer::camera::{Camera, Projection};
use frenderer::renderer::billboard::{BlendMode as Blend, SingleRenderState as RenderParticle};
use frenderer::renderer::textured;
use frenderer::types::*;
use frenderer::{Engine, FrendererSettings, Key, Result};
use rand::prelude::*;
const DT: f64 = 1.0 / 60.0;

// #[derive(Debug)]
struct ParticleSystem {
    transform: Isometry3,
    texture: frenderer::assets::TextureRef,
    emitter: EmitterShape,
    max_particles: usize,
    min_particles: usize,
    size_min: Vec2,
    size_adjust: Vec2,
    dsize_min: Vec2,
    dsize_adjust: Vec2,
    life_min: u16,
    life_adjust: u16,
    w_min: f32,
    w_adjust: f32,
    dw_min: f32,
    dw_adjust: f32,
    rot_range: f32,
    velocity_sampler: VectorSampler,
    acceleration_sampler: VectorSampler,
    color_interp: Vec<Color>,
    emit_rate: f32,
    next_emit: f32,
    // TODO replace with Vec<Rect> for UVs
    base_uv_cel: usize,
    uv_cel_count: usize,
    particles: Rc<RefCell<Vec<RenderParticle>>>,
    particle_states: Vec<ParticleState>,
}
struct VectorSampler {
    mag_range: (f32, f32),
    yz_range: (f32, f32),
    xz_range: (f32, f32),
}
impl VectorSampler {
    fn new(yz_range: (f32, f32), xz_range: (f32, f32), mag_range: (f32, f32)) -> Self {
        Self {
            yz_range,
            xz_range,
            mag_range,
        }
    }
    fn sample(&self, rng: &mut impl Rng) -> Vec3 {
        let (mag_min, mag_max) = self.mag_range;
        let (xz_min, xz_max) = self.xz_range;
        let (yz_min, yz_max) = self.yz_range;
        let mag = mag_min.lerp(mag_max, rng.gen::<f32>());
        let xz = xz_min.lerp(xz_max, rng.gen::<f32>());
        let yz = yz_min.lerp(yz_max, rng.gen::<f32>());
        let rot = Rotor3::from_rotation_xz(xz) * Rotor3::from_rotation_yz(yz);
        rot * (Vec3::unit_z() * mag)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct AABB {
    center: Vec3,
    extents: Vec3,
}
#[derive(Debug)]
enum EmitterShape {
    Point(Vec3),
    Box(AABB),
}
impl EmitterShape {
    fn sample(&self, rng: &mut impl Rng) -> Vec3 {
        match self {
            Self::Point(p) => *p,
            Self::Box(r) => Vec3::new(
                r.center.x + (rng.gen::<f32>() - 0.5) * r.extents.x,
                r.center.y + (rng.gen::<f32>() - 0.5) * r.extents.y,
                r.center.z + (rng.gen::<f32>() - 0.5) * r.extents.z,
            ),
        }
    }
}
// render particle has position, orientation, size, UV, color
// so here we store the ways those change
#[derive(Clone, Copy, Default, PartialEq, Debug)]
struct ParticleState {
    dsize: Vec2,
    t: u16,
    tmax: u16,
    w: f32,
    velocity: Vec3,
    dw: f32,
    acceleration: Vec3,
    // ...
}

impl ParticleSystem {
    fn new_smoke(texture: frenderer::assets::TextureRef, transform: Isometry3) -> Self {
        const MAX: usize = 128;
        Self {
            transform,
            emitter: EmitterShape::Point(Vec3::zero()),
            max_particles: MAX,
            emit_rate: 15.0,
            next_emit: 0.0,
            particles: Rc::new(RefCell::new(vec![RenderParticle::default(); MAX])),
            particle_states: vec![ParticleState::default(); MAX],
            texture,
            rot_range: 2.0 * PI,
            base_uv_cel: 12,
            uv_cel_count: 2,
            min_particles: 1.min(MAX),
            size_min: Vec2::new(32.0, 32.0),
            size_adjust: Vec2::zero(),
            dsize_min: Vec2::new(3.0, 3.0),
            dsize_adjust: Vec2::new(15.0, 15.0),
            life_min: 60,
            life_adjust: 240,
            w_min: 0.0,
            w_adjust: 0.0,
            dw_min: 0.0,
            dw_adjust: 0.0,
            velocity_sampler: VectorSampler::new(
                (-PI / 8.0, PI / 8.0),
                (-PI / 8.0, PI / 8.0),
                (8.0, 16.0),
            ),
            acceleration_sampler: VectorSampler::new((0.0, 0.0), (0.0, 0.0), (0.0, 0.0)),
            color_interp: vec![Color(128, 128, 128, 196), Color(128, 128, 128, 0)],
        }
    }
    fn new_hearts(texture: frenderer::assets::TextureRef, transform: Isometry3) -> Self {
        const MAX: usize = 256;
        Self {
            transform,
            emitter: EmitterShape::Point(Vec3::zero()),
            max_particles: MAX,
            emit_rate: 15.0,
            next_emit: 0.0,
            particles: Rc::new(RefCell::new(vec![RenderParticle::default(); MAX])),
            particle_states: vec![ParticleState::default(); MAX],
            texture,
            rot_range: 0.0,
            base_uv_cel: 64,
            uv_cel_count: 1,
            min_particles: 10.min(MAX),
            size_min: Vec2::new(16.0, 16.0),
            size_adjust: Vec2::new(8.0, 8.0),
            dsize_min: Vec2::new(0.0, 0.0),
            dsize_adjust: Vec2::new(0.0, 0.0),
            life_min: 30,
            life_adjust: 240,
            w_min: PI / 16.0,
            w_adjust: PI / 16.0,
            dw_min: 0.0,
            dw_adjust: 0.0,
            velocity_sampler: VectorSampler::new(
                (-PI / 2.0 - PI / 8.0, -PI / 2.0 + PI / 8.0),
                (-PI / 8.0, PI / 8.0),
                (50.0, 50.0),
            ),
            acceleration_sampler: VectorSampler::new(
                (PI / 2.0, PI / 2.0),
                (0.0, 0.0),
                (40.0, 40.0),
            ),
            color_interp: vec![
                Color(255, 0, 0, 196),
                Color(255, 0, 0, 128),
                Color(255, 0, 0, 0),
            ],
        }
    }
    fn initialize_particles(&mut self, rng: &mut impl Rng, howmany: usize) {
        let mut particles = self.particles.borrow_mut();
        for _pi in 0..howmany {
            let mut p = RenderParticle::default();
            let mut ps = ParticleState::default();
            p.uv_region = index_to_uv(self.base_uv_cel);
            p.position = self.transform * self.emitter.sample(rng);
            p.rot = rng.gen::<f32>() * self.rot_range - self.rot_range / 2.0;
            p.size = self.size_min + self.size_adjust * rng.gen::<f32>();
            p.rgba = self.color_interp[0].to_rgba8888_array();
            ps.dsize = self.dsize_min + self.dsize_adjust * rng.gen::<f32>();
            ps.t = 0;
            ps.tmax = self.life_min + (self.life_adjust as f32 * rng.gen::<f32>()) as u16;
            ps.w = self.w_min + self.w_adjust * rng.gen::<f32>();
            ps.dw = self.dw_min + self.dw_adjust * rng.gen::<f32>();
            ps.velocity = self.transform.rotation * self.velocity_sampler.sample(rng);
            ps.acceleration = self.transform.rotation * self.acceleration_sampler.sample(rng);
            particles.push(p);
            self.particle_states.push(ps);
        }
    }
    fn update(&mut self) {
        let mut rng = rand::thread_rng();
        self.next_emit -= DT as f32;
        let pc = { self.particles.borrow().len() } as i32;
        let howmany = pc + (self.min_particles as i32 - pc);
        // how many units of 1.0/self.emit_rate is next_emit below 0 by?
        // 1.5 --> -1.5 = -1
        // 1 --> -1/1 = -1 --> 0
        // 0.5 --> -.5/1.0 = -1/2 --> 0
        // 0 --> 1
        // -0.5 --> .5/1.0 = 1/2 --> 1
        // -1/emit_rate --> 2
        // -1.5 --> 2
        // -2 --> 3
        let howmany_more = (-self.next_emit / (1.0 / self.emit_rate)).floor() + 1.0;
        let howmany = (howmany + howmany_more as i32).min(self.max_particles as i32 - pc);
        if howmany > 0 {
            self.initialize_particles(&mut rng, howmany as usize);
            if howmany_more > 0.0 {
                self.next_emit += howmany_more / self.emit_rate; // 1.0 / emits per second = seconds per emit
            }
        }
        let mut particles = self.particles.borrow_mut();
        let mut dead = vec![];
        for (idx, (p, ps)) in particles
            .iter_mut()
            .zip(self.particle_states.iter_mut())
            .enumerate()
        {
            ps.t += 1;
            let tr = ps.t as f32 / ps.tmax as f32;
            if tr >= 1.0 {
                dead.push(idx);
                continue;
            }
            p.size += ps.dsize * DT as f32;
            ps.velocity += ps.acceleration * DT as f32;
            p.position += ps.velocity * DT as f32;
            ps.w += ps.dw * DT as f32;
            p.rot += ps.w * DT as f32;
            p.rgba = Self::color_interpolate(&self.color_interp, tr);
            let which_uv = self.base_uv_cel + (tr * self.uv_cel_count as f32) as usize;
            p.uv_region = index_to_uv(which_uv as usize);
        }
        for dead in dead.into_iter().rev() {
            particles.swap_remove(dead);
            self.particle_states.swap_remove(dead);
        }
        dbg!(particles.len());
    }
    fn render(&self, rs: &mut frenderer::renderer::RenderState) {
        rs.render_billboard_batch((self.texture, Blend::Additive), self.particles.clone());
    }
    fn color_interpolate(cols: &[Color], r: f32) -> [u8; 4] {
        let len = cols.len();
        let r_idx = len as f32 * r;
        let which_0 = (r_idx as usize).min(len - 1);
        let which_1 = (which_0 + 1).min(len - 1);
        let which_r = if which_0 == which_1 {
            1.0
        } else {
            (r_idx - which_0 as f32) / ((which_1 - which_0) as f32)
        };
        cols[which_0]
            .interpolate(&cols[which_1], which_r)
            .to_rgba8888_array()
    }
}
struct Floor {
    trf: Similarity3,
    model: Rc<textured::Model>,
}

struct World {
    camera: Camera,
    smoke: ParticleSystem,
    hearts: ParticleSystem,
    floor: Floor,
}
impl frenderer::World for World {
    fn update(&mut self, input: &frenderer::Input, _assets: &mut frenderer::assets::Assets) {
        let particle_drot_xz = input.key_axis(Key::Z, Key::X) * PI / 2.0 * DT as f32;
        let particle_drot_xy = input.key_axis(Key::C, Key::V) * PI / 2.0 * DT as f32;
        let particle_dx = input.key_axis(Key::A, Key::D) * 10.0 * DT as f32;
        let particle_dy = input.key_axis(Key::Q, Key::E) * 10.0 * DT as f32;
        let particle_dz = input.key_axis(Key::W, Key::S) * 10.0 * DT as f32;
        self.smoke
            .transform
            .prepend_translation(Vec3::new(particle_dx, particle_dy, particle_dz));
        self.hearts
            .transform
            .prepend_rotation(Rotor3::from_rotation_xz(particle_drot_xz));
        self.hearts
            .transform
            .prepend_rotation(Rotor3::from_rotation_xy(particle_drot_xy));

        self.hearts.update();
        self.smoke.update();
        let camera_drot = input.key_axis(Key::Left, Key::Right) * PI / 4.0 * DT as f32;
        self.camera
            .transform
            .prepend_rotation(Rotor3::from_rotation_xz(camera_drot));
        // std::process::exit(0);
    }
    fn render(
        &mut self,
        _a: &mut frenderer::assets::Assets,
        rs: &mut frenderer::renderer::RenderState,
    ) {
        rs.set_camera(self.camera);
        self.hearts.render(rs);
        self.smoke.render(rs);
        rs.render_textured_interpolated(
            self.floor.model.clone(),
            0,
            textured::SingleRenderState::new(self.floor.trf),
        );
    }
}
fn index_to_uv(idx: usize) -> Rect {
    const DIM: usize = 2048;
    const DIM_F: f32 = DIM as f32;
    const IMG_DIM: usize = 128;
    const IMG_DIM_F: f32 = IMG_DIM as f32 / DIM_F;
    const IMGS_ACROSS: usize = DIM / IMG_DIM;
    let x = idx % IMGS_ACROSS;
    let y = idx / IMGS_ACROSS;
    let ux = (x * IMG_DIM) as f32 / DIM_F;
    let uy = (y * IMG_DIM) as f32 / DIM_F;
    Rect::new(ux, 1.0 - uy - IMG_DIM_F, IMG_DIM_F, IMG_DIM_F)
}

fn main() -> Result<()> {
    frenderer::color_eyre::install()?;
    let mut engine: Engine = Engine::new(FrendererSettings::default(), DT);
    let particle_tex = engine
        .assets()
        .load_texture(&std::path::Path::new("content/particles.png"))?;
    // we know this image is 2048x2048, and the textures are all 128x128
    //0..4: circle
    //5..7: dirt
    //8..9: fire
    //10..15: flame
    //16: flare
    //17..19: light
    //20..24: magic
    //25..29: muzzle
    //30..32: scorch
    //33: scratch
    //34..37: slash
    //38..47: smoke
    //48..54: spark
    //55..63: star
    //64..65: symbol
    //66..72: trace
    //73..75: twirl
    //76..79: window
    let floor_tex = engine
        .assets()
        .load_texture(std::path::Path::new("content/cube-diffuse.jpg"))?;
    let floor_meshes = engine
        .assets()
        .load_textured(std::path::Path::new("content/floor.obj"))?;
    let floor = engine
        .assets()
        .create_textured_model(floor_meshes, vec![floor_tex]);

    let camera = Camera::look_at(
        Vec3::new(0., 0., 100.),
        Vec3::new(0., 0., 0.),
        Vec3::new(0., 1., 0.),
        Projection::Perspective { fov: PI / 2.0 },
    );
    let world = World {
        camera,
        floor: Floor {
            model: floor,
            trf: Similarity3::new(Vec3::new(0.0, -25.0, 0.0), Rotor3::identity(), 10.0),
        },
        smoke: ParticleSystem::new_smoke(
            particle_tex,
            Isometry3::new(
                Vec3::new(20.0, 0.0, 0.0),
                Rotor3::from_rotation_xy(PI / 2.0),
            ),
        ),
        hearts: ParticleSystem::new_hearts(
            particle_tex,
            Isometry3::new(Vec3::new(-20.0, 0.0, 0.0), Rotor3::identity()),
        ),
    };
    engine.play(world)
}
