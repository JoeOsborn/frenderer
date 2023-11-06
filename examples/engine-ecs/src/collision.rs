use crate::geom::*;
use crate::hecs::{Entity, World};
use crate::Transform;
pub struct Pushable();
pub struct Solid();
pub struct SolidPushable();
pub struct Trigger();
pub struct BoxCollision(pub AABB);

pub struct Contact(pub Entity, pub Entity, pub Vec2);

const COARSE_DIM: usize = 128;
const FINE_DIM: usize = 32;

trait Cell {
    type Element: Copy;
    fn inserted(
        &mut self,
        first: u32,
        elt: Self::Element,
        idx: u32,
        storage: &CellRowStorage<Self::Element>,
    );
    fn removed(
        &mut self,
        first: Option<NonZeroU32>,
        elt: Self::Element,
        storage: &CellRowStorage<Self::Element>,
    );
}

struct EltStorage<Elt: Copy> {
    elt: Elt,
    next: Option<NonZeroU32>,
}

struct CellRowStorage<Elt: Copy> {
    values: Vec<EltStorage<Elt>>,
    first_free: Option<NonZeroU32>,
}

struct CellColumn<C: Cell> {
    cell: C,
    first_trigger: Option<NonZeroU32>,
    first_solid: Option<NonZeroU32>,
    first_pushable: Option<NonZeroU32>,
    first_solid_pushable: Option<NonZeroU32>,
}

struct CellRow<C: Cell> {
    storage: CellRowStorage<C::Element>,
    cells: Vec<CellColumn<C>>,
}

struct Grid<C: Cell> {
    rows: Vec<Option<CellRow<C>>>,
    dim: usize,
    width: usize,
    height: usize,
}

impl<C: Cell> Grid<C> {
    fn new(dim: usize, width: usize, height: usize) -> Self {
        Self {
            rows: vec![None; width * height],
            dim,
            width,
            height,
        }
    }
}

struct CoarseCell {}
impl Cell for CoarseCell {
    type Element = u32;
    fn inserted(
        &mut self,
        _first: u32,
        _elt: Self::Element,
        _idx: u32,
        _storage: &CellRowStorage<Self::Element>,
    ) {
    }
    fn removed(
        &mut self,
        _first: Option<NonZeroU32>,
        _elt: Self::Element,
        _storage: &CellRowStorage<Self::Element>,
    ) {
    }
}
struct FineCell {
    aabb: AABB,
}
impl Cell for FineCell {
    type Element = (Entity, AABB);
    fn inserted(
        &mut self,
        _first: u32,
        elt: Self::Element,
        _idx: u32,
        _storage: &CellRowStorage<Self::Element>,
    ) {
        // expand aabb
    }
    fn removed(
        &mut self,
        _first: Option<NonZeroU32>,
        _elt: Self::Element,
        _storage: &CellRowStorage<Self::Element>,
    ) {
        // recompute aabb
    }
}

pub(crate) struct Contacts {
    coarse_grid: Grid<CoarseCell>,
    fine_grid: Grid<FineCell>,

    pub(crate) triggers: Vec<Contact>,
    pub(crate) displacements: Vec<Contact>,
    contacts: Vec<(Entity, Entity, (f32, f32), Vec2)>,
}
impl Contacts {
    pub(crate) fn new() -> Self {
        const W: usize = 65536;
        const H: usize = 65536;
        Self {
            triggers: Vec::with_capacity(32),
            displacements: Vec::with_capacity(32),
            contacts: Vec::with_capacity(32),
            coarse_grid: Grid::new(COARSE_DIM, W / COARSE_DIM, H / COARSE_DIM),
            fine_grid: Grid::new(FINE_DIM, W / FINE_DIM, H / FINE_DIM),
        }
    }
    pub(crate) fn remake_index(&mut self, world: &mut World) {
        self.contacts.clear();
        self.triggers.clear();
        self.displacements.clear();

        for (e, (trf, cbox, _trigger)) in world.query_mut::<(&Transform, &BoxCollision, &Trigger)>()
        {
            let aabb = cbox.0 + trf.translation();
            // add aabb to fine cell at center
            // get all fine cells covering aabb
            // add all fine cells to coarse grid
        }
        for (e, (trf, cbox, _solid)) in world.query_mut::<(&Transform, &BoxCollision, &Solid)>() {
            self.e_solid.push((e, cbox.0 + trf.translation()))
        }
        for (e, (trf, cbox, _pushable)) in
            world.query_mut::<(&Transform, &BoxCollision, &Pushable)>()
        {
            self.e_pushable.push((e, cbox.0 + trf.translation()))
        }
        for (e, (trf, cbox, _solid_pushable)) in
            world.query_mut::<(&Transform, &BoxCollision, &SolidPushable)>()
        {
            self.e_solid_pushable.push((e, cbox.0 + trf.translation()))
        }
    }
    pub(crate) fn update_index(&mut self, _world: &mut World) {}
    pub(crate) fn shrink_index(&mut self, _world: &mut World) {}
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
        for (ci, cj, weights, _contact_disp) in self.contacts.drain(..) {
            let mut query_i = world
                .query_one::<(&mut Transform, &BoxCollision)>(ci)
                .unwrap();
            let mut query_j = world
                .query_one::<(&mut Transform, &BoxCollision)>(cj)
                .unwrap();
            // These unwraps are all safe: the objects are definitely still around, and ci and cj are different.
            let (trf_i, BoxCollision(aabb_i)) = query_i.get().unwrap();
            let (trf_j, BoxCollision(aabb_j)) = query_j.get().unwrap();
            let aabb_i = *aabb_i + trf_i.translation();
            let aabb_j = *aabb_j + trf_j.translation();
            let disp = aabb_j.displacement(aabb_i).unwrap_or(Vec2::ZERO);
            if disp.x.abs() < std::f32::EPSILON || disp.y.abs() < std::f32::EPSILON {
                continue;
            }
            let (disp_i, disp_j) = compute_disp(aabb_i, aabb_j, weights, disp);

            displace(ci, cj, trf_i, disp_i, displacements);
            displace(cj, ci, trf_j, disp_j, displacements);
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
    } else if amt.y.abs() <= amt.x.abs() {
        trf.y += amt.y;
        displacements.push(Contact(char_id, char_other, Vec2 { y: amt.y, x: 0.0 }));
    }
}

fn gather_within(grp: &[(Entity, AABB)], mut f: impl FnMut(Entity, Entity, Vec2)) {
    for (ci, (ent_i, aabb_i)) in grp.iter().enumerate() {
        for (ent_j, aabb_j) in grp[(ci + 1)..].iter() {
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
        for (ent_j, aabb_j) in grp_b.iter() {
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
