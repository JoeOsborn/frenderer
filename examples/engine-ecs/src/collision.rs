use std::{num::NonZeroU32, usize};

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
    type Element: PartialEq;
    fn inserted(
        &mut self,
        first: u32,
        elt: Self::Element,
        idx: u32,
        storage: &CellRowStorage<Self::Element>,
    );
}

struct EltStorage<Elt> {
    elt: Elt,
    next: Option<NonZeroU32>,
}

struct EltIterator<'elts, Elt: PartialEq> {
    elts: &'elts [EltStorage<Elt>],
    cur: Option<NonZeroU32>,
}
impl<'elts, Elt: 'elts + PartialEq> Iterator for EltIterator<'elts, Elt> {
    type Item = &'elts Elt;

    fn next(&mut self) -> Option<Self::Item> {
        self.cur.map(|num| {
            let elt_node = &self.elts[num.get() as usize - 1];
            self.cur = elt_node.next;
            &elt_node.elt
        })
    }
}

struct CellRowStorage<Elt: PartialEq> {
    values: Vec<EltStorage<Elt>>,
    first_free: Option<NonZeroU32>,
}

impl<Elt: PartialEq> CellRowStorage<Elt> {
    fn elements<'s>(&'s self, first: Option<NonZeroU32>) -> EltIterator<'s, Elt> {
        EltIterator {
            elts: self.values.as_slice(),
            cur: first,
        }
    }
    fn new() -> Self {
        Self {
            values: Vec::with_capacity(128),
            first_free: None,
        }
    }
}

#[derive(Default)]
struct CellColumn<C: Cell + Default> {
    cell: C,
    first: Option<NonZeroU32>,
}

impl<Elt: PartialEq> CellRowStorage<Elt> {
    fn push(&mut self, elt: Elt, col_first: &mut Option<NonZeroU32>) -> NonZeroU32 {
        // iterate through, avoid duplicating
    }
}

struct CellRow<C: Cell + Default> {
    storage: CellRowStorage<C::Element>,
    cells: Vec<CellColumn<C>>,
}
impl<C: Cell + Default> CellRow<C> {
    fn new(width: usize) -> Self {
        let mut cells = Vec::with_capacity(width);
        cells.resize_with(width, CellColumn::default);
        Self {
            storage: CellRowStorage::new(),
            cells,
        }
    }
    fn push(&mut self, pos: u32, elt: C::Element) {
        let mut col = self.cells[pos as usize];
        let elt_idx = self.storage.push(elt, &mut col.first);
        col.cell
            .inserted(col.first.unwrap().get() - 1, elt, elt_idx, &self.storage);
    }
}

struct Grid<C: Cell + Default> {
    rows: Vec<Option<CellRow<C>>>,
    dim: usize,
    width: usize,
    height: usize,
}

impl<C: Cell + Default> Grid<C> {
    fn new(dim: usize, width: usize, height: usize) -> Self {
        Self {
            rows: (0..height).map(|_| None).collect(),
            dim,
            width,
            height,
        }
    }
    fn push(&mut self, pos: IVec2, elt: C::Element) {
        let pos: IVec2 = pos / self.dim as i32;
        assert!(pos.x >= 0);
        assert!(pos.y >= 0);
        if self.rows[pos.y as usize].is_none() {
            self.rows[pos.y as usize] = Some(CellRow::new(self.width));
        }
        let row = self.rows[pos.y as usize].as_mut().unwrap();
        row.push(pos.x as u32, elt);
    }
}
#[derive(Default)]
struct CoarseCell {}
impl Cell for CoarseCell {
    type Element = (u32, u32);
    fn inserted(
        &mut self,
        _first: u32,
        _elt: Self::Element,
        _idx: u32,
        _storage: &CellRowStorage<Self::Element>,
    ) {
    }
}
#[derive(Default)]
struct FineCell {
    aabb: AABB,
}
// The u8 is collision flags:
const COL_TRIG: u8 = 0b0000_0001;
const COL_SOL: u8 = 0b0000_0010;
const COL_PUSH: u8 = 0b0000_0100;
impl Cell for FineCell {
    type Element = (Entity, AABB, u8);
    fn inserted(
        &mut self,
        _first: u32,
        (_ent, aabb, _flags): Self::Element,
        _idx: u32,
        _storage: &CellRowStorage<Self::Element>,
    ) {
        // expand aabb
        self.aabb = self.aabb.union(aabb);
    }
}

impl FineCell {
    fn recompute_aabb(
        &mut self,
        first: Option<NonZeroU32>,
        storage: &CellRowStorage<<Self as Cell>::Element>,
    ) {
        // recompute aabb
        // let mut aabb = AABB {
        // center: self.aabb.center,
        // size: Vec2 { x: 1.0, y: 1.0 },
        // };
        let mut aabb = AABB::zeroed();
        for (_, aabb_i, _) in storage.elements(first) {
            aabb = aabb.union(*aabb_i);
        }
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

        // clear all the coarse cells (set their first ptr to None, clear their storage vec, set their first free to None)
        // clear all the fine cells (set their first ptr to None, clear their storage vec, set their first free to None)

        for (e, (trf, cbox, _trigger)) in world.query_mut::<(&Transform, &BoxCollision, &Trigger)>()
        {
            let aabb = cbox.0 + trf.translation();
            // add aabb to fine cell at center
            self.fine_grid
                .push(aabb.center.as_ivec2(), (e, aabb, COL_TRIG));
        }
        // for (e, (trf, cbox, _solid)) in world.query_mut::<(&Transform, &BoxCollision, &Solid)>() {
        //     self.e_solid.push((e, cbox.0 + trf.translation()))
        // }
        // for (e, (trf, cbox, _pushable)) in
        //     world.query_mut::<(&Transform, &BoxCollision, &Pushable)>()
        // {
        //     self.e_pushable.push((e, cbox.0 + trf.translation()))
        // }
        // for (e, (trf, cbox, _solid_pushable)) in
        //     world.query_mut::<(&Transform, &BoxCollision, &SolidPushable)>()
        // {
        //     self.e_solid_pushable.push((e, cbox.0 + trf.translation()))
        // }
        // add each fine cell to the coarse cells it occupies
    }
    pub(crate) fn update_index(&mut self, _world: &mut World) {
        // add each fine cell to the coarse cells it now occupies
    }
    pub(crate) fn shrink_index(&mut self, _world: &mut World) {
        // reorder each fine cell row's storage so that each cell's elements are stored contiguously

        // clear all the coarse cells (set their first ptr to None, clear their storage vec)

        // for every fine cell, recompute its aabb and insert into the coarse cells it occupies
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
        // gather_contacts_across(
        //     &self.e_solid,
        //     &self.e_pushable,
        //     (0.0, 1.0),
        //     &mut self.contacts,
        // );
        // // solid->solid_pushable has a restitution
        // gather_contacts_across(
        //     &self.e_solid,
        //     &self.e_solid_pushable,
        //     (0.0, 1.0),
        //     &mut self.contacts,
        // );
        // // pushable->pushable has no restitution
        // //gather_contacts_within(&self.e_pushable, &mut self.contacts);
        // // solid_pushable->pushable has a restitution
        // gather_contacts_across(
        //     &self.e_solid_pushable,
        //     &self.e_pushable,
        //     (0.5, 0.5),
        //     &mut self.contacts,
        // );
        // // solid_pushable->solid_pushable has a restitution
        // gather_contacts_within(&self.e_solid_pushable, (0.5, 0.5), &mut self.contacts);
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
        // gather_triggers_within(&self.e_triggers, &mut self.triggers);
        // gather_triggers_across(&self.e_triggers, &self.e_solid, &mut self.triggers);
        // gather_triggers_across(&self.e_triggers, &self.e_pushable, &mut self.triggers);
        // gather_triggers_across(&self.e_triggers, &self.e_solid_pushable, &mut self.triggers);
        // pushables are implicitly triggers too, but can't exist within a solid or solid pushable any longer.
        // same for solid_pushable, which can't exist within any other solid pushable.
        // gather_triggers_within(&self.e_pushable, &mut self.triggers);
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
