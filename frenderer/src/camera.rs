use crate::types::*;
pub struct Camera {
    eye: Vec3,
    at: Vec3,
    up: Vec3,
    fov: f32,
    ratio: f32,
    z_far: f32,
}
impl Camera {
    pub fn look_at(eye: Vec3, at: Vec3, up: Vec3) -> Camera {
        Camera {
            eye,
            at,
            up,
            fov: PI / 2.0,
            ratio: 4.0 / 3.0,
            z_far: 1000.0,
        }
    }
    pub fn set_ratio(&mut self, r:f32) { self.ratio = r; }
    pub fn as_matrix(&self) -> Mat4 {
        // projection * view
        let proj =
            ultraviolet::projection::rh_yup::perspective_vk(self.fov, self.ratio, 0.1, self.z_far);
        proj * Mat4::look_at(self.eye, self.at, self.up)
    }
}
