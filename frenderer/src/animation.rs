use crate::types::*;
use color_eyre::eyre::{eyre, Result};
use russimp::bone::Bone as RBone;
use russimp::node::Node;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct Rig {
    pub joints: Vec<Joint>,
    ibms: Vec<Mat4>,
    joints_by_name: HashMap<String, u8>,
}
pub struct Joint {
    transform: Similarity3,
    children: [u8; 4],
}
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable, Default)]
pub struct Bone {
    pub translation_scale: [f32; 4],
    pub rotation: [f32; 4],
}
impl Bone {
    pub fn new(s: Similarity3) -> Self {
        Bone {
            rotation: s.rotation.normalized().into_quaternion_array(),
            translation_scale: [s.translation.x, s.translation.y, s.translation.z, s.scale],
        }
    }
    pub fn rotor(&self) -> Rotor3 {
        Rotor3::from_quaternion_array(self.rotation)
    }
    pub fn translation(&self) -> Vec3 {
        Vec3::new(
            self.translation_scale[0],
            self.translation_scale[1],
            self.translation_scale[2],
        )
    }
    pub fn scale(&self) -> f32 {
        self.translation_scale[3]
    }
    pub fn transform(&self) -> Similarity3 {
        Similarity3::new(self.translation(), self.rotor(), self.scale())
    }
}
impl Rig {
    pub fn which_joint(&self, node_name: &str) -> u8 {
        *self.joints_by_name.get(node_name).unwrap_or_else(|| {
            panic!(
                "No entry found for key {:?} in {:?}",
                node_name,
                self.joints_by_name.keys().collect::<Vec<_>>()
            )
        })
    }
    // Recursively find the node at path bone_root
    fn find_node(root: Rc<RefCell<Node>>, bone_root: &[&str]) -> Option<Rc<RefCell<Node>>> {
        if bone_root.is_empty() {
            None
        } else if root.borrow().name == bone_root[0] {
            if bone_root.len() == 1 {
                return Some(root);
            }
            println!(
                "Look for {:?} in {:?}",
                &bone_root[1..],
                &root.borrow().name
            );
            for c in root.borrow().children.iter() {
                if let Some(n) = Self::find_node(c.clone(), &bone_root[1..]) {
                    return Some(n);
                }
            }
            None
        } else {
            None
        }
    }
    pub fn load(root: Rc<RefCell<Node>>, bones: &[RBone], bone_root: &[&str]) -> Result<Self> {
        let default_bone = russimp::bone::Bone {
            weights: vec![],
            name: root.borrow().name.clone(),
            offset_matrix: russimp::Matrix4x4 {
                a1: 1.0,
                a2: 0.0,
                a3: 0.0,
                a4: 0.0,
                b1: 0.0,
                b2: 1.0,
                b3: 0.0,
                b4: 0.0,
                c1: 0.0,
                c2: 0.0,
                c3: 1.0,
                c4: 0.0,
                d1: 0.0,
                d2: 0.0,
                d3: 0.0,
                d4: 1.0,
            },
        };
        let mut bones: HashMap<_, _> = bones.iter().map(|b| (b.name.clone(), b)).collect();
        // dbg!(bones.keys().collect::<Vec<_>>());
        let mut node_children = Vec::with_capacity(bones.len());
        let mut joints = Vec::with_capacity(bones.len());
        let mut ibms = Vec::with_capacity(bones.len());
        let mut queue = std::collections::VecDeque::with_capacity(bones.len());
        let mut joints_by_name = HashMap::with_capacity(bones.len());
        let root = Self::find_node(root, bone_root)
            .ok_or_else(|| eyre!("Couldn't find bone path {:?}", bone_root))?;
        bones
            .entry(root.borrow().name.clone())
            .or_insert(&default_bone);
        queue.push_back(root);
        while let Some(next) = queue.pop_front() {
            let next = next.borrow();
            // println!("Loading {:?}", &next.name);
            // skip nodes with no corresponding bones
            if !bones.contains_key(&next.name) {
                // println!("Skip {:?}", &next.name);
                continue;
            }
            if joints.len() > 255 {
                panic!("Too many joints");
            }
            joints_by_name.insert(next.name.clone(), joints.len() as u8);
            let transform = Self::into_similarity(Self::mat4_transpose(next.transformation));
            let joint = Joint {
                transform,
                children: [255, 255, 255, 255],
            };
            joints.push(joint);
            ibms.push(Self::mat4_transpose(bones[&next.name].offset_matrix));
            if next.children.len() > 4 {
                panic!("Too many children of {:?}", &next.name);
            }
            queue.extend(next.children.clone());
            node_children.push(next.children.clone());
        }
        queue.clear();
        for (i, n) in node_children.into_iter().enumerate() {
            let joint = &mut joints[i];
            for (child, idx) in n.iter().zip(joint.children.iter_mut()) {
                let child = child.borrow();
                if !bones.contains_key(&child.name) {
                    // println!("Skip {:?}", &child.name);
                    continue;
                }
                if let Some(ji) = joints_by_name.get(&child.name) {
                    *idx = *ji;
                } else {
                    panic!("Unknown joint {:?}", child);
                }
            }
        }
        Ok(Rig {
            joints,
            ibms,
            joints_by_name,
        })
    }

    fn mat4_transpose(trf: russimp::Matrix4x4) -> Mat4 {
        Mat4::from([
            [trf.a1, trf.b1, trf.c1, trf.d1],
            [trf.a2, trf.b2, trf.c2, trf.d2],
            [trf.a3, trf.b3, trf.c3, trf.d3],
            [trf.a4, trf.b4, trf.c4, trf.d4],
        ])
    }

    fn into_similarity(mat: Mat4) -> Similarity3 {
        // per https://math.stackexchange.com/questions/237369/given-this-transformation-matrix-how-do-i-decompose-it-into-translation-rotati#1463487
        let trans = Vec3::new(mat.cols[3][0], mat.cols[3][1], mat.cols[3][2]);
        let mut scale = Vec3::new(mat.cols[0].mag(), mat.cols[1].mag(), mat.cols[2].mag());
        if (scale.x - scale.y).abs() < 0.05 {
            scale.x = scale.y;
        }
        if (scale.x - scale.z).abs() < 0.05 {
            scale.x = scale.z;
        }
        if (scale.y - scale.z).abs() < 0.05 {
            scale.y = scale.z;
        }
        if (scale.x - 1.0).abs() < 0.05 {
            scale.x = 1.0;
        }
        if (scale.y - 1.0).abs() < 0.05 {
            scale.y = 1.0;
        }
        if (scale.z - 1.0).abs() < 0.05 {
            scale.z = 1.0;
        }
        // make sure scale is uniform
        assert!((scale.x - scale.y).abs() < 0.05);
        assert!((scale.y - scale.z).abs() < 0.05);
        assert!((scale.x - scale.z).abs() < 0.05);
        assert!(scale.x > 0.005);
        // figure out rotation
        let rot_mat = Mat4::new(
            mat.cols[0] / scale.x,
            mat.cols[1] / scale.y,
            mat.cols[2] / scale.z,
            Vec4::new(0.0, 0.0, 0.0, 1.0),
        );
        let rot = rot_mat.extract_rotation().normalized();
        Similarity3::new(trans, rot, scale.x)
    }
    pub fn write_bones(&self, bones: &mut Vec<Bone>, anim: &Animation, state: &AnimationState) {
        bones.reserve(self.joints.len());
        let first_bone = bones.len();
        for j in self.joints.iter() {
            bones.push(Bone::new(j.transform));
        }
        // animation sampling goes here, then use the sampled animation to fill the bones
        let mut t = state.t;
        if anim.settings.looping {
            while t >= anim.duration {
                t -= anim.duration;
            }
        }
        for c in anim.channels.iter() {
            let trf = bones[first_bone + c.target as usize].transform();
            bones[first_bone + c.target as usize] = Bone::new(c.sample(t, trf));
        }

        // right now all bones have their positions set in joint-local terms.
        // we need to go from top to bottom to fix that.
        // After this process, every bone's transform data will represent a bone-to-root transform,
        // which we can use to modify vertices (since they're in the model's root coordinate space too).
        for (ji, j) in self.joints.iter().enumerate() {
            // transform all direct child bones by this bone's transformation.
            let btrans = bones[first_bone + ji].transform();
            for &ci in j.children.iter() {
                if ci == 255 {
                    break;
                }
                let b2 = bones[first_bone + ci as usize].transform();
                let b2trans = btrans * b2;
                bones[first_bone + ci as usize] = Bone::new(b2trans);
            }
            // but then we need to multiply by the inverse bind matrix to
            // turn this sampled, transformed bone into a "change in vertex translations"
            let ibm = self.ibms[ji];
            let post_ibm: Mat4 = btrans.into_homogeneous_matrix() * ibm;
            bones[first_bone + ji] = Bone::new(Self::into_similarity(post_ibm));
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct AnimationState {
    pub t: f32,
}
impl AnimationState {
    pub fn interpolate(&self, other: &Self, r: f32) -> Self {
        Self {
            t: self.t.lerp(other.t, r),
        }
    }
    pub fn tick(&mut self, dt: f64) {
        self.t += dt as f32;
    }
}
#[derive(Clone, Copy, Debug)]
pub struct AnimationSettings {
    pub looping: bool,
}
pub struct Animation {
    name: String,
    channels: Vec<Channel>,
    duration: f32,
    settings: AnimationSettings,
}
impl Animation {
    pub fn name(&self) -> &str {
        &self.name
    }
}
#[derive(Debug)]
pub struct Channel {
    name: String,
    target: u8, // joint index
    position_keys: Vec<(f32, Vec3)>,
    rotation_keys: Vec<(f32, Rotor3)>,
    scale_keys: Vec<(f32, Vec3)>,
}
impl Channel {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn sample(&self, t: f32, trf: Similarity3) -> Similarity3 {
        let (p1, p2, pr) = Self::sample_keys(&self.position_keys, trf.translation, t);
        //dbg!(p1,p2,pr);
        let p = p1.lerp(p2, pr);
        let (r1, r2, rr) = Self::sample_keys(&self.rotation_keys, trf.rotation, t);
        let r = r1.lerp(r2, rr).normalized();
        let (s1, s2, sr) = Self::sample_keys(
            &self.scale_keys,
            Vec3::new(trf.scale, trf.scale, trf.scale),
            t,
        );
        let s = s1.lerp(s2, sr).x;
        Similarity3::new(p, r, s)
    }
    fn sample_keys<KT: Copy>(keys: &[(f32, KT)], default: KT, t: f32) -> (KT, KT, f32) {
        keys.windows(2)
            // find a pair of keyframes so t1 <= t and t2 > t
            .find(|ks| ks[0].0 <= t && ks[1].0 > t)
            // extract k1, k2, and r (t-t1)/(t2-t1)
            .map(|ks| (ks[0].1, ks[1].1, (t - ks[0].0) / (ks[1].0 - ks[0].0)))
            // if no such pair, use the last keyframe only and a ratio of 1.0
            .or_else(|| keys.last().map(|k| (k.1, k.1, 1.0)))
            // if there were no keyframes at all, use the default transform
            .unwrap_or((default, default, 1.0))
    }
}

impl Animation {
    pub fn load(
        anim: &russimp::animation::Animation,
        rig: &Rig,
        settings: AnimationSettings,
    ) -> Result<Self> {
        // an animation has several channels. each channel is a target with keyframes.
        // we want to turn this into a representation saying which bones to change when.
        // let's keep the channels and sample from each channel in turn, rather than merging them together
        let name = anim.name.clone();
        let tps = anim.ticks_per_second as f32;
        let duration = anim.duration as f32 / tps;
        let channels: Vec<_> = anim
            .channels
            .iter()
            .map(|c| Channel {
                name: c.name.clone(),
                target: rig.which_joint(&c.name),
                position_keys: c
                    .position_keys
                    .iter()
                    .map(|k| {
                        (
                            k.time as f32 / tps,
                            Vec3::new(k.value.x, k.value.y, k.value.z),
                        )
                    })
                    .collect(),
                rotation_keys: c
                    .rotation_keys
                    .iter()
                    .map(|k| {
                        (
                            k.time as f32 / tps,
                            Rotor3::from_quaternion_array([
                                k.value.x, k.value.y, k.value.z, k.value.w,
                            ]),
                        )
                    })
                    .collect(),
                scale_keys: c
                    .scaling_keys
                    .iter()
                    .map(|k| {
                        (
                            k.time as f32 / tps,
                            Vec3::new(k.value.x, k.value.y, k.value.z),
                        )
                    })
                    .collect(),
            })
            .collect();
        Ok(Animation {
            name,
            duration,
            channels,
            settings,
        })
    }
}
