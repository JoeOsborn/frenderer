use crate::types::*;
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub transform: Similarity3,
    pub ratio: f32,
    pub projection: Projection,
}
#[derive(Clone, Copy, Debug)]
pub enum Projection {
    Perspective { fov: f32 },
    Orthographic { width: f32, depth: f32 },
}
impl Camera {
    pub fn look_at(eye: Vec3, at: Vec3, up: Vec3, proj: Projection) -> Camera {
        let iso = Mat4::look_at(eye, at, up).into_isometry();
        Self::from_transform(Similarity3::new(iso.translation, iso.rotation, 1.0), proj)
    }
    pub fn from_transform(s: Similarity3, proj: Projection) -> Self {
        Self {
            transform: s,
            ratio: 4.0 / 3.0,
            projection: proj,
        }
    }
    pub fn set_ratio(&mut self, r: f32) {
        self.ratio = r;
    }
    pub fn as_matrix(&self) -> Mat4 {
        self.projection.as_matrix(self.ratio) * self.transform.into_homogeneous_matrix()
    }
}
impl Interpolate for Camera {
    fn interpolate(&self, other: Self, r: f32) -> Self {
        Self {
            transform: self.transform.interpolate(other.transform, r),
            ratio: self.ratio.interpolate(other.ratio, r),
            projection: self.projection.interpolate(other.projection, r),
        }
    }
    fn interpolate_limit(&self, other: Self, r: f32, lim: f32) -> Self {
        Self {
            transform: self.transform.interpolate_limit(other.transform, r, lim),
            ratio: self.ratio.interpolate_limit(other.ratio, r, lim),
            projection: self.projection.interpolate_limit(other.projection, r, lim),
        }
    }
}
impl Projection {
    pub fn as_matrix(&self, r: f32) -> Mat4 {
        match self {
            Projection::Perspective { fov } => {
                ultraviolet::projection::rh_yup::perspective_reversed_infinite_z_vk(*fov, r, 0.1)
            }
            Projection::Orthographic { width, depth } => orthographic_reversed_z_vk(
                -width / 2.0,
                width / 2.0,
                -(width / 2.0) / r,
                (width / 2.0) / r,
                0.1,
                *depth,
            ),
        }
    }
}

impl Interpolate for Projection {
    fn interpolate(&self, other: Self, r: f32) -> Self {
        match (self, other) {
            (Self::Perspective { fov }, Self::Perspective { fov: ofov }) => Self::Perspective {
                fov: fov.interpolate(ofov, r),
            },
            (
                Self::Orthographic { width, depth },
                Self::Orthographic {
                    width: owidth,
                    depth: odepth,
                },
            ) => Self::Orthographic {
                width: width.interpolate(owidth, r),
                depth: depth.interpolate(odepth, r),
            },
            (_, _) => other,
        }
    }
    fn interpolate_limit(&self, other: Self, r: f32, lim: f32) -> Self {
        match (self, other) {
            (Self::Perspective { fov }, Self::Perspective { fov: ofov }) => Self::Perspective {
                fov: fov.interpolate_limit(ofov, r, lim),
            },
            (
                Self::Orthographic { width, depth },
                Self::Orthographic {
                    width: owidth,
                    depth: odepth,
                },
            ) => Self::Orthographic {
                width: width.interpolate_limit(owidth, r, lim),
                depth: depth.interpolate_limit(odepth, r, lim),
            },
            (_, _) => other,
        }
    }
}

fn orthographic_reversed_z_vk(
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
    mut near: f32,
    mut far: f32,
) -> Mat4 {
    let rml = right - left;
    let rpl = right + left;
    let tmb = top - bottom;
    let tpb = top + bottom;
    std::mem::swap(&mut far, &mut near);
    let fmn = far - near;
    Mat4::new(
        Vec4::new(2.0 / rml, 0.0, 0.0, 0.0),
        Vec4::new(0.0, -2.0 / tmb, 0.0, 0.0),
        Vec4::new(0.0, 0.0, -1.0 / fmn, 0.0),
        Vec4::new(-(rpl / rml), -(tpb / tmb), -(near / fmn), 1.0),
    )
}
