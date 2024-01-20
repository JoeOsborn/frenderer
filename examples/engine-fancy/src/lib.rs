use std::{any::Any, ops::Range};

pub use frenderer::{input::Input, Camera2D, Frenderer, Region, Transform};
pub struct Engine {
    pub renderer: Frenderer,
    pub input: Input,
    event_loop: winit::event_loop::EventLoop<()>,
    window: winit::window::Window,
    camera_: geom::Rect,
}

impl Engine {
    pub fn new(builder: winit::window::WindowBuilder) -> Self {
        let event_loop = winit::event_loop::EventLoop::new();
        let window = builder.build(&event_loop).unwrap();
        let renderer = frenderer::with_default_runtime(&window, None);
        let camera = geom::Rect {
            pos: geom::Vec2 { x: 0.0, y: 0.0 },
            size: geom::Vec2 {
                x: renderer.gpu.config.width as f32,
                y: renderer.gpu.config.height as f32,
            },
        };
        let input = Input::default();
        Self {
            renderer,
            input,
            window,
            event_loop,
            camera_: camera,
        }
    }
    pub fn camera(&self) -> geom::Rect {
        self.camera_
    }
    pub fn set_camera(&mut self, r: geom::Rect) {
        self.camera_ = r;
    }
    pub fn run(mut self) {
        const DT: f32 = 1.0 / 60.0;
        const DT_FUDGE_AMOUNT: f32 = 0.0002;
        const DT_MAX: f32 = DT * 5.0;
        const TIME_SNAPS: [f32; 5] = [15.0, 30.0, 60.0, 120.0, 144.0];
        let mut acc = 0.0;
        let mut now = frenderer::Instant::now();
        self.event_loop.run(move |event, _, control_flow| {
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
                    now = frenderer::Instant::now();
                    // While we have time to spend
                    while acc >= DT {
                        // simulate a frame
                        acc -= DT;
                        println!("tick");
                        //update_game();
                        self.input.next_frame();
                    }
                    // Render prep
                    //self.renderer.sprites.set_camera_all(&frend.gpu, camera);
                    // update sprite positions and sheet regions
                    // ok now render.
                    // We could just call frend.render().
                    self.renderer.render();
                    self.window.request_redraw();
                }
                event => {
                    if self.renderer.process_window_event(&event) {
                        self.window.request_redraw();
                    }
                    self.input.process_input_event(&event);
                }
            }
        });
    }
    pub fn create_entity_type<'s, Controller: EntityController>(
        &'s mut self,
        controller: Controller,
    ) -> EntityTypeBuilder<'s, Controller> {
        EntityTypeBuilder {
            engine: self,
            spritesheet_: None,
            sprite_size_: None,
            animations_: vec![],
            collision_: CollisionFlags::NONE,
            gravity_: Vec2 { x: 0.0, y: 0.0 },
            size_: None,
            controller,
        }
    }
    pub fn create_entity<T: EntityController>(
        &mut self,
        type_id: EntityTypeID<T>,
        pos: geom::Vec2,
        dat: T::InitData,
    ) -> &mut Entity {
        todo!();
    }
}
pub mod geom;

#[repr(C)]
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    bytemuck::Zeroable,
    bytemuck::Pod,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct CollisionFlags(u8);
bitflags::bitflags! {
    impl CollisionFlags: u8 {
        const NONE    = 0b000;
        const TRIGGER = 0b001;
        const MOVABLE = 0b010;
        const SOLID   = 0b100;
    }
    // e.g. a moving platform could be MOVABLE | SOLID
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct EntityTypeID<T: EntityController>(usize, PhantomData<T>);

pub struct Entity {
    pub pos: geom::Vec2,
    pub size: geom::Vec2,
    pub vel: geom::Vec2,
    pub acc: geom::Vec2,
    pub gravity: geom::Vec2,
    anim: usize,
    anim_t: f32,
    collision: CollisionFlags,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct EntityID(usize);

pub trait EntityController {
    type InitData;
    fn update(&mut self, engine: &mut Engine);
    fn entity_created(
        &mut self,
        engine: &mut Engine,
        entity: EntityID,
        custom_data: Self::InitData,
    );
    fn entity_destroyed(&mut self, engine: &mut Engine, which: EntityID);
}

#[derive(Default)]
pub struct BasicController {}
impl EntityController for BasicController {
    type InitData = ();
    fn update(&mut self, _engine: &mut Engine) {}

    fn entity_created(
        &mut self,
        _engine: &mut Engine,
        _entity: EntityID,
        _custom_data: Self::InitData,
    ) {
    }

    fn entity_destroyed(&mut self, engine: &mut Engine, which: EntityID) {}
}

pub struct EntityTypeBuilder<'e, Controller: EntityController = BasicController> {
    engine: &'e mut Engine,
    spritesheet_: Option<(frenderer::wgpu::Texture, geom::IVec2)>,
    sprite_size_: Option<geom::IVec2>,
    animations_: Vec<(Range<usize>, f32)>,
    collision_: CollisionFlags,
    gravity_: geom::Vec2,
    size_: Option<geom::Vec2>,
    controller: Controller,
}
impl<'e, Controller: EntityController> EntityTypeBuilder<'e, Controller> {
    pub fn spritesheet(mut self, img: frenderer::wgpu::Texture, cell_size: geom::IVec2) -> Self {
        self.spritesheet_ = Some((img, cell_size));
        self
    }
    pub fn sprite_size(mut self, sz: geom::IVec2) -> Self {
        self.sprite_size_ = Some(sz);
        self
    }
    pub fn animations(mut self, anims: impl Into<Vec<(Range<usize>, f32)>>) -> Self {
        self.animations_ = anims.into();
        self
    }
    pub fn size(mut self, sz: geom::Vec2) -> Self {
        self.size_ = Some(sz);
        self
    }
    pub fn collision(mut self, flags: CollisionFlags) -> Self {
        self.collision_ = flags;
        self
    }
    pub fn gravity(mut self, grav: geom::Vec2) -> Self {
        self.gravity_ = grav;
        self
    }
    pub fn build(mut self) -> EntityTypeID<Controller> {
        // register type in engine
        // with metadata necessary for downcasting to any I guess
        //
        todo!();
    }
}
