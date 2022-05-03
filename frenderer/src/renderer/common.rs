use crate::types::*;
use bytemuck::{Pod, Zeroable};

pub(crate) mod matrix_only {
    use super::*;
    #[repr(C)]
    #[derive(Clone, Default, Debug, Copy, Pod, Zeroable)]
    pub struct SingleRenderState {
        pub(crate) translation: Vec3,
        pub(crate) sz: f32,
        pub(crate) rotation: Rotor3,
    }
    impl SingleRenderState {
        pub fn new(transform: Similarity3) -> Self {
            Self {
                translation: transform.translation,
                sz: transform.scale,
                rotation: transform.rotation,
            }
        }
        pub fn transform(&self) -> Similarity3 {
            Similarity3::new(self.translation, self.rotation, self.sz)
        }
    }
    impl super::super::SingleRenderState for SingleRenderState {
        fn interpolate(&self, other: &Self, r: f32) -> Self {
            Self::new(
                self.transform()
                    .interpolate_limit(other.transform(), r, 10.0),
            )
        }
    }
    #[repr(C)]
    #[derive(Clone, Copy, Zeroable, Default, Pod, Debug, PartialEq)]
    pub(crate) struct InstanceData {
        pub(crate) translation_sz: [f32; 4],
        pub(crate) rotor: [f32; 4],
    }
    vulkano::impl_vertex!(InstanceData, translation_sz, rotor);
}
