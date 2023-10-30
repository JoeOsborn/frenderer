pub use bytemuck::Zeroable;
pub use frenderer::{
    input::{Input, Key},
    wgpu, Frenderer, GPUCamera as Camera, Region, Transform,
};
pub struct BitFont<B: std::ops::RangeBounds<char> = std::ops::RangeInclusive<char>> {
    _spritesheet: Spritesheet,
    font: frenderer::BitFont<B>,
}
pub trait Game: Sized + 'static {
    type Tag: TagType;
    fn new(engine: &mut Engine<Self>) -> Self;
    fn update(&mut self, engine: &mut Engine<Self>);
    fn handle_collisions(
        &mut self,
        engine: &mut Engine<Self>,
        contacts: impl Iterator<Item = Contact<Self::Tag>>,
    );
    fn handle_triggers(
        &mut self,
        engine: &mut Engine<Self>,
        contacts: impl Iterator<Item = Contact<Self::Tag>>,
    );
    fn render(&mut self, engine: &mut Engine<Self>);
}

const COLLISION_STEPS: usize = 3;

pub trait TagType: Copy + Eq + Ord {}

pub struct Chara<Tag: TagType> {
    aabb_: geom::AABB,
    vel_: geom::Vec2,
    uv_: geom::Rect,
    tag_: Option<Tag>,
}

impl<Tag: TagType> Chara<Tag> {
    pub fn pos(&self) -> geom::Vec2 {
        self.aabb_.center
    }
    pub fn set_pos(&mut self, p: geom::Vec2) {
        self.aabb_.center = p;
    }
    pub fn aabb(&self) -> geom::AABB {
        self.aabb_
    }
    pub fn set_aabb(&mut self, b: geom::AABB) {
        self.aabb_ = b;
    }
    pub fn vel(&self) -> geom::Vec2 {
        self.vel_
    }
    pub fn set_vel(&mut self, v: geom::Vec2) {
        self.vel_ = v;
    }
}

struct TextDraw(frenderer::BitFont, String, geom::Vec2, f32);

pub struct Engine<G: Game> {
    pub renderer: Frenderer,
    pub input: Input,
    camera: Camera,
    event_loop: Option<winit::event_loop::EventLoop<()>>,
    window: winit::window::Window,
    charas_nocollide: Vec<Chara<G::Tag>>,
    charas_trigger: Vec<Chara<G::Tag>>,
    charas_physical: Vec<(Chara<G::Tag>, u8 /* col flags */)>,
    texts: (usize, Vec<TextDraw>),
}

pub struct Contact<T: TagType>(pub CharaID, pub T, pub CharaID, pub T, pub geom::Vec2);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CharaID(u8 /* group */, u32 /* index within group */);

impl<G: Game> Engine<G> {
    pub fn new(builder: winit::window::WindowBuilder) -> Self {
        let event_loop = winit::event_loop::EventLoop::new();
        let window = builder.build(&event_loop).unwrap();
        let renderer = frenderer::with_default_runtime(&window);
        let input = Input::default();
        let camera = Camera {
            screen_pos: [0.0, 0.0],
            screen_size: window.inner_size().into(),
        };
        Self {
            renderer,
            input,
            window,
            event_loop: Some(event_loop),
            camera,
            charas_nocollide: vec![],
            charas_trigger: vec![],
            charas_physical: vec![],
            texts: (0, Vec::with_capacity(128)),
        }
    }
    pub fn run(mut self) {
        let mut game = G::new(&mut self);
        const DT: f32 = 1.0 / 60.0;
        const DT_FUDGE_AMOUNT: f32 = 0.0002;
        const DT_MAX: f32 = DT * 5.0;
        const TIME_SNAPS: [f32; 5] = [15.0, 30.0, 60.0, 120.0, 144.0];
        let mut contacts = Contacts {
            triggers: Vec::with_capacity(32),
            displacements: Vec::with_capacity(32),
            contacts: Vec::with_capacity(32),
        };
        let mut acc = 0.0;
        let mut now = std::time::Instant::now();
        self.event_loop
            .take()
            .unwrap()
            .run(move |event, _, control_flow| {
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
                            game.update(&mut self);
                            for chara in self
                                .charas_nocollide
                                .iter_mut()
                                .chain(self.charas_trigger.iter_mut())
                                .chain(self.charas_physical.iter_mut().map(|(c, _f)| c))
                            {
                                chara.aabb_.center += chara.vel_;
                            }
                            for _iter in 0..COLLISION_STEPS {
                                Self::do_collisions(&mut self.charas_physical, &mut contacts);
                                game.handle_collisions(&mut self, contacts.displacements.drain(..));
                                contacts.clear();
                            }
                            Self::gather_triggers(
                                &mut self.charas_trigger,
                                &mut self.charas_physical,
                                &mut contacts,
                            );
                            game.handle_triggers(&mut self, contacts.triggers.drain(..));
                            contacts.clear();
                            self.input.next_frame();
                        }
                        game.render(&mut self);
                        let chara_len = self.charas_nocollide.len()
                            + self.charas_trigger.len()
                            + self.charas_physical.len();
                        let (text_len, texts) = &self.texts;
                        self.renderer.sprites.resize_sprite_group(
                            &self.renderer.gpu,
                            0,
                            chara_len + text_len,
                        );
                        let (trfs, uvs) = self.renderer.sprites.get_sprites_mut(0);
                        // iterate through charas and update trf,uv
                        // TODO: this could be more efficient by only updating charas which changed
                        for (chara, (trf, uv)) in self
                            .charas_nocollide
                            .iter()
                            .chain(self.charas_trigger.iter())
                            .chain(self.charas_physical.iter().map(|(c, _f)| c))
                            .zip(trfs.iter_mut().zip(uvs.iter_mut()))
                        {
                            *trf = chara.aabb_.into();
                            *uv = chara.uv_.into();
                        }
                        // iterate through texts and draw each one
                        let mut sprite_idx = chara_len;
                        for TextDraw(font, text, pos, sz) in texts.iter() {
                            let (count, _) = font.draw_text(
                                &mut self.renderer.sprites,
                                0,
                                sprite_idx,
                                text,
                                (*pos).into(),
                                *sz,
                            );
                            sprite_idx += count;
                        }
                        assert_eq!(sprite_idx, chara_len + text_len);
                        // TODO: this could be more efficient by only uploading charas which changed
                        self.renderer.sprites.upload_sprites(
                            &self.renderer.gpu,
                            0,
                            0..(chara_len + text_len),
                        );
                        // update sprites from charas
                        // update texts
                        self.renderer
                            .sprites
                            .set_camera_all(&self.renderer.gpu, self.camera);
                        self.renderer.render();
                        self.texts.0 = 0;
                        self.texts.1.clear();
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
    fn do_collisions(charas: &mut [(Chara<G::Tag>, u8)], contacts: &mut Contacts<G::Tag>) {
        for (ci, (chara_i, _flags)) in charas.iter().enumerate() {
            let id_i = CharaID(2, ci as u32);
            if chara_i.tag_.is_none() {
                continue;
            }
            let tag_i = chara_i.tag_.unwrap();
            for (cj, (chara_j, _flags)) in charas.iter().enumerate().skip(ci + 1) {
                if chara_j.tag_.is_none() {
                    continue;
                }
                let tag_j = chara_j.tag_.unwrap();
                let id_j = CharaID(2, cj as u32);
                if let Some(disp) = chara_i.aabb_.displacement(chara_j.aabb_) {
                    contacts.push(id_i, tag_i, id_j, tag_j, disp);
                }
            }
        }
        // now do restitution for movable vs movable, movable vs solid, solid vs movable
        contacts.sort();
        let displacements = &mut contacts.displacements;
        for (ci, cj, _contact_disp) in contacts.contacts.drain(..) {
            let (char_i, flags_i) = &charas[ci.1 as usize];
            let tag_i = char_i.tag_.unwrap();
            let flags_i = Collision::Colliding(*flags_i);
            let (char_j, flags_j) = &charas[cj.1 as usize];
            let flags_j = Collision::Colliding(*flags_j);
            let tag_j = char_j.tag_.unwrap();
            // if neither is solid, continue (no actual occlusion)
            // TODO: group solid and movable and solid+movable into three groups?  or movable, solid+movable?
            if !flags_i.is_solid() && !flags_j.is_solid() {
                continue;
            }
            // if both are immovable, continue (nothing to do)
            if !flags_i.is_movable() && !flags_j.is_movable() {
                continue;
            }
            let disp = char_j
                .aabb_
                .displacement(char_i.aabb_)
                .unwrap_or(geom::Vec2::ZERO);
            if disp.x.abs() < std::f32::EPSILON || disp.y.abs() < std::f32::EPSILON {
                continue;
            }
            let (disp_i, disp_j) = Self::compute_disp(char_i, flags_i, char_j, flags_j, disp);
            Self::displace(
                ci,
                cj,
                &mut charas[ci.1 as usize].0,
                tag_j,
                disp_i,
                displacements,
            );
            Self::displace(
                cj,
                ci,
                &mut charas[cj.1 as usize].0,
                tag_i,
                disp_j,
                displacements,
            );
        }
    }
    fn displace(
        char_id: CharaID,
        char_other: CharaID,
        chara: &mut Chara<G::Tag>,
        other_tag: G::Tag,
        amt: geom::Vec2,
        displacements: &mut Vec<Contact<G::Tag>>,
    ) {
        if amt.x.abs() < amt.y.abs() {
            chara.aabb_.center.x += amt.x;
            displacements.push(Contact(
                char_id,
                chara.tag_.unwrap(),
                char_other,
                other_tag,
                geom::Vec2 { x: amt.x, y: 0.0 },
            ));
        } else if amt.y.abs() <= amt.x.abs() {
            chara.aabb_.center.y += amt.y;
            displacements.push(Contact(
                char_id,
                chara.tag_.unwrap(),
                char_other,
                other_tag,
                geom::Vec2 { y: amt.y, x: 0.0 },
            ));
        }
    }
    fn compute_disp(
        ci: &Chara<G::Tag>,
        flags_i: Collision,
        cj: &Chara<G::Tag>,
        flags_j: Collision,
        mut disp: geom::Vec2,
    ) -> (geom::Vec2, geom::Vec2) {
        // Preconditions: at least one is movable
        assert!(flags_i.is_movable() || flags_j.is_movable());
        // Preconditions: at least one is solid
        assert!(flags_i.is_solid() || flags_j.is_solid());
        // Guy is left of wall, push left
        if ci.aabb_.center.x < cj.aabb_.center.x {
            disp.x *= -1.0;
        }
        // Guy is below wall, push down
        if ci.aabb_.center.y < cj.aabb_.center.y {
            disp.y *= -1.0;
        }
        // both are movable and solid, split disp
        if flags_i.is_movable_solid() && flags_j.is_movable_solid() {
            (disp / 2.0, -disp / 2.0)
        } else if !flags_i.is_movable() && flags_j.is_movable() {
            // cj is movable and ci is not movable, so can't move ci whether or not ci/cj is solid
            (geom::Vec2::ZERO, -disp)
        } else {
            // ci is movable and cj is not movable, so can't move cj whether or not ci/cj is solid
            (disp, geom::Vec2::ZERO)
        }
    }
    fn gather_triggers(
        triggers: &mut [Chara<G::Tag>],
        solids: &mut [(Chara<G::Tag>, u8)],
        contacts: &mut Contacts<G::Tag>,
    ) {
        for (ci, chara_i) in triggers.iter().enumerate() {
            let id_i = CharaID(1, ci as u32);
            if chara_i.tag_.is_none() {
                continue;
            }
            let tag_i = chara_i.tag_.unwrap();
            for (cj, chara_j) in triggers.iter().enumerate().skip(ci + 1) {
                if chara_j.tag_.is_none() {
                    continue;
                }
                let tag_j = chara_j.tag_.unwrap();
                let id_j = CharaID(1, cj as u32);
                if let Some(disp) = chara_i.aabb_.displacement(chara_j.aabb_) {
                    contacts.push_trigger(id_i, tag_i, id_j, tag_j, disp);
                }
            }
            for (cj, (chara_j, _flags)) in solids.iter().enumerate() {
                if chara_j.tag_.is_none() {
                    continue;
                }
                let tag_j = chara_j.tag_.unwrap();
                let id_j = CharaID(2, cj as u32);
                if let Some(disp) = chara_i.aabb_.displacement(chara_j.aabb_) {
                    contacts.push_trigger(id_i, tag_i, id_j, tag_j, disp);
                }
            }
        }
    }
    pub fn make_chara(
        &mut self,
        spritesheet: Spritesheet,
        tag: G::Tag,
        aabb: geom::AABB,
        uv: geom::Rect,
        col: Collision,
    ) -> CharaID {
        col.check();
        self.ensure_spritegroup_size(
            spritesheet.0,
            self.charas_nocollide.len()
                + self.charas_trigger.len()
                + self.charas_physical.len()
                + 1,
        );
        let chara = Chara {
            aabb_: aabb,
            uv_: uv,
            vel_: geom::Vec2::ZERO,
            tag_: Some(tag),
        };
        let (grp, len) = match col {
            Collision::None => {
                self.charas_nocollide.push(chara);
                (0, self.charas_nocollide.len())
            }
            Collision::Trigger => {
                self.charas_trigger.push(chara);
                (1, self.charas_trigger.len())
            }
            Collision::Colliding(flags) => {
                self.charas_physical.push((chara, flags));
                (2, self.charas_physical.len())
            }
        };
        CharaID(grp, len as u32 - 1)
    }
    pub fn recycle_chara(
        &mut self,
        spritesheet: Spritesheet,
        tag: G::Tag,
        aabb: geom::AABB,
        uv: geom::Rect,
        col: Collision,
    ) -> CharaID {
        col.check();
        match col {
            Collision::None => {
                if let Some(idx) = self.charas_nocollide.iter().position(|c| c.tag_.is_none()) {
                    self.charas_nocollide[idx] = Chara {
                        aabb_: aabb,
                        uv_: uv,
                        vel_: geom::Vec2::ZERO,
                        tag_: Some(tag),
                    };
                    CharaID(0, idx as u32)
                } else {
                    self.make_chara(spritesheet, tag, aabb, uv, col)
                }
            }
            Collision::Trigger => {
                if let Some(idx) = self.charas_trigger.iter().position(|c| c.tag_.is_none()) {
                    self.charas_trigger[idx] = Chara {
                        aabb_: aabb,
                        uv_: uv,
                        vel_: geom::Vec2::ZERO,
                        tag_: Some(tag),
                    };
                    CharaID(1, idx as u32)
                } else {
                    self.make_chara(spritesheet, tag, aabb, uv, col)
                }
            }
            Collision::Colliding(flags) => {
                if let Some(idx) = self
                    .charas_physical
                    .iter()
                    .position(|(c, _flags)| c.tag_.is_none())
                {
                    self.charas_physical[idx] = (
                        Chara {
                            aabb_: aabb,
                            uv_: uv,
                            vel_: geom::Vec2::ZERO,
                            tag_: Some(tag),
                        },
                        flags,
                    );
                    CharaID(2, idx as u32)
                } else {
                    self.make_chara(spritesheet, tag, aabb, uv, col)
                }
            }
        }
    }
    pub fn make_font<B: std::ops::RangeBounds<char>>(
        &mut self,
        spritesheet: Spritesheet,
        range: B,
        uv: geom::Rect,
        chars_per_row: u32,
    ) -> BitFont<B> {
        BitFont {
            font: frenderer::BitFont::with_sheet_region(range, uv.into(), chars_per_row),
            _spritesheet: spritesheet,
        }
    }
    pub fn charas_by_tag_mut(
        &mut self,
        tag: G::Tag,
    ) -> impl Iterator<Item = (CharaID, &mut Chara<G::Tag>)> {
        self.charas_nocollide
            .iter_mut()
            .enumerate()
            .filter(move |(_ci, c)| c.tag_ == Some(tag))
            .map(move |(ci, c)| (CharaID(0, ci as u32), c))
            .chain(
                self.charas_trigger
                    .iter_mut()
                    .enumerate()
                    .filter(move |(_ci, c)| c.tag_ == Some(tag))
                    .map(move |(ci, c)| (CharaID(1, ci as u32), c)),
            )
            .chain(
                self.charas_physical
                    .iter_mut()
                    .enumerate()
                    .filter(move |(_ci, (c, _flags))| c.tag_ == Some(tag))
                    .map(move |(ci, (c, _flags))| (CharaID(2, ci as u32), c)),
            )
    }
    pub fn charas_by_tag(
        &mut self,
        tag: G::Tag,
    ) -> impl Iterator<Item = (CharaID, &Chara<G::Tag>)> {
        self.charas_nocollide
            .iter()
            .enumerate()
            .filter(move |(_ci, c)| c.tag_ == Some(tag))
            .map(move |(ci, c)| (CharaID(0, ci as u32), c))
            .chain(
                self.charas_trigger
                    .iter()
                    .enumerate()
                    .filter(move |(_ci, c)| c.tag_ == Some(tag))
                    .map(move |(ci, c)| (CharaID(1, ci as u32), c)),
            )
            .chain(
                self.charas_physical
                    .iter()
                    .enumerate()
                    .filter(move |(_ci, (c, _flags))| c.tag_ == Some(tag))
                    .map(move |(ci, (c, _flags))| (CharaID(2, ci as u32), c)),
            )
    }
    pub fn chara_mut(&mut self, id: CharaID) -> &mut Chara<G::Tag> {
        match id.0 {
            0 => &mut self.charas_nocollide[id.1 as usize],
            1 => &mut self.charas_trigger[id.1 as usize],
            2 => &mut self.charas_physical[id.1 as usize].0,
            _ => panic!("invalid chara grouping"),
        }
    }
    pub fn chara(&self, id: CharaID) -> &Chara<G::Tag> {
        match id.0 {
            0 => &self.charas_nocollide[id.1 as usize],
            1 => &self.charas_trigger[id.1 as usize],
            2 => &self.charas_physical[id.1 as usize].0,
            _ => panic!("invalid chara grouping"),
        }
    }
    pub fn kill_chara(&mut self, id: CharaID) {
        let ch = match id.0 {
            0 => &mut self.charas_nocollide[id.1 as usize],
            1 => &mut self.charas_trigger[id.1 as usize],
            2 => &mut self.charas_physical[id.1 as usize].0,
            _ => panic!("invalid chara grouping"),
        };
        ch.tag_ = None;
        ch.aabb_ = geom::AABB::zeroed();
        ch.uv_ = geom::Rect::zeroed();
    }
}

impl<G: Game> Engine<G> {
    fn ensure_spritegroup_size(&mut self, group: usize, count: usize) {
        if count > self.renderer.sprites.sprite_group_size(group) {
            self.renderer.sprites.resize_sprite_group(
                &self.renderer.gpu,
                group,
                count.next_power_of_two(),
            );
        }
    }
    pub fn set_camera(&mut self, camera: Camera) {
        self.camera = camera;
    }
    pub fn add_spritesheet(&mut self, img: image::RgbaImage, label: Option<&str>) -> Spritesheet {
        //self.charas.push(Vec::with_capacity(1024));
        //self.texts.push((0, vec![]));
        let idx = self.renderer.sprites.add_sprite_group(
            &self.renderer.gpu,
            self.renderer.gpu.create_texture(
                &img,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                img.dimensions(),
                label,
            ),
            vec![Transform::zeroed(); 1024],
            vec![Region::zeroed(); 1024],
            self.camera,
        );
        assert!(idx == 0, "We only support one spritesheet for now");
        Spritesheet(idx)
    }
    pub fn draw_string(&mut self, font: &BitFont, text: String, pos: geom::Vec2, char_sz: f32) {
        self.texts.0 += text.len();
        self.texts
            .1
            .push(TextDraw(font.font.clone(), text, pos, char_sz));
    }
}

pub mod geom;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Spritesheet(usize);

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Collision {
    None = 0,
    Trigger,
    Colliding(u8),
}
impl Collision {
    pub const MOVABLE: u8 = 0b01;
    pub const SOLID: u8 = 0b10;
    fn check(self) {
        match self {
            Self::Colliding(0) => panic!("Can't be colliding but neither solid nor movable"),
            Self::Colliding(n) if n > 3 => panic!("Invalid colliding mask"),
            _ => (),
        }
    }
    pub fn solid() -> Self {
        Self::Colliding(Self::SOLID)
    }
    pub fn movable() -> Self {
        Self::Colliding(Self::MOVABLE)
    }
    pub fn movable_solid() -> Self {
        Self::Colliding(Self::MOVABLE | Self::SOLID)
    }
    pub fn none() -> Self {
        Self::None
    }
    pub fn trigger() -> Self {
        Self::Trigger
    }
    pub fn is_solid(&self) -> bool {
        matches!(self, Self::Colliding(flags) if (flags & Self::SOLID) == Self::SOLID)
    }
    pub fn is_movable(&self) -> bool {
        match self {
            Self::Colliding(flags) if (flags & Self::MOVABLE) == Self::MOVABLE => true,
            _ => false,
        }
    }
    pub fn is_movable_solid(&self) -> bool {
        match self {
            Self::Colliding(flags)
                if (flags & (Self::MOVABLE | Self::SOLID)) == (Self::MOVABLE | Self::SOLID) =>
            {
                true
            }
            _ => false,
        }
    }
    pub fn is_none(&self) -> bool {
        match self {
            Self::None => true,
            _ => false,
        }
    }
    pub fn is_trigger(&self) -> bool {
        match self {
            Self::Trigger => true,
            _ => false,
        }
    }
}
pub struct Contacts<T: TagType> {
    triggers: Vec<Contact<T>>,
    displacements: Vec<Contact<T>>,
    contacts: Vec<(CharaID, CharaID, geom::Vec2)>,
}
impl<T: TagType> Contacts<T> {
    fn clear(&mut self) {
        self.contacts.clear();
        self.triggers.clear();
        self.displacements.clear();
    }
    fn sort(&mut self) {
        self.contacts.sort_by(|c1, c2| {
            c2.2.length_squared()
                .partial_cmp(&c1.2.length_squared())
                .unwrap()
        })
    }
    fn push_trigger(
        &mut self,
        char_id: CharaID,
        tag: T,
        char_other: CharaID,
        tag_other: T,
        amt: geom::Vec2,
    ) {
        if tag > tag_other {
            self.triggers
                .push(Contact(char_other, tag_other, char_id, tag, amt));
        } else {
            self.triggers
                .push(Contact(char_id, tag, char_other, tag_other, amt));
        }
    }
    fn push(&mut self, ci: CharaID, tag_i: T, cj: CharaID, tag_j: T, disp: geom::Vec2) {
        if tag_i > tag_j {
            self.contacts.push((ci, cj, disp));
        } else {
            self.contacts.push((cj, ci, disp));
        }
    }
}
