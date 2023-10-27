pub use bytemuck::Zeroable;
pub use frenderer::{
    input::{Input, Key},
    wgpu, Frenderer, GPUCamera as Camera, Region, Transform,
};
pub struct BitFont<B: std::ops::RangeBounds<char> = std::ops::RangeInclusive<char>> {
    spritesheet: Spritesheet,
    font: frenderer::BitFont<B>,
}
pub trait Game: Sized + 'static {
    type Tag: TagType;
    fn new(engine: &mut Engine<Self>) -> Self;
    fn update(&mut self, engine: &mut Engine<Self>);
    fn handle_collisions(&mut self, engine: &mut Engine<Self>, contacts: &Contacts<Self::Tag>);
    fn render(&mut self, engine: &mut Engine<Self>);
}

const COLLISION_STEPS: usize = 3;

pub trait TagType: Copy + Eq + Ord {}

pub struct Chara<Tag: TagType> {
    aabb_: geom::AABB,
    vel_: geom::Vec2,
    uv_: geom::Rect,
    tag_: Option<Tag>,
    collision_: CollisionFlags,
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
    charas: Vec<Vec<Chara<G::Tag>>>,
    texts: Vec<(usize, Vec<TextDraw>)>,
}

pub struct Contact(pub CharaID, pub CharaID, pub geom::Vec2);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CharaID(
    u32, /* spritesheet */
    u32, /* index within charas[spritesheet] */
);

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
            charas: vec![],
            texts: vec![],
        }
    }
    pub fn run(mut self) {
        let mut game = G::new(&mut self);
        const DT: f32 = 1.0 / 60.0;
        const DT_FUDGE_AMOUNT: f32 = 0.0002;
        const DT_MAX: f32 = DT * 5.0;
        const TIME_SNAPS: [f32; 5] = [15.0, 30.0, 60.0, 120.0, 144.0];
        let mut contacts = Contacts {
            grouped_contacts: std::collections::BTreeMap::new(),
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
                            for charas in self.charas.iter_mut() {
                                for chara in charas.iter_mut() {
                                    chara.aabb_.center += chara.vel_;
                                }
                            }
                            for _iter in 0..COLLISION_STEPS {
                                Self::do_collisions(&mut self.charas, &mut contacts);
                                game.handle_collisions(&mut self, &contacts);
                                contacts.clear();
                            }
                            self.input.next_frame();
                        }
                        game.render(&mut self);
                        for (idx, charas) in self.charas.iter().enumerate() {
                            let chara_len = charas.len();
                            let (text_len, texts) = &self.texts[idx];
                            self.renderer.sprites.resize_sprite_group(
                                &self.renderer.gpu,
                                idx,
                                chara_len + text_len,
                            );
                            let (trfs, uvs) = self.renderer.sprites.get_sprites_mut(idx);
                            // iterate through charas and update trf,uv
                            // TODO: this could be more efficient by only updating charas which changed
                            for (chara, (trf, uv)) in
                                charas.iter().zip(trfs.iter_mut().zip(uvs.iter_mut()))
                            {
                                *trf = chara.aabb_.into();
                                *uv = chara.uv_.into();
                            }
                            // iterate through texts and draw each one
                            let mut sprite_idx = chara_len;
                            for TextDraw(font, text, pos, sz) in texts.iter() {
                                let (count, _) = font.draw_text(
                                    &mut self.renderer.sprites,
                                    idx,
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
                                idx,
                                0..(chara_len + text_len),
                            );
                        }
                        // update sprites from charas
                        // update texts
                        self.renderer
                            .sprites
                            .set_camera_all(&self.renderer.gpu, self.camera);
                        self.renderer.render();
                        self.texts.iter_mut().for_each(|(count, txts)| {
                            *count = 0;
                            txts.clear();
                        });
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
    fn do_collisions(charas: &mut [Vec<Chara<G::Tag>>], contacts: &mut Contacts<G::Tag>) {
        for (gi, group_i) in charas.iter().enumerate() {
            for (ci, chara_i) in group_i.iter().enumerate() {
                if chara_i.tag_.is_none() || chara_i.collision_.is_empty() {
                    continue;
                }
                let id_i = CharaID(gi as u32, ci as u32);
                for (cj, chara_j) in group_i.iter().enumerate().skip(ci + 1) {
                    if chara_j.tag_.is_none() || chara_j.collision_.is_empty() {
                        continue;
                    }
                    let id_j = CharaID(gi as u32, cj as u32);
                    if let Some(disp) = chara_i.aabb_.displacement(chara_j.aabb_) {
                        contacts.push(
                            chara_i.tag_.unwrap(),
                            chara_j.tag_.unwrap(),
                            id_i,
                            id_j,
                            disp,
                        );
                    }
                }
                for (gj, group_j) in charas.iter().enumerate().skip(gi + 1) {
                    for (cj, chara_j) in group_j.iter().enumerate() {
                        if chara_j.tag_.is_none() || chara_j.collision_.is_empty() {
                            continue;
                        }
                        let id_j = CharaID(gj as u32, cj as u32);
                        if let Some(disp) = chara_i.aabb_.displacement(chara_j.aabb_) {
                            contacts.push(
                                chara_i.tag_.unwrap(),
                                chara_j.tag_.unwrap(),
                                id_i,
                                id_j,
                                disp,
                            );
                        }
                    }
                }
            }
        }
        // now do restitution for movable vs movable, movable vs solid, solid vs movable
        contacts.sort();
        for Contact(ci, cj, _disp) in contacts.contacts.iter() {
            let char_i = &charas[ci.0 as usize][ci.1 as usize];
            let char_j = &charas[cj.0 as usize][cj.1 as usize];
            // if either is a trigger, continue (triggers don't get restituted even by solid things)
            if char_i.collision_.contains(CollisionFlags::TRIGGER)
                || char_j.collision_.contains(CollisionFlags::TRIGGER)
            {
                continue;
            }
            // if neither is solid, continue (no actual occlusion)
            if !char_i.collision_.contains(CollisionFlags::SOLID)
                && !char_j.collision_.contains(CollisionFlags::SOLID)
            {
                continue;
            }
            // if both are immovable, continue (nothing to do)
            if !char_i.collision_.contains(CollisionFlags::MOVABLE)
                && !char_j.collision_.contains(CollisionFlags::MOVABLE)
            {
                continue;
            }
            let disp = char_j
                .aabb_
                .displacement(char_i.aabb_)
                .unwrap_or(geom::Vec2::ZERO);
            if disp.x.abs() < std::f32::EPSILON || disp.y.abs() < std::f32::EPSILON {
                continue;
            }
            let (disp_i, disp_j) = Self::compute_disp(char_i, char_j, disp);
            Self::displace(&mut charas[ci.0 as usize][ci.1 as usize], disp_i);
            Self::displace(&mut charas[cj.0 as usize][cj.1 as usize], disp_j);
        }
    }
    fn displace(chara: &mut Chara<G::Tag>, amt: Vec2) {
        if amt.x.abs() < amt.y.abs() {
            chara.aabb_.center.x += amt.x;
        } else if amt.y.abs() <= amt.x.abs() {
            chara.aabb_.center.y += amt.y;
        }
    }
    fn compute_disp(
        ci: &Chara<G::Tag>,
        cj: &Chara<G::Tag>,
        mut disp: geom::Vec2,
    ) -> (geom::Vec2, geom::Vec2) {
        // Preconditions: neither is a trigger or empty
        assert!(!ci.collision_.contains(CollisionFlags::TRIGGER));
        assert!(!ci.collision_.is_empty());
        assert!(!cj.collision_.contains(CollisionFlags::TRIGGER));
        assert!(!cj.collision_.is_empty());
        // Preconditions: at least one is movable
        assert!(
            ci.collision_.contains(CollisionFlags::MOVABLE)
                || cj.collision_.contains(CollisionFlags::MOVABLE)
        );
        // Preconditions: at least one is solid
        assert!(
            ci.collision_.contains(CollisionFlags::SOLID)
                || cj.collision_.contains(CollisionFlags::SOLID)
        );
        // Guy is left of wall, push left
        if ci.aabb_.center.x < cj.aabb_.center.x {
            disp.x *= -1.0;
        }
        // Guy is below wall, push down
        if ci.aabb_.center.y < cj.aabb_.center.y {
            disp.y *= -1.0;
        }
        // both are movable and solid, split disp
        if ci.collision_ == CollisionFlags::MOVABLE | CollisionFlags::SOLID
            && cj.collision_ == CollisionFlags::MOVABLE | CollisionFlags::SOLID
        {
            (disp / 2.0, -disp / 2.0)
        } else if !ci.collision_.contains(CollisionFlags::MOVABLE)
            && cj.collision_.contains(CollisionFlags::MOVABLE)
        {
            // cj is movable and ci is not movable, so can't move ci whether or not ci/cj is solid
            (geom::Vec2::ZERO, -disp)
        } else {
            // ci is movable and cj is not movable, so can't move cj whether or not ci/cj is solid
            (disp, geom::Vec2::ZERO)
        }
    }
    pub fn make_chara(
        &mut self,
        spritesheet: Spritesheet,
        tag: G::Tag,
        aabb: geom::AABB,
        uv: geom::Rect,
        col: CollisionFlags,
    ) -> CharaID {
        col.check();
        self.ensure_spritegroup_size(spritesheet.0, self.charas[spritesheet.0].len() + 1);
        self.charas[spritesheet.0].push(Chara {
            aabb_: aabb,
            uv_: uv,
            vel_: geom::Vec2::ZERO,
            tag_: Some(tag),
            collision_: col,
        });
        CharaID(
            spritesheet.0 as u32,
            self.charas[spritesheet.0].len() as u32 - 1,
        )
    }
    pub fn recycle_chara(
        &mut self,
        spritesheet: Spritesheet,
        tag: G::Tag,
        aabb: geom::AABB,
        uv: geom::Rect,
        col: CollisionFlags,
    ) -> CharaID {
        col.check();
        if let Some(idx) = self.charas[spritesheet.0]
            .iter()
            .position(|c| c.tag_.is_none())
        {
            self.charas[spritesheet.0][idx] = Chara {
                aabb_: aabb,
                uv_: uv,
                vel_: geom::Vec2::ZERO,
                tag_: Some(tag),
                collision_: col,
            };
            CharaID(spritesheet.0 as u32, idx as u32)
        } else {
            self.make_chara(spritesheet, tag, aabb, uv, col)
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
            spritesheet,
        }
    }
    pub fn charas_by_tag_mut(
        &mut self,
        spritesheet: Spritesheet,
        tag: G::Tag,
    ) -> impl Iterator<Item = (CharaID, &mut Chara<G::Tag>)> {
        self.charas[spritesheet.0]
            .iter_mut()
            .enumerate()
            .filter(move |(_ci, c)| c.tag_ == Some(tag))
            .map(move |(ci, c)| (CharaID(spritesheet.0 as u32, ci as u32), c))
    }
    pub fn charas_by_tag(
        &mut self,
        spritesheet: Spritesheet,
        tag: G::Tag,
    ) -> impl Iterator<Item = (CharaID, &Chara<G::Tag>)> {
        self.charas[spritesheet.0]
            .iter()
            .enumerate()
            .filter(move |(_ci, c)| c.tag_ == Some(tag))
            .map(move |(ci, c)| (CharaID(spritesheet.0 as u32, ci as u32), c))
    }
    pub fn chara_mut(&mut self, id: CharaID) -> &mut Chara<G::Tag> {
        &mut self.charas[id.0 as usize][id.1 as usize]
    }
    pub fn chara(&self, id: CharaID) -> &Chara<G::Tag> {
        &self.charas[id.0 as usize][id.1 as usize]
    }
    pub fn kill_chara(&mut self, id: CharaID) {
        self.charas[id.0 as usize][id.1 as usize].tag_ = None;
        self.charas[id.0 as usize][id.1 as usize].aabb_ = geom::AABB::zeroed();
        self.charas[id.0 as usize][id.1 as usize].uv_ = geom::Rect::zeroed();
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
        self.charas.push(Vec::with_capacity(1024));
        self.texts.push((0, vec![]));
        Spritesheet(self.renderer.sprites.add_sprite_group(
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
        ))
    }
    pub fn draw_string(&mut self, font: &BitFont, text: String, pos: geom::Vec2, char_sz: f32) {
        self.texts[font.spritesheet.0].0 += text.len();
        self.texts[font.spritesheet.0]
            .1
            .push(TextDraw(font.font.clone(), text, pos, char_sz));
    }
}

pub mod geom;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Spritesheet(usize);

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
        // empty: Does not participate in contact generation
        const TRIGGER = 0b001; // Contacts generated, no collision response
        const MOVABLE = 0b010; // Can be moved by restitution if touching a solid object
        const SOLID   = 0b100; // Can force restitution of movable objects
    }
    // e.g. a moving platform could be MOVABLE | SOLID. if enemies can
    // pass through each other they should be MOVABLE. a static wall
    // should just be SOLID. the "end of level" marker should be
    // TRIGGER.
}
impl CollisionFlags {
    fn check(&self) {
        assert!(!(self.contains(Self::TRIGGER) && self.contains(Self::SOLID)));
    }
}
pub struct Contacts<Tag: TagType> {
    // contacts is grouped by tag pairs
    grouped_contacts: std::collections::BTreeMap<(Tag, Tag), Vec<Contact>>,
    contacts: Vec<Contact>,
}
impl<Tag: TagType> Contacts<Tag> {
    pub fn query(&self, a: Tag, b: Tag) -> Option<impl Iterator<Item = &Contact>> {
        self.grouped_contacts
            .get(&(if a < b { (a, b) } else { (b, a) }))
            .map(|cs| cs.iter())
    }
    fn clear(&mut self) {
        for val in self.grouped_contacts.values_mut() {
            val.clear();
        }
        self.contacts.clear();
    }
    fn sort(&mut self) {
        self.contacts.sort_by(|c1, c2| {
            c2.2.length_squared()
                .partial_cmp(&c1.2.length_squared())
                .unwrap()
        })
    }
    fn push(&mut self, tag_i: Tag, tag_j: Tag, ci: CharaID, cj: CharaID, disp: geom::Vec2) {
        self.contacts.push(Contact(ci, cj, disp));
        self.grouped_contacts
            .entry(if tag_i < tag_j {
                (tag_i, tag_j)
            } else {
                (tag_j, tag_i)
            })
            .or_insert(Vec::with_capacity(32))
            .push(Contact(ci, cj, disp));
    }
}
