use crate::geom;
use crate::Chara;
use crate::{CharaID, TagType};

pub struct Contact<T: TagType>(pub CharaID, pub T, pub CharaID, pub T, pub geom::Vec2);

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct CollisionFlags(u8);

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Collision {
    None = 0,
    Trigger,
    Colliding(CollisionFlags),
}
impl Collision {
    const PUSHABLE: u8 = 0b01;
    const SOLID: u8 = 0b10;
    pub(crate) fn check(self) {
        match self {
            Self::Colliding(CollisionFlags(0)) => {
                panic!("Can't be colliding but neither solid nor pushable")
            }
            Self::Colliding(n) if n.0 > 3 => panic!("Invalid colliding mask"),
            _ => (),
        }
    }
    pub fn solid() -> Self {
        Self::Colliding(CollisionFlags(Self::SOLID))
    }
    pub fn pushable() -> Self {
        Self::Colliding(CollisionFlags(Self::PUSHABLE))
    }
    pub fn pushable_solid() -> Self {
        Self::Colliding(CollisionFlags(Self::PUSHABLE | Self::SOLID))
    }
    pub fn none() -> Self {
        Self::None
    }
    pub fn trigger() -> Self {
        Self::Trigger
    }
    pub fn is_solid(&self) -> bool {
        matches!(self, Self::Colliding(flags) if (flags.0 & Self::SOLID) == Self::SOLID)
    }
    pub fn is_pushable(&self) -> bool {
        matches!(self, Self::Colliding(flags) if (flags.0 & Self::PUSHABLE) == Self::PUSHABLE)
    }
    pub fn is_pushable_solid(&self) -> bool {
        matches!(self, Self::Colliding(flags)
                if (flags.0 & (Self::PUSHABLE| Self::SOLID)) == (Self::PUSHABLE| Self::SOLID))
    }
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
    pub fn is_trigger(&self) -> bool {
        matches!(self, Self::Trigger)
    }
}
pub(crate) struct Contacts<T: TagType> {
    pub(crate) triggers: Vec<Contact<T>>,
    pub(crate) displacements: Vec<Contact<T>>,
    contacts: Vec<(CharaID, CharaID, geom::Vec2)>,
}
impl<T: TagType> Contacts<T> {
    pub(crate) fn new() -> Self {
        Self {
            triggers: Vec::with_capacity(32),
            displacements: Vec::with_capacity(32),
            contacts: Vec::with_capacity(32),
        }
    }
    pub(crate) fn clear(&mut self) {
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

pub(crate) fn do_collisions<Tag: TagType>(
    charas: &mut [(Chara<Tag>, CollisionFlags)],
    contacts: &mut Contacts<Tag>,
) {
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
    // now do restitution for pushable vs pushable, pushable vs solid, solid vs pushable
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
        // TODO: group solid and pushable and solid+pushable into three groups?  or pushable, solid+pushable?
        if !flags_i.is_solid() && !flags_j.is_solid() {
            continue;
        }
        // if both are impushable, continue (nothing to do)
        if !flags_i.is_pushable() && !flags_j.is_pushable() {
            continue;
        }
        let disp = char_j
            .aabb_
            .displacement(char_i.aabb_)
            .unwrap_or(geom::Vec2::ZERO);
        if disp.x.abs() < std::f32::EPSILON || disp.y.abs() < std::f32::EPSILON {
            continue;
        }
        let (disp_i, disp_j) = compute_disp(char_i, flags_i, char_j, flags_j, disp);
        displace(
            ci,
            cj,
            &mut charas[ci.1 as usize].0,
            tag_j,
            disp_i,
            displacements,
        );
        displace(
            cj,
            ci,
            &mut charas[cj.1 as usize].0,
            tag_i,
            disp_j,
            displacements,
        );
    }
}
pub(crate) fn displace<Tag: TagType>(
    char_id: CharaID,
    char_other: CharaID,
    chara: &mut Chara<Tag>,
    other_tag: Tag,
    amt: geom::Vec2,
    displacements: &mut Vec<Contact<Tag>>,
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
fn compute_disp<Tag: TagType>(
    ci: &Chara<Tag>,
    flags_i: Collision,
    cj: &Chara<Tag>,
    flags_j: Collision,
    mut disp: geom::Vec2,
) -> (geom::Vec2, geom::Vec2) {
    // Preconditions: at least one is pushable
    assert!(flags_i.is_pushable() || flags_j.is_pushable());
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
    // both are pushable and solid, split disp
    if flags_i.is_pushable_solid() && flags_j.is_pushable_solid() {
        (disp / 2.0, -disp / 2.0)
    } else if !flags_i.is_pushable() && flags_j.is_pushable() {
        // cj is pushable and ci is not pushable, so can't move ci whether or not ci/cj is solid
        (geom::Vec2::ZERO, -disp)
    } else {
        // ci is pushable and cj is not pushable, so can't move cj whether or not ci/cj is solid
        (disp, geom::Vec2::ZERO)
    }
}
pub(crate) fn gather_triggers<Tag: TagType>(
    triggers: &mut [Chara<Tag>],
    solids: &mut [(Chara<Tag>, CollisionFlags)],
    contacts: &mut Contacts<Tag>,
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
