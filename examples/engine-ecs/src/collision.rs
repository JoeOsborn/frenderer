use crate::geom::*;
use crate::hecs::{Entity, World};
use crate::Transform;
pub struct Pushable(usize);
impl Default for Pushable {
    fn default() -> Self {
        Self(usize::MAX)
    }
}
pub struct Solid(usize);
impl Default for Solid {
    fn default() -> Self {
        Self(usize::MAX)
    }
}
pub struct SolidPushable(usize);
impl Default for SolidPushable {
    fn default() -> Self {
        Self(usize::MAX)
    }
}
pub struct Trigger(usize);
impl Default for Trigger {
    fn default() -> Self {
        Self(usize::MAX)
    }
}
pub struct BoxCollision(pub AABB);

#[derive(Clone, Copy)]
pub struct Contact(pub Entity, pub Entity, pub Vec2);

pub(crate) struct Contacts {
    e_triggers: Vec<(Entity, AABB)>,
    e_solid_pushable: Vec<(Entity, AABB)>,
    e_solid: Vec<(Entity, AABB)>,
    e_pushable: Vec<(Entity, AABB)>,
    e_triggers_free: Vec<usize>,
    e_solid_pushable_free: Vec<usize>,
    e_solid_free: Vec<usize>,
    e_pushable_free: Vec<usize>,

    pub(crate) triggers: Vec<Contact>,
    pub(crate) displacements: Vec<Contact>,
    contacts: Vec<(Entity, Entity, (f32, f32), Vec2)>,
}
impl Contacts {
    pub(crate) fn new() -> Self {
        Self {
            triggers: Vec::with_capacity(32),
            displacements: Vec::with_capacity(32),
            contacts: Vec::with_capacity(32),
            e_triggers: vec![],
            e_solid_pushable: vec![],
            e_solid: vec![],
            e_pushable: vec![],
            e_triggers_free: vec![],
            e_solid_pushable_free: vec![],
            e_solid_free: vec![],
            e_pushable_free: vec![],
        }
    }
    pub(crate) fn insert_entity(&mut self, entity: Entity, world: &mut World) {
        if let Ok((trf, cbox, trigger)) =
            world.query_one_mut::<(&Transform, &BoxCollision, &mut Trigger)>(entity)
        {
            let val = (entity, cbox.0 + trf.translation());
            if let Some(free) = self.e_triggers_free.pop() {
                self.e_triggers[free] = val;
                trigger.0 = free;
            } else {
                self.e_triggers.push(val);
                trigger.0 = self.e_triggers.len() - 1;
            }
        } else if let Ok((trf, cbox, solid)) =
            world.query_one_mut::<(&Transform, &BoxCollision, &mut Solid)>(entity)
        {
            let val = (entity, cbox.0 + trf.translation());
            if let Some(free) = self.e_solid_free.pop() {
                self.e_solid[free] = val;
                solid.0 = free;
            } else {
                self.e_solid.push(val);
                solid.0 = self.e_solid.len() - 1;
            }
        } else if let Ok((trf, cbox, pushable)) =
            world.query_one_mut::<(&Transform, &BoxCollision, &mut Pushable)>(entity)
        {
            let val = (entity, cbox.0 + trf.translation());
            if let Some(free) = self.e_pushable_free.pop() {
                self.e_pushable[free] = val;
                pushable.0 = free;
            } else {
                self.e_pushable.push(val);
                pushable.0 = self.e_pushable.len() - 1;
            }
        } else if let Ok((trf, cbox, solid_pushable)) =
            world.query_one_mut::<(&Transform, &BoxCollision, &mut SolidPushable)>(entity)
        {
            let val = (entity, cbox.0 + trf.translation());
            if let Some(free) = self.e_solid_pushable_free.pop() {
                self.e_solid_pushable[free] = val;
                solid_pushable.0 = free;
            } else {
                self.e_solid_pushable.push(val);
                solid_pushable.0 = self.e_solid_pushable.len() - 1;
            }
        }
    }
    pub(crate) fn remove_entity(&mut self, entity: Entity, world: &mut World) {
        if let Ok((trigger,)) = world.query_one_mut::<(&Trigger,)>(entity) {
            let pos = trigger.0;
            self.e_triggers_free.push(pos);
            self.e_triggers[pos] = (hecs::Entity::DANGLING, AABB::zeroed());
        } else if let Ok((solid,)) = world.query_one_mut::<(&Solid,)>(entity) {
            let pos = solid.0;
            self.e_solid_free.push(pos);
            self.e_solid[pos] = (hecs::Entity::DANGLING, AABB::zeroed());
        } else if let Ok((pushable,)) = world.query_one_mut::<(&Pushable,)>(entity) {
            let pos = pushable.0;
            self.e_pushable_free.push(pos);
            self.e_pushable[pos] = (hecs::Entity::DANGLING, AABB::zeroed());
        } else if let Ok((solid_pushable,)) = world.query_one_mut::<(&SolidPushable,)>(entity) {
            let pos = solid_pushable.0;
            self.e_solid_pushable_free.push(pos);
            self.e_solid_pushable[pos] = (hecs::Entity::DANGLING, AABB::zeroed());
        }
    }
    pub(crate) fn frame_update_index(&mut self, world: &mut World) {
        self.contacts.clear();
        // update all aabbs for entities in each vec, in case the game has done funky stuff to move them around
        for (entity, (trf, cbox, trigger)) in
            world.query_mut::<(&Transform, &BoxCollision, &Trigger)>()
        {
            update_internal(entity, trf, cbox, &mut self.e_triggers[trigger.0]);
        }
        for (entity, (trf, cbox, solid)) in world.query_mut::<(&Transform, &BoxCollision, &Solid)>()
        {
            update_internal(entity, trf, cbox, &mut self.e_solid[solid.0]);
        }
        for (entity, (trf, cbox, pushable)) in
            world.query_mut::<(&Transform, &BoxCollision, &Pushable)>()
        {
            update_internal(entity, trf, cbox, &mut self.e_pushable[pushable.0]);
        }
        for (entity, (trf, cbox, solid_pushable)) in
            world.query_mut::<(&Transform, &BoxCollision, &SolidPushable)>()
        {
            update_internal(
                entity,
                trf,
                cbox,
                &mut self.e_solid_pushable[solid_pushable.0],
            );
        }
    }
    pub(crate) fn step_update_index(&mut self, _world: &mut World) {
        // No need to update due to displacements here since we handle it during displace()
    }
    pub(crate) fn optimize_index(&mut self, _world: &mut World) {
        // Could compact the lists if we wanted to but eh, that sounds tricky, we'd have to update the components of the involved entities
    }
    fn sort(&mut self) {
        self.contacts.sort_by(|c1, c2| {
            c2.3.length_squared()
                .partial_cmp(&c1.3.length_squared())
                .unwrap()
        })
    }
    pub(crate) fn do_collisions(&mut self, world: &mut World) {
        // no need to check solid-solid
        // gather_contacts_within(&self.e_solid, &mut self.contacts);
        // solid->pushable has a restitution
        gather_contacts_across(
            &self.e_solid,
            &self.e_pushable,
            (0.0, 1.0),
            &mut self.contacts,
        );
        // solid->solid_pushable has a restitution
        gather_contacts_across(
            &self.e_solid,
            &self.e_solid_pushable,
            (0.0, 1.0),
            &mut self.contacts,
        );
        // pushable->pushable has no restitution
        //gather_contacts_within(&self.e_pushable, &mut self.contacts);
        // solid_pushable->pushable has a restitution
        gather_contacts_across(
            &self.e_solid_pushable,
            &self.e_pushable,
            (0.5, 0.5),
            &mut self.contacts,
        );
        // solid_pushable->solid_pushable has a restitution
        gather_contacts_within(&self.e_solid_pushable, (0.5, 0.5), &mut self.contacts);
        self.sort();
        let displacements = &mut self.displacements;
        let first_disp = displacements.len();
        for (ci, cj, weights, _contact_disp) in self.contacts.drain(..) {
            assert_ne!(ci, cj);
            let aabb_i = {
                let (trf_i, BoxCollision(aabb_i)) = world
                    .query_one_mut::<(&Transform, &BoxCollision)>(ci)
                    .unwrap();
                *aabb_i + trf_i.translation()
            };
            let aabb_j = {
                let (trf_j, BoxCollision(aabb_j)) = world
                    .query_one_mut::<(&Transform, &BoxCollision)>(cj)
                    .unwrap();
                *aabb_j + trf_j.translation()
            };
            let disp = aabb_j.displacement(aabb_i).unwrap_or(Vec2::ZERO);
            if disp.x.abs() < std::f32::EPSILON || disp.y.abs() < std::f32::EPSILON {
                continue;
            }
            let (disp_i, disp_j) = compute_disp(aabb_i, aabb_j, weights, disp);
            {
                let (trf_i,) = world.query_one_mut::<(&mut Transform,)>(ci).unwrap();
                displace(ci, cj, trf_i, disp_i, displacements);
            }
            {
                let (trf_j,) = world.query_one_mut::<(&mut Transform,)>(cj).unwrap();
                displace(cj, ci, trf_j, disp_j, displacements);
            }
        }
        let limit = displacements.len();
        assert!(first_disp <= displacements.len());
        assert!(limit <= displacements.len());
        for i in first_disp..limit {
            let Contact(ci, cj, _disp) = self.displacements[i];
            self.update_entity(ci, world);
            self.update_entity(cj, world);
        }
    }
    pub(crate) fn update_entity(&mut self, entity: Entity, world: &mut World) {
        // big if else to update entity's aabb in correct vec
        if let Ok((trf, cbox, trigger)) =
            world.query_one_mut::<(&Transform, &BoxCollision, &mut Trigger)>(entity)
        {
            update_internal(entity, trf, cbox, &mut self.e_triggers[trigger.0]);
        } else if let Ok((trf, cbox, solid)) =
            world.query_one_mut::<(&Transform, &BoxCollision, &mut Solid)>(entity)
        {
            update_internal(entity, trf, cbox, &mut self.e_solid[solid.0]);
        } else if let Ok((trf, cbox, pushable)) =
            world.query_one_mut::<(&Transform, &BoxCollision, &mut Pushable)>(entity)
        {
            update_internal(entity, trf, cbox, &mut self.e_pushable[pushable.0]);
        } else if let Ok((trf, cbox, solid_pushable)) =
            world.query_one_mut::<(&Transform, &BoxCollision, &mut SolidPushable)>(entity)
        {
            update_internal(
                entity,
                trf,
                cbox,
                &mut self.e_solid_pushable[solid_pushable.0],
            );
        }
    }
    pub(crate) fn gather_triggers(&mut self) {
        gather_triggers_within(&self.e_triggers, &mut self.triggers);
        gather_triggers_across(&self.e_triggers, &self.e_solid, &mut self.triggers);
        gather_triggers_across(&self.e_triggers, &self.e_pushable, &mut self.triggers);
        gather_triggers_across(&self.e_triggers, &self.e_solid_pushable, &mut self.triggers);
        // pushables are implicitly triggers too, but can't exist within a solid or solid pushable any longer.
        // same for solid_pushable, which can't exist within any other solid pushable.
        gather_triggers_within(&self.e_pushable, &mut self.triggers);
    }
}

fn compute_disp(ci: AABB, cj: AABB, weights: (f32, f32), mut disp: Vec2) -> (Vec2, Vec2) {
    // Guy is left of wall, push left
    if ci.center.x < cj.center.x {
        disp.x *= -1.0;
    }
    // Guy is below wall, push down
    if ci.center.y < cj.center.y {
        disp.y *= -1.0;
    }
    (disp * weights.0, -disp * weights.1)
}

fn displace(
    char_id: Entity,
    char_other: Entity,
    trf: &mut Transform,
    amt: Vec2,
    displacements: &mut Vec<Contact>,
) {
    if amt.x.abs() < amt.y.abs() {
        trf.x += amt.x;
        displacements.push(Contact(char_id, char_other, Vec2 { x: amt.x, y: 0.0 }));
    } else
    /* amt.y.abs() <= amt.x.abs() */
    {
        trf.y += amt.y;
        displacements.push(Contact(char_id, char_other, Vec2 { y: amt.y, x: 0.0 }));
    }
}

fn gather_within(grp: &[(Entity, AABB)], mut f: impl FnMut(Entity, Entity, Vec2)) {
    for (ci, (ent_i, aabb_i)) in grp.iter().enumerate() {
        if *ent_i == Entity::DANGLING {
            continue;
        }
        for (ent_j, aabb_j) in grp[(ci + 1)..].iter() {
            if *ent_j == Entity::DANGLING {
                continue;
            }
            if let Some(disp) = aabb_i.displacement(*aabb_j) {
                f(*ent_i, *ent_j, disp);
            }
        }
    }
}

fn gather_across(
    grp_a: &[(Entity, AABB)],
    grp_b: &[(Entity, AABB)],
    mut f: impl FnMut(Entity, Entity, Vec2),
) {
    for (ent_i, aabb_i) in grp_a.iter() {
        if *ent_i == Entity::DANGLING {
            continue;
        }
        for (ent_j, aabb_j) in grp_b.iter() {
            if *ent_j == Entity::DANGLING {
                continue;
            }
            if let Some(disp) = aabb_i.displacement(*aabb_j) {
                f(*ent_i, *ent_j, disp);
            }
        }
    }
}

fn gather_contacts_within(
    grp: &[(Entity, AABB)],
    weights: (f32, f32),
    contacts: &mut Vec<(Entity, Entity, (f32, f32), Vec2)>,
) {
    gather_within(grp, |ent_i, ent_j, disp| {
        contacts.push((ent_i, ent_j, weights, disp))
    });
}
fn gather_contacts_across(
    grp_a: &[(Entity, AABB)],
    grp_b: &[(Entity, AABB)],
    weights: (f32, f32),
    contacts: &mut Vec<(Entity, Entity, (f32, f32), Vec2)>,
) {
    gather_across(grp_a, grp_b, |ent_i, ent_j, disp| {
        contacts.push((ent_i, ent_j, weights, disp))
    });
}
fn gather_triggers_within(grp: &[(Entity, AABB)], triggers: &mut Vec<Contact>) {
    gather_within(grp, |ent_i, ent_j, disp| {
        triggers.push(Contact(ent_i, ent_j, disp))
    });
}
fn gather_triggers_across(
    grp_a: &[(Entity, AABB)],
    grp_b: &[(Entity, AABB)],
    triggers: &mut Vec<Contact>,
) {
    gather_across(grp_a, grp_b, |ent_i, ent_j, disp| {
        triggers.push(Contact(ent_i, ent_j, disp))
    });
}
fn update_internal(
    entity: Entity,
    trf: &Transform,
    cbox: &BoxCollision,
    old_val: &mut (Entity, AABB),
) {
    let val = (entity, cbox.0 + trf.translation());
    assert_eq!(entity, old_val.0);
    *old_val = val;
}
