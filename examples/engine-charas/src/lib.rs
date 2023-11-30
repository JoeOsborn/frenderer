use std::sync::Arc;

pub use bytemuck::Zeroable;
pub use frenderer::{wgpu, Camera2D as Camera, Frenderer, SheetRegion, Transform};
pub use helperer::{
    input::{Input, Key},
    Clock,
};
mod gfx;
pub use gfx::{BitFont, Spritesheet};

mod game;
pub use game::{Game, TagType};
mod collision;
pub use collision::{Collision, Contact};

const COLLISION_STEPS: usize = 3;
mod chara;
pub use chara::Chara;
use gfx::TextDraw;

pub mod geom;

pub struct Engine<G: Game> {
    pub renderer: Frenderer,
    pub input: Input,
    camera: Camera,
    event_loop: Option<winit::event_loop::EventLoop<()>>,
    window: Arc<winit::window::Window>,
    // We could pull these three out into a "World" or "CollisionWorld",
    // but note that the collision system only needs AABBs and tags, not vel or uv.
    charas_nocollide: Vec<Chara<G::Tag>>,
    charas_trigger: Vec<Chara<G::Tag>>,
    charas_physical: Vec<(Chara<G::Tag>, collision::CollisionFlags)>,
    // Text drawing
    texts: Vec<TextDraw>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CharaID(
    u8, /* group */
    /* consider: generation, and matching generation on chara */
    u32, /* index within group */
);

impl<G: Game> Engine<G> {
    const C_NC: u8 = 0;
    const C_TR: u8 = 1;
    const C_PH: u8 = 2;

    pub fn new(builder: winit::window::WindowBuilder) -> Result<Self, Box<dyn std::error::Error>> {
        let event_loop = winit::event_loop::EventLoop::new()?;
        let window = Arc::new(builder.build(&event_loop)?);
        let renderer = frenderer::with_default_runtime(window.clone());
        let input = Input::default();
        let camera = Camera {
            screen_pos: [0.0, 0.0],
            screen_size: window.inner_size().into(),
        };
        Ok(Self {
            renderer,
            input,
            window,
            event_loop: Some(event_loop),
            camera,
            charas_nocollide: vec![],
            charas_trigger: vec![],
            charas_physical: vec![],
            texts: Vec::with_capacity(128),
        })
    }
    pub fn run(mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut clock = Clock::new(1.0 / 60.0, 0.0002, 5);
        let mut game = G::new(&mut self);
        let mut contacts = collision::Contacts::new();

        Ok(self.event_loop.take().unwrap().run(
            move |event, target| match helperer::handle_event(
                &mut clock,
                &self.window,
                &event,
                target,
                &mut self.input,
                &mut self.renderer,
            ) {
                helperer::EventPhase::Simulate(steps) => {
                    for _ in 0..steps {
                        game.update(&mut self);
                        for (_id, chara) in self.charas_mut() {
                            // This will update dead charas too but it won't cause any harm
                            chara.aabb_.center += chara.vel_;
                        }
                        for _iter in 0..COLLISION_STEPS {
                            collision::do_collisions(&mut self.charas_physical, &mut contacts);
                            game.handle_collisions(&mut self, contacts.displacements.drain(..));
                            contacts.clear();
                        }
                        collision::gather_triggers(
                            &mut self.charas_trigger,
                            &mut self.charas_physical,
                            &mut contacts,
                        );
                        game.handle_triggers(&mut self, contacts.triggers.drain(..));
                        contacts.clear();
                        self.input.next_frame();
                    }
                }
                helperer::EventPhase::Draw => {
                    game.render(&mut self);
                    let chara_len = self.charas().count();
                    let text_len: usize = self.texts.iter().map(|t| t.1.len()).sum();
                    self.renderer.sprites.resize_sprite_group(
                        &self.renderer.gpu,
                        0,
                        chara_len + text_len,
                    );
                    let (trfs, uvs) = self.renderer.sprites.get_sprites_mut(0);
                    // iterate through charas and update trf,uv
                    // TODO: this could be more efficient by only updating charas which changed, or could be done during integration?
                    for ((_id, chara), (trf, uv)) in Self::charas_internal(
                        &self.charas_nocollide,
                        &self.charas_trigger,
                        &self.charas_physical,
                    )
                    .zip(trfs.iter_mut().zip(uvs.iter_mut()))
                    {
                        *trf = chara.aabb_.into();
                        *uv = chara.uv_;
                    }
                    // iterate through texts and draw each one
                    let mut sprite_idx = chara_len;
                    for TextDraw(font, text, pos, sz) in self.texts.iter() {
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
                    self.texts.clear();
                    self.window.request_redraw();
                }
                helperer::EventPhase::Quit => {
                    target.exit();
                }
                helperer::EventPhase::Wait => {}
            },
        )?)
    }
    pub fn make_chara(
        &mut self,
        spritesheet: Spritesheet,
        tag: G::Tag,
        aabb: geom::AABB,
        uv: frenderer::SheetRegion,
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
                (Self::C_NC, self.charas_nocollide.len())
            }
            Collision::Trigger => {
                self.charas_trigger.push(chara);
                (Self::C_TR, self.charas_trigger.len())
            }
            Collision::Colliding(flags) => {
                self.charas_physical.push((chara, flags));
                (Self::C_PH, self.charas_physical.len())
            }
        };
        CharaID(grp, len as u32 - 1)
    }
    pub fn recycle_chara(
        &mut self,
        spritesheet: Spritesheet,
        tag: G::Tag,
        aabb: geom::AABB,
        uv: frenderer::SheetRegion,
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
                    CharaID(Self::C_NC, idx as u32)
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
                    CharaID(Self::C_TR, idx as u32)
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
                    CharaID(Self::C_PH, idx as u32)
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
        uv: SheetRegion,
        chars_per_row: u16,
    ) -> BitFont<B> {
        BitFont {
            font: helperer::BitFont::with_sheet_region(range, uv, chars_per_row),
            _spritesheet: spritesheet,
        }
    }
    pub fn charas_by_tag_mut(
        &mut self,
        tag: G::Tag,
    ) -> impl Iterator<Item = (CharaID, &mut Chara<G::Tag>)> {
        self.charas_mut()
            .filter(move |(_id, c)| c.tag_ == Some(tag))
    }
    pub fn charas_by_tag(
        &mut self,
        tag: G::Tag,
    ) -> impl Iterator<Item = (CharaID, &Chara<G::Tag>)> {
        self.charas().filter(move |(_id, c)| c.tag_ == Some(tag))
    }
    pub fn chara_mut(&mut self, id: CharaID) -> Option<&mut Chara<G::Tag>> {
        match id.0 {
            Self::C_NC if self.charas_nocollide[id.1 as usize].tag_.is_some() => {
                Some(&mut self.charas_nocollide[id.1 as usize])
            }
            Self::C_TR if self.charas_trigger[id.1 as usize].tag_.is_some() => {
                Some(&mut self.charas_trigger[id.1 as usize])
            }
            Self::C_PH if self.charas_physical[id.1 as usize].0.tag_.is_some() => {
                Some(&mut self.charas_physical[id.1 as usize].0)
            }
            _ => None,
        }
    }
    pub fn chara(&self, id: CharaID) -> Option<&Chara<G::Tag>> {
        match id.0 {
            Self::C_NC if self.charas_nocollide[id.1 as usize].tag_.is_some() => {
                Some(&self.charas_nocollide[id.1 as usize])
            }
            Self::C_TR if self.charas_trigger[id.1 as usize].tag_.is_some() => {
                Some(&self.charas_trigger[id.1 as usize])
            }
            Self::C_PH if self.charas_physical[id.1 as usize].0.tag_.is_some() => {
                Some(&self.charas_physical[id.1 as usize].0)
            }
            _ => None,
        }
    }
    fn charas_internal<'s>(
        nc: &'s [Chara<G::Tag>],
        tr: &'s [Chara<G::Tag>],
        ph: &'s [(Chara<G::Tag>, collision::CollisionFlags)],
    ) -> impl Iterator<Item = (CharaID, &'s Chara<G::Tag>)> {
        nc.iter()
            .enumerate()
            .map(move |(ci, c)| (CharaID(Self::C_NC, ci as u32), c))
            .chain(
                tr.iter()
                    .enumerate()
                    .map(move |(ci, c)| (CharaID(Self::C_TR, ci as u32), c)),
            )
            .chain(
                ph.iter()
                    .enumerate()
                    .map(move |(ci, (c, _flags))| (CharaID(Self::C_PH, ci as u32), c)),
            )
    }
    fn charas(&self) -> impl Iterator<Item = (CharaID, &Chara<G::Tag>)> {
        Self::charas_internal(
            &self.charas_nocollide,
            &self.charas_trigger,
            &self.charas_physical,
        )
    }
    fn charas_mut(&mut self) -> impl Iterator<Item = (CharaID, &mut Chara<G::Tag>)> {
        self.charas_nocollide
            .iter_mut()
            .enumerate()
            .map(move |(ci, c)| (CharaID(Self::C_NC, ci as u32), c))
            .chain(
                self.charas_trigger
                    .iter_mut()
                    .enumerate()
                    .map(move |(ci, c)| (CharaID(Self::C_TR, ci as u32), c)),
            )
            .chain(
                self.charas_physical
                    .iter_mut()
                    .enumerate()
                    .map(move |(ci, (c, _flags))| (CharaID(Self::C_PH, ci as u32), c)),
            )
    }
    pub fn kill_chara(&mut self, id: CharaID) {
        let ch = match id.0 {
            Self::C_NC => &mut self.charas_nocollide[id.1 as usize],
            Self::C_TR => &mut self.charas_trigger[id.1 as usize],
            Self::C_PH => &mut self.charas_physical[id.1 as usize].0,
            _ => panic!("invalid chara grouping"),
        };
        ch.tag_ = None;
        ch.aabb_ = geom::AABB::zeroed();
        ch.uv_ = SheetRegion::zeroed();
    }
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
    pub fn add_spritesheet(
        &mut self,
        imgs: &[&image::RgbaImage],
        label: Option<&str>,
    ) -> Spritesheet {
        let img_bytes: Vec<_> = imgs.iter().map(|img| img.as_raw().as_slice()).collect();
        let idx = self.renderer.sprites.add_sprite_group(
            &self.renderer.gpu,
            &self.renderer.gpu.create_array_texture(
                &img_bytes,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                imgs[0].dimensions(),
                label,
            ),
            vec![Transform::zeroed(); 1024],
            vec![SheetRegion::zeroed(); 1024],
            self.camera,
        );
        assert!(idx == 0, "We only support one spritesheet for now");
        Spritesheet(idx)
    }
    pub fn draw_string(&mut self, font: &BitFont, text: String, pos: geom::Vec2, char_sz: f32) {
        self.texts
            .push(TextDraw(font.font.clone(), text, pos, char_sz));
    }
}

impl<G: Game> std::ops::Index<CharaID> for Engine<G> {
    type Output = Chara<G::Tag>;

    fn index(&self, index: CharaID) -> &Self::Output {
        self.chara(index).unwrap()
    }
}

impl<G: Game> std::ops::IndexMut<CharaID> for Engine<G> {
    fn index_mut(&mut self, index: CharaID) -> &mut Self::Output {
        self.chara_mut(index).unwrap()
    }
}
