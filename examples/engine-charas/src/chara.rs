use crate::geom;
use crate::TagType;

pub struct Chara<Tag: TagType> {
    pub(crate) aabb_: geom::AABB, // Consider: Transform; but then you'd need to handle rotation in collision
    pub(crate) vel_: geom::Vec2,
    // consider: CollisionShape?  Might want to have subgroups within the three engine collision groups for that.
    pub(crate) uv_: geom::Rect, // consider: AnimationState
    // Consider: "depth" and use that in the renderer to get right ordering of charas across groups
    pub(crate) tag_: Option<Tag>,
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
