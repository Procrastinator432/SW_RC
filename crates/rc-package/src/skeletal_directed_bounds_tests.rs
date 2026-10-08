use super::*;
use crate::{
    quaternion_animation::{PortableQuaternionMath, QuaternionMath},
    skeletal_director_rotation::PortableDirectorRotationHost,
    skeletal_root_pose::quaternion_translation_matrix,
    skeletal_world_bounds::{WorldPoseBounds, WorldPoseBoundsHost},
};
fn pose(x: f32) -> RootTransform {
    RootTransform {
        rotation: [0, 0, 0, 1f32.to_bits()],
        position: [x.to_bits(), 0, 0],
    }
}
fn identity() -> [u32; 16] {
    quaternion_translation_matrix(pose(0.))
}
fn hierarchy() -> BoneHierarchy {
    BoneHierarchy {
        parents: vec![-1, 0, 1],
        descendants: vec![2, 1, 0],
        depths: vec![0, 1, 2],
        move_bone: -1,
    }
}
fn bounds() -> PoseBounds {
    PoseBounds {
        minimum: [99f32.to_bits(); 3],
        maximum: [100f32.to_bits(); 3],
        sphere: [123; 4],
        byte_60: 7,
        byte_61: 8,
        byte_179: 9,
    }
}
fn padding() -> BoundsPadding {
    BoundsPadding {
        minimum: [0; 3],
        maximum: [0; 3],
        k_one: [1f32.to_bits(); 3],
    }
}
fn input<'a>(
    local: &'a [RootTransform],
    h: &'a BoneHierarchy,
    p: &'a BoundsPadding,
) -> DirectedPoseInput<'a> {
    DirectedPoseInput {
        local,
        hierarchy: h,
        editor: false,
        mesh_to_world: identity(),
        actor_scale: [1f32.to_bits(); 3],
        padding: p,
    }
}
fn rotation() -> PortableDirectorRotationHost<PortableQuaternionMath> {
    PortableDirectorRotationHost {
        math: PortableQuaternionMath,
    }
}
struct SeedPolicy {
    calls: usize,
    fail: usize,
}
impl QuaternionMath for SeedPolicy {
    fn reciprocal_sqrt_seed(&mut self, n: f32) -> Result<f32, String> {
        self.calls += 1;
        if self.calls == self.fail {
            Err("seed boundary".into())
        } else {
            Ok(1. / n.sqrt())
        }
    }
    fn spherical_weights(&mut self, _: f32, _: f32) -> Result<[f32; 2], String> {
        panic!("unused")
    }
}
fn world() -> WorldPoseBounds {
    WorldPoseBounds {
        minimum: [77; 3],
        maximum: [88; 3],
        valid: 0,
        sphere: [456; 4],
    }
}
#[test]
fn director_then_child_positions_contribute_before_successful_world_publication() {
    let local = [pose(2.), pose(1.), pose(1.)];
    let h = hierarchy();
    let p = padding();
    let mut b = bounds();
    let mut w = world();
    let mut matrices = vec![];
    let mut d = BoneDirector { words: [0; 28] };
    d.words[0] = 1;
    d.words[18] = 1;
    d.words[19] = 1;
    d.words[14..18].copy_from_slice(&[10f32, 0., 0., 1.].map(f32::to_bits));
    let mut publication = WorldPoseBoundsHost {
        math: PortableQuaternionMath,
        actor: Some((identity(), &mut w)),
    };
    assert_eq!(
        evaluate_directed_pose(
            input(&local, &h, &p),
            &mut [d],
            &mut matrices,
            &mut b,
            &mut rotation(),
            &mut publication
        )
        .unwrap(),
        1
    );
    assert_eq!(
        matrices
            .iter()
            .map(|m| f32::from_bits(m[12]))
            .collect::<Vec<_>>(),
        [2., 10., 11.]
    );
    assert_eq!(b.minimum[0], 1.4000001f32.to_bits());
    assert_eq!(b.maximum[0], 14.200001f32.to_bits());
    assert_eq!((b.byte_60, b.byte_61, b.byte_179), (1, 1, 0));
    assert_eq!(w.minimum, b.minimum);
    assert_eq!(w.maximum, b.maximum);
}
struct FailRotation;
impl DirectorRotationHost for FailRotation {
    fn matrix_quaternion(&mut self, _: [u32; 16]) -> Result<[u32; 4], String> {
        Err("rotation boundary".into())
    }
    fn angle_difference(&mut self, _: [u32; 4], _: [u32; 4]) -> Result<f32, String> {
        panic!("unused")
    }
    fn slerp(&mut self, _: [u32; 4], _: [u32; 4], _: f32) -> Result<[u32; 4], String> {
        panic!("unused")
    }
    fn rotation_angle_fast(&mut self, _: [u32; 4]) -> Result<f32, String> {
        panic!("unused")
    }
    fn power(&mut self, _: [u32; 4], _: f32) -> Result<[u32; 4], String> {
        panic!("unused")
    }
}
#[test]
fn director_failure_keeps_prior_raw_points_but_excludes_current_point_and_completion() {
    let local = [pose(2.), pose(3.), pose(4.)];
    let h = hierarchy();
    let p = padding();
    let mut b = bounds();
    let mut matrices = vec![];
    let mut d = BoneDirector { words: [0; 28] };
    d.words[0] = 1;
    d.words[18] = 0x100;
    d.words[19] = 1;
    let mut publication = WorldPoseBoundsHost {
        math: SeedPolicy { calls: 0, fail: 0 },
        actor: None,
    };
    assert_eq!(
        evaluate_directed_pose(
            input(&local, &h, &p),
            &mut [d],
            &mut matrices,
            &mut b,
            &mut FailRotation,
            &mut publication
        ),
        Err("rotation boundary".into())
    );
    assert_eq!(b.minimum, [2f32.to_bits(), 0, 0]);
    assert_eq!(b.maximum, b.minimum);
    assert_eq!(b.sphere, [123; 4]);
    assert_eq!((b.byte_60, b.byte_61, b.byte_179), (7, 8, 9));
    assert_eq!(matrices[1][12], 5f32.to_bits());
    assert_eq!(matrices[2], [0; 16]);
    assert_eq!(publication.math.calls, 0);
}
#[test]
fn local_seed_failure_keeps_expanded_box_old_spheres_and_flags() {
    let local = [pose(2.), pose(3.), pose(4.)];
    let h = hierarchy();
    let p = padding();
    let mut b = bounds();
    let mut w = world();
    let old = w.clone();
    let mut matrices = vec![];
    let mut publication = WorldPoseBoundsHost {
        math: SeedPolicy { calls: 0, fail: 1 },
        actor: Some((identity(), &mut w)),
    };
    assert!(evaluate_directed_pose(
        input(&local, &h, &p),
        &mut [],
        &mut matrices,
        &mut b,
        &mut rotation(),
        &mut publication
    )
    .is_err());
    assert_eq!(b.maximum[0], 11.8f32.to_bits());
    assert_eq!(b.sphere, [123; 4]);
    assert_eq!((b.byte_60, b.byte_61, b.byte_179), (7, 8, 9));
    assert_eq!(w.minimum, old.minimum);
    assert_eq!(w.sphere, old.sphere);
}
#[test]
fn world_seed_failure_keeps_new_boxes_local_sphere_old_world_sphere_and_flags() {
    let local = [pose(2.), pose(3.), pose(4.)];
    let h = hierarchy();
    let p = padding();
    let mut b = bounds();
    let mut w = world();
    let mut matrices = vec![];
    let mut publication = WorldPoseBoundsHost {
        math: SeedPolicy { calls: 0, fail: 2 },
        actor: Some((identity(), &mut w)),
    };
    assert!(evaluate_directed_pose(
        input(&local, &h, &p),
        &mut [],
        &mut matrices,
        &mut b,
        &mut rotation(),
        &mut publication
    )
    .is_err());
    assert_ne!(b.sphere, [123; 4]);
    assert_eq!((b.byte_60, b.byte_61, b.byte_179), (7, 8, 9));
    assert_eq!(w.minimum, b.minimum);
    assert_eq!(w.maximum, b.maximum);
    assert_eq!(w.valid, 1);
    assert_eq!(w.sphere, [456; 4]);
}
#[test]
fn invalid_parent_stops_after_prior_point_collection_without_finalization() {
    let local = [pose(2.), pose(3.), pose(4.)];
    let mut h = hierarchy();
    h.parents[1] = 2;
    let p = padding();
    let mut b = bounds();
    let mut matrices = vec![];
    let mut publication = WorldPoseBoundsHost {
        math: SeedPolicy { calls: 0, fail: 0 },
        actor: None,
    };
    assert!(evaluate_directed_pose(
        input(&local, &h, &p),
        &mut [],
        &mut matrices,
        &mut b,
        &mut rotation(),
        &mut publication
    )
    .is_err());
    assert_eq!(b.minimum, [2f32.to_bits(), 0, 0]);
    assert_eq!(b.maximum, b.minimum);
    assert_eq!(matrices[1][12], 3f32.to_bits());
    assert_eq!(publication.math.calls, 0);
}
#[test]
fn move_only_pose_keeps_previous_point_bounds_then_completes_without_actor() {
    let local = [pose(200.)];
    let h = BoneHierarchy {
        parents: vec![-1],
        descendants: vec![0],
        depths: vec![0],
        move_bone: 0,
    };
    let p = padding();
    let mut b = bounds();
    let mut matrices = vec![];
    let mut publication = WorldPoseBoundsHost {
        math: SeedPolicy { calls: 0, fail: 0 },
        actor: None,
    };
    assert_eq!(
        evaluate_directed_pose(
            input(&local, &h, &p),
            &mut [],
            &mut matrices,
            &mut b,
            &mut rotation(),
            &mut publication
        )
        .unwrap(),
        0
    );
    assert_eq!(b.minimum, [117.8f32.to_bits(); 3]);
    assert_eq!(b.maximum, [121.00001f32.to_bits(); 3]);
    assert_eq!(publication.math.calls, 1);
    assert_eq!(b.byte_61, 1);
}
