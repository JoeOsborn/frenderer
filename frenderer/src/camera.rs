use crate::types::*;
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub transform: Similarity3,
    pub fov: f32,
    pub ratio: f32,
}
impl Camera {
    pub fn look_at(eye: Vec3, at: Vec3, up: Vec3) -> Camera {
        let iso = Mat4::look_at(eye, at, up).into_isometry();
        Self::from_transform(Similarity3::new(iso.translation, iso.rotation, 1.0))
    }
    pub fn from_transform(s: Similarity3) -> Self {
        Self {
            transform: s,
            fov: PI / 2.0,
            ratio: 4.0 / 3.0,
        }
    }
    pub fn set_ratio(&mut self, r: f32) {
        self.ratio = r;
    }
    pub fn as_matrix(&self) -> Mat4 {
        // projection * view
        let proj = ultraviolet::projection::rh_yup::perspective_reversed_infinite_z_vk(
            self.fov, self.ratio, 0.1,
        );
        proj * self.transform.into_homogeneous_matrix()
    }
    pub fn interpolate(&self, other: &Self, r: f32) -> Self {
        Self {
            transform: self.transform.lerp(&other.transform, r),
            fov: self.fov.lerp(other.fov, r),
            ratio: self.ratio.lerp(other.ratio, r),
        }
    }
}
