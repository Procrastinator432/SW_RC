use super::*;
use crate::{
    quaternion_animation::PortableQuaternionMath,
    skeletal_director_rotation::PortableDirectorRotationHost,
};
fn identity() -> [u32; 16] {
    quaternion_translation_matrix(RootTransform {
        rotation: [0, 0, 0, 1f32.to_bits()],
        position: [0; 3],
    })
}
fn director(bone: u32, flags: u32) -> BoneDirector {
    let mut words = [0; 28];
    words[0] = bone;
    words[1] = bone;
    words[2..18].copy_from_slice(&identity());
    words[18] = flags;
    words[19] = 1;
    words[20] = (-1f32).to_bits();
    words[21] = (-1f32).to_bits();
    BoneDirector { words }
}
fn host() -> PortableDirectorRotationHost<PortableQuaternionMath> {
    PortableDirectorRotationHost {
        math: PortableQuaternionMath,
    }
}
#[test]
fn absolute_rotation_scales_columns_clears_w_and_preserves_translation() {
    let mut d = director(0, 0x100);
    d.words[2 + 3] = 3f32.to_bits();
    let mut m = [identity()];
    m[0][12..16].copy_from_slice(&[7f32, 8., 9., 2.].map(f32::to_bits));
    apply_director_prepared(
        &mut d,
        0,
        &mut m,
        identity(),
        [2f32, 3., 4.].map(f32::to_bits),
        &mut host(),
    )
    .unwrap();
    assert_eq!(
        [m[0][0], m[0][5], m[0][10]],
        [2f32, 3., 4.].map(f32::to_bits)
    );
    assert_eq!(m[0][3], 0);
    assert_eq!(&m[0][12..16], &[7f32, 8., 9., 2.].map(f32::to_bits));
    assert_eq!(d.words[27], 1);
}
#[test]
fn final_translation_then_component_scale_uses_working_matrix_again() {
    let mut d = director(0, 0x10101);
    d.words[2] = 2f32.to_bits();
    d.words[14..18].copy_from_slice(&[3f32, 4., 5., 6.].map(f32::to_bits));
    let mut m = [identity()];
    apply_director_prepared(
        &mut d,
        0,
        &mut m,
        identity(),
        [3f32.to_bits(); 3],
        &mut host(),
    )
    .unwrap();
    assert_eq!(m[0][0], 12f32.to_bits());
    assert_eq!(&m[0][12..16], &d.words[14..18]);
}
#[test]
fn relative_composition_inherits_current_and_no_prior_range_is_allowed() {
    let mut d = director(0, 0x101);
    d.words[19] = 3;
    d.words[14] = 2f32.to_bits();
    let mut m = [identity()];
    m[0][12] = 5f32.to_bits();
    apply_director_prepared(
        &mut d,
        0,
        &mut m,
        identity(),
        [1f32.to_bits(); 3],
        &mut host(),
    )
    .unwrap();
    assert_eq!(m[0][12], 7f32.to_bits());
}
struct PowerHost {
    alphas: Vec<f32>,
    fail: usize,
}
impl DirectorRotationHost for PowerHost {
    fn matrix_quaternion(&mut self, _: [u32; 16]) -> Result<[u32; 4], String> {
        Ok([0, 0, 0, 1f32.to_bits()])
    }
    fn angle_difference(&mut self, _: [u32; 4], _: [u32; 4]) -> Result<f32, String> {
        panic!("disabled")
    }
    fn slerp(&mut self, _: [u32; 4], _: [u32; 4], _: f32) -> Result<[u32; 4], String> {
        panic!("disabled")
    }
    fn rotation_angle_fast(&mut self, _: [u32; 4]) -> Result<f32, String> {
        panic!("disabled")
    }
    fn power(&mut self, _: [u32; 4], alpha: f32) -> Result<[u32; 4], String> {
        self.alphas.push(alpha);
        if self.alphas.len() == self.fail {
            Err("power boundary".into())
        } else {
            Ok([0, 0, alpha.to_bits(), 1f32.to_bits()])
        }
    }
}
#[test]
fn correction_walks_previous_indices_backwards_preserving_translation() {
    let mut d = director(3, 0x100);
    d.words[1] = 0;
    d.words[19] = 3;
    let mut m = [identity(); 4];
    for (i, v) in m.iter_mut().enumerate() {
        v[12] = (i as f32 + 7.).to_bits();
    }
    let mut h = PowerHost {
        alphas: vec![],
        fail: 0,
    };
    apply_director_prepared(&mut d, 3, &mut m, identity(), [1f32.to_bits(); 3], &mut h).unwrap();
    assert_eq!(h.alphas, vec![0.75, 0.5, 0.25]);
    for (i, v) in m.iter().enumerate() {
        assert_eq!(v[12], (i as f32 + 7.).to_bits());
    }
    assert_ne!(m[0][0], 1f32.to_bits());
    assert_ne!(m[2][0], m[0][0]);
}
#[test]
fn correction_failure_keeps_history_current_rows_and_completed_previous_rows() {
    let mut d = director(3, 0x10101);
    d.words[1] = 0;
    d.words[19] = 3;
    d.words[2] = 2f32.to_bits();
    d.words[14] = 9f32.to_bits();
    let mut m = [identity(); 4];
    let mut h = PowerHost {
        alphas: vec![],
        fail: 2,
    };
    assert!(
        apply_director_prepared(&mut d, 3, &mut m, identity(), [1f32.to_bits(); 3], &mut h)
            .is_err()
    );
    assert_eq!(d.words[27], 1);
    assert_eq!(d.words[25], 1f32.to_bits());
    assert_eq!(m[3][0], 2f32.to_bits());
    assert_eq!(m[3][12], 0);
    assert_eq!(m[1], identity());
    assert_ne!(m[2], identity());
    assert_eq!(m[0], identity());
}
#[test]
fn mutable_pose_selects_first_active_director_and_child_inherits_rotation() {
    let locals = [
        RootTransform {
            rotation: [0, 0, 0, 1f32.to_bits()],
            position: [0; 3],
        },
        RootTransform {
            rotation: [0, 0, 0, 1f32.to_bits()],
            position: [1f32.to_bits(), 0, 0],
        },
    ];
    let h = BoneHierarchy {
        parents: vec![-1, 0],
        descendants: vec![1, 0],
        depths: vec![0, 1],
        move_bone: -1,
    };
    let mut disabled = director(0, 0);
    disabled.words[27] = 0x55;
    let mut active = director(0x80000000, 0x100);
    active.words[2] = 2f32.to_bits();
    let mut directors = [disabled, active, director(0, 0x100)];
    let mut output = vec![];
    assert_eq!(
        build_mutable_directed_pose(
            &locals,
            &h,
            false,
            &mut directors,
            identity(),
            [1f32.to_bits(); 3],
            &mut host(),
            &mut output
        )
        .unwrap(),
        1
    );
    assert_eq!(directors[0].words[27], 0x55);
    assert_eq!(directors[1].words[27], 1);
    assert_eq!(directors[2].words[27], 0);
    assert_eq!(output[1][12], 2f32.to_bits());
}
