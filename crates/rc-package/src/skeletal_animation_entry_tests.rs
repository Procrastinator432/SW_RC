use super::*;
use crate::skeletal_root_pose::{RootSampleRequest, RootSequence};
use crate::{
    quaternion_animation::PortableQuaternionMath,
    skeletal_director_rotation::PortableDirectorRotationHost,
    skeletal_hierarchy::prepare_hierarchy, skeletal_root_pose::quaternion_translation_matrix,
};
#[derive(Default)]
struct RootHost {
    events: Vec<String>,
    fail: bool,
}
impl RootPoseHost for RootHost {
    fn sequence(
        &mut self,
        i: usize,
        c: &mut [AnimationChannel],
    ) -> Result<Option<RootSequence>, String> {
        self.events.push(format!("sequence {i}"));
        c[i].words[17] = 123;
        Ok(Some(RootSequence {
            token: 123,
            frames: 10,
            track_count_word: 1,
        }))
    }
    fn root_track(&mut self, _: RootSequence) -> Result<i32, String> {
        self.events.push("root track".into());
        Ok(0)
    }
    fn sample(&mut self, r: RootSampleRequest, root: &mut RootTransform) -> Result<(), String> {
        self.events.push("sample".into());
        root.position = pose(10. + r.channel as f32).position;
        if self.fail {
            Err("sample failure".into())
        } else {
            Ok(())
        }
    }
}
impl Fixture {
    fn run_frame(
        &mut self,
        frame_type: i32,
        root: &mut RootHost,
    ) -> Result<AnimationFrameResult, String> {
        evaluate_animation_frame(
            &mut self.state,
            AnimationFrameInput {
                frame_type,
                animation: AnimationFullInput {
                    bones: &self.bones,
                    hierarchy: &self.hierarchy,
                    linkup_count: 2,
                    editor: self.editor,
                    word_11c: self.disabled,
                    actor_scale: [1f32.to_bits(); 3],
                    padding: &self.padding,
                },
            },
            &mut self.inverse,
            &mut self.scratch,
            &mut self.channels,
            &mut self.directors,
            AnimationFrameHosts {
                preparation: &mut self.prep,
                root,
                pose: FullPoseHosts {
                    channels: &mut self.channel,
                    rotation: &mut PortableDirectorRotationHost {
                        math: PortableQuaternionMath,
                    },
                    publication: &mut self.publication,
                },
            },
        )
    }
    fn run_root(&mut self, host: &mut RootHost) -> Result<AnimationRootResult, String> {
        apply_animation_root(
            &mut self.state,
            AnimationRootInput {
                bones: &self.bones,
                linkup_count: 2,
                word_11c: self.disabled,
            },
            &mut self.inverse,
            &mut self.channels,
            &mut self.prep,
            host,
        )
    }
}
#[test]
fn frame_type_three_selects_root_before_cache_and_skips_full_hosts() {
    let mut f = Fixture::new();
    f.state.bounds.byte_61 = 7;
    f.scratch.positions.clear();
    f.hierarchy.parents.clear();
    let mut h = RootHost::default();
    assert!(matches!(
        f.run_frame(3, &mut h).unwrap(),
        AnimationFrameResult::Root(_)
    ));
    assert_eq!(f.state.buffers.positions[0], pose(10.).position);
    assert_eq!(f.prep.calls, ["transform", "linkup 0", "linkup 1"]);
    assert_eq!(f.channel.calls, 0);
    assert_eq!(f.publication.calls, 0);
    assert_eq!(f.state.bounds.byte_61, 7);
}
#[test]
fn all_other_frame_types_select_full_with_one_preparation() {
    for kind in [-1, 0, 1, 2, 4, i32::MIN, i32::MAX] {
        let mut f = Fixture::new();
        let mut h = RootHost::default();
        assert!(matches!(
            f.run_frame(kind, &mut h).unwrap(),
            AnimationFrameResult::Full(_)
        ));
        assert_eq!(f.prep.calls, ["transform", "linkup 0", "linkup 1"]);
        assert_eq!(f.channel.calls, 1);
        assert_eq!(f.publication.calls, 1);
        assert!(h.events.is_empty());
        assert_eq!(f.state.bounds.byte_61, 1);
    }
}
#[test]
fn frame_dispatch_preserves_preparation_failure_for_both_branches() {
    for kind in [0, 3] {
        let mut f = Fixture::new();
        f.prep.fail = true;
        let mut h = RootHost::default();
        assert!(f.run_frame(kind, &mut h).is_err());
        assert_eq!(f.prep.calls, ["transform", "linkup 0"]);
        assert_eq!(f.channel.calls, 0);
        assert!(h.events.is_empty());
        assert_eq!(f.state.buffers.positions[0], pose(7.).position);
    }
}
#[test]
fn root_entry_ignores_pose_cache_and_preserves_nonroot_arrays_bounds_and_scratch() {
    let mut f = Fixture::new();
    f.state.bounds.byte_61 = 7;
    let mut h = RootHost::default();
    assert_eq!(f.run_root(&mut h).unwrap().sampled, 1);
    assert_eq!(f.prep.calls, ["transform", "linkup 0", "linkup 1"]);
    assert_eq!(
        f.state.buffers.positions,
        vec![pose(10.).position, pose(1.).position]
    );
    assert_eq!(f.state.buffers.matrices[0][12], 10f32.to_bits());
    assert_eq!(f.state.buffers.matrices[1], [123; 16]);
    assert_eq!(
        (
            f.state.bounds.byte_60,
            f.state.bounds.byte_61,
            f.state.bounds.byte_179
        ),
        (7, 7, 9)
    );
    assert_eq!(f.state.bounds.sphere, [123; 4]);
    assert_eq!(f.scratch.positions[0], pose(9.).position);
    assert_eq!(f.publication.calls, 0);
    assert_eq!(f.channel.calls, 0);
}
#[test]
fn cold_root_preparation_initializes_tails_without_full_pose_completion() {
    let mut f = Fixture::new();
    f.state.buffers.rotations.clear();
    f.state.buffers.positions.clear();
    f.state.buffers.matrices.clear();
    f.inverse.clear();
    f.state.bounds.byte_61 = 7;
    f.disabled = 1;
    let mut h = RootHost::default();
    let r = f.run_root(&mut h).unwrap();
    assert!(r.preparation.inverse_built);
    assert_eq!(r.sampled, 0);
    assert_eq!(f.state.buffers.positions, vec![pose(3.).position, [0; 3]]);
    assert_eq!(f.state.buffers.rotations[1], [0; 4]);
    assert_eq!(f.state.buffers.matrices[1], [0; 16]);
    assert_eq!(
        (
            f.state.bounds.byte_60,
            f.state.bounds.byte_61,
            f.state.bounds.byte_179
        ),
        (7, 0, 9)
    );
    assert!(h.events.is_empty());
}
#[test]
fn root_sampling_error_preserves_partial_root_and_previous_matrix() {
    let mut f = Fixture::new();
    let mut h = RootHost {
        fail: true,
        ..Default::default()
    };
    assert!(f.run_root(&mut h).is_err());
    assert_eq!(f.state.buffers.positions[0], pose(10.).position);
    assert_eq!(f.state.buffers.rotations[0], pose(3.).rotation);
    assert_eq!(f.state.buffers.matrices[0], [123; 16]);
    assert_eq!(f.channels[0].words[17], 123);
    assert_eq!(f.state.bounds.byte_61, 0);
}
#[test]
fn root_preparation_failure_does_not_reset_root_to_reference() {
    let mut f = Fixture::new();
    f.prep.fail = true;
    let mut h = RootHost::default();
    assert!(f.run_root(&mut h).is_err());
    assert_eq!(f.state.buffers.positions[0], pose(7.).position);
    assert_eq!(f.state.buffers.mesh_to_world[12], 30f32.to_bits());
    assert!(h.events.is_empty());
}
#[test]
fn root_empty_skeleton_errors_after_preparation_without_panicking() {
    let mut f = Fixture::new();
    f.bones.clear();
    let mut h = RootHost::default();
    assert!(f.run_root(&mut h).unwrap_err().contains("reference root"));
    assert!(f.state.buffers.positions.is_empty());
    assert!(f.state.buffers.matrices.is_empty());
    assert_eq!(f.prep.calls, ["transform", "linkup 0", "linkup 1"]);
    assert!(h.events.is_empty());
}
#[test]
fn root_last_eligible_sample_wins_without_channel_history_commit() {
    let mut f = Fixture::new();
    f.channels.push(f.channels[0].clone());
    f.channels[1].words[14] = 0.5f32.to_bits();
    f.channels[1].words[11] = 0.5f32.to_bits();
    f.channels[0].words[12] = 99;
    let mut h = RootHost::default();
    assert_eq!(f.run_root(&mut h).unwrap().sampled, 2);
    assert_eq!(f.state.buffers.positions[0], pose(11.).position);
    assert_eq!(f.channels[0].words[12], 99);
    assert_eq!(f.channels[0].words[17], 123);
    assert_eq!(f.state.buffers.matrices[0][12], 11f32.to_bits());
}
fn pose(x: f32) -> RootTransform {
    RootTransform {
        rotation: [0, 0, 0, 1f32.to_bits()],
        position: [x.to_bits(), 0, 0],
    }
}
fn bone(parent: i32, x: f32) -> StoredBone {
    StoredBone {
        name: crate::skeletal_animation::AnimationName {
            index: 0,
            name: "Root".into(),
        },
        flags: 0,
        rotation: pose(x).rotation,
        position: pose(x).position,
        word_24: 0,
        word_28_first: 0,
        word_28: 0,
        word_30: 0,
        word_38: 0,
        word_34: parent,
    }
}
struct Prep {
    calls: Vec<String>,
    fail: bool,
}
impl AnimationPreparationHost for Prep {
    fn mesh_to_world(&mut self) -> Result<[u32; 16], String> {
        self.calls.push("transform".into());
        Ok(quaternion_translation_matrix(pose(30.)))
    }
    fn refresh_linkup(&mut self, i: usize) -> Result<(), String> {
        self.calls.push(format!("linkup {i}"));
        if self.fail {
            Err("linkup failure".into())
        } else {
            Ok(())
        }
    }
}
struct Channel {
    calls: usize,
    fail: bool,
}
impl PreparedChannelStackHost for Channel {
    fn apply(
        &mut self,
        i: usize,
        c: &mut [AnimationChannel],
        _: &[RootTransform],
        scratch: &mut [RootTransform],
    ) -> Result<bool, String> {
        self.calls += 1;
        c[i].words[12] = 42;
        scratch[0] = pose(10.);
        if self.fail {
            Err("channel failure".into())
        } else {
            Ok(true)
        }
    }
}
struct Publication {
    calls: usize,
    fail: bool,
    cache: Option<u8>,
}
impl PoseBoundsHost for Publication {
    fn reciprocal_sqrt_seed(&mut self, n: f32) -> Result<f32, String> {
        Ok(1. / n.sqrt())
    }
    fn publish(&mut self, b: &PoseBounds) -> Result<(), String> {
        self.calls += 1;
        self.cache = Some(b.byte_61);
        if self.fail {
            Err("publication failure".into())
        } else {
            Ok(())
        }
    }
}
struct Fixture {
    state: AnimationInstance,
    bones: Vec<StoredBone>,
    hierarchy: BoneHierarchy,
    inverse: Vec<[u32; 16]>,
    scratch: ChannelScratchBuffers,
    channels: Vec<AnimationChannel>,
    directors: Vec<BoneDirector>,
    padding: BoundsPadding,
    prep: Prep,
    channel: Channel,
    publication: Publication,
    editor: bool,
    disabled: u32,
}
impl Fixture {
    fn new() -> Self {
        let bones = vec![bone(-1, 3.), bone(0, 1.)];
        let hierarchy = prepare_hierarchy(&bones).unwrap();
        let mut c = AnimationChannel::default();
        c.words[16] = 2;
        c.words[11] = 1f32.to_bits();
        Self {
            state: AnimationInstance {
                buffers: InstanceAnimationBuffers {
                    rotations: vec![pose(7.).rotation; 2],
                    positions: vec![pose(7.).position, pose(1.).position],
                    matrices: vec![[123; 16]; 2],
                    mesh_to_world: [66; 16],
                },
                bounds: PoseBounds {
                    minimum: [99; 3],
                    maximum: [100; 3],
                    sphere: [123; 4],
                    byte_60: 7,
                    byte_61: 0,
                    byte_179: 9,
                },
            },
            bones,
            hierarchy,
            inverse: vec![[88; 16]],
            scratch: ChannelScratchBuffers {
                rotations: vec![pose(9.).rotation; 2],
                positions: vec![pose(9.).position, pose(2.).position],
            },
            channels: vec![c],
            directors: vec![],
            padding: BoundsPadding {
                minimum: [0; 3],
                maximum: [0; 3],
                k_one: [1f32.to_bits(); 3],
            },
            prep: Prep {
                calls: vec![],
                fail: false,
            },
            channel: Channel {
                calls: 0,
                fail: false,
            },
            publication: Publication {
                calls: 0,
                fail: false,
                cache: None,
            },
            editor: false,
            disabled: 0,
        }
    }
    fn run(&mut self) -> Result<AnimationFullResult, String> {
        apply_animation_full(
            &mut self.state,
            AnimationFullInput {
                bones: &self.bones,
                hierarchy: &self.hierarchy,
                linkup_count: 2,
                editor: self.editor,
                word_11c: self.disabled,
                actor_scale: [1f32.to_bits(); 3],
                padding: &self.padding,
            },
            &mut self.inverse,
            &mut self.scratch,
            &mut self.channels,
            &mut self.directors,
            AnimationHosts {
                preparation: &mut self.prep,
                pose: FullPoseHosts {
                    channels: &mut self.channel,
                    rotation: &mut PortableDirectorRotationHost {
                        math: PortableQuaternionMath,
                    },
                    publication: &mut self.publication,
                },
            },
        )
    }
}
#[test]
fn cached_entry_prepares_transform_linkups_before_skipping_invalid_scratch_and_hierarchy() {
    let mut f = Fixture::new();
    f.state.bounds.byte_61 = 7;
    f.scratch.positions.clear();
    f.hierarchy.parents.clear();
    let r = f.run().unwrap();
    assert_eq!(r.pose.channels, ChannelStackResult::Cached);
    assert_eq!(f.prep.calls, ["transform", "linkup 0", "linkup 1"]);
    assert_eq!(f.state.buffers.mesh_to_world[12], 30f32.to_bits());
    assert_eq!(f.state.buffers.matrices, vec![[123; 16]; 2]);
    assert_eq!(f.inverse, vec![[88; 16]]);
    assert_eq!(f.state.bounds.byte_61, 7);
    assert_eq!(f.channel.calls, 0);
    assert_eq!(f.publication.calls, 0);
    assert!(f.scratch.positions.is_empty());
}
#[test]
fn cold_instance_prepares_zero_scratch_then_reference_pose_and_completion() {
    let mut f = Fixture::new();
    f.state.buffers.rotations.clear();
    f.state.buffers.positions.clear();
    f.state.buffers.matrices.clear();
    f.state.bounds.byte_61 = 7;
    f.inverse.clear();
    f.scratch.rotations.clear();
    f.scratch.positions.clear();
    f.disabled = 1;
    let r = f.run().unwrap();
    assert!(r.preparation.inverse_built && r.scratch_resized);
    assert_eq!(r.pose.channels, ChannelStackResult::Reference);
    assert_eq!(
        f.state.buffers.positions,
        vec![pose(3.).position, pose(1.).position]
    );
    assert_eq!(f.scratch.rotations, vec![[0; 4]; 2]);
    assert_eq!(f.scratch.positions, vec![[0; 3]; 2]);
    assert_eq!(f.state.buffers.matrices[1][12], 4f32.to_bits());
    assert_eq!(f.inverse.len(), 2);
    assert_eq!(f.state.bounds.byte_61, 1);
}
#[test]
fn empty_reference_cache_invalidates_an_otherwise_cached_pose() {
    let mut f = Fixture::new();
    f.state.bounds.byte_61 = 7;
    f.inverse.clear();
    let r = f.run().unwrap();
    assert!(r.preparation.inverse_built);
    assert!(matches!(
        r.pose.channels,
        ChannelStackResult::Channels { .. }
    ));
    assert_eq!(f.publication.cache, Some(0));
    assert_eq!(f.state.bounds.byte_61, 1);
}
#[test]
fn preparation_failure_stops_before_scratch_growth_and_channels() {
    let mut f = Fixture::new();
    f.scratch.rotations.clear();
    f.scratch.positions.clear();
    f.prep.fail = true;
    assert!(f.run().is_err());
    assert!(f.scratch.rotations.is_empty());
    assert_eq!(f.channel.calls, 0);
    assert_eq!(f.state.buffers.mesh_to_world[12], 30f32.to_bits());
    assert_eq!(f.prep.calls, ["transform", "linkup 0"]);
}
#[test]
fn channel_error_copies_back_split_scratch_and_keeps_previous_local_pose() {
    let mut f = Fixture::new();
    f.channel.fail = true;
    assert!(f.run().is_err());
    assert_eq!(f.scratch.positions[0], pose(10.).position);
    assert_eq!(f.state.buffers.positions[0], pose(7.).position);
    assert_eq!(f.channels[0].words[12], 42);
    assert_eq!(f.state.buffers.matrices, vec![[123; 16]; 2]);
    assert_eq!(f.state.bounds.byte_61, 0);
    assert_eq!(f.publication.calls, 0);
}
#[test]
fn publication_error_copies_back_committed_pose_and_keeps_incomplete_flags() {
    let mut f = Fixture::new();
    f.publication.fail = true;
    assert!(f.run().is_err());
    assert_eq!(f.state.buffers.positions[0], pose(10.).position);
    assert_eq!(f.state.buffers.matrices[1][12], 12f32.to_bits());
    assert_eq!(f.channels[0].words[12], 42);
    assert_eq!(
        (
            f.state.bounds.byte_60,
            f.state.bounds.byte_61,
            f.state.bounds.byte_179
        ),
        (7, 0, 9)
    );
}
#[test]
fn unpaired_scratch_tail_is_preserved_after_success() {
    let mut f = Fixture::new();
    f.scratch.rotations.push([0x7fc12345; 4]);
    let r = f.run().unwrap();
    assert!(!r.scratch_resized);
    assert_eq!(f.scratch.rotations.len(), 3);
    assert_eq!(f.scratch.rotations[2], [0x7fc12345; 4]);
    assert_eq!(f.scratch.positions.len(), 2);
}
#[test]
fn editor_runs_after_cached_preparation_and_short_scratch_fails_explicitly() {
    let mut f = Fixture::new();
    f.state.bounds.byte_61 = 8;
    f.editor = true;
    f.disabled = 1;
    assert_eq!(
        f.run().unwrap().pose.channels,
        ChannelStackResult::Reference
    );
    assert_eq!(f.publication.cache, Some(8));
    f.state.bounds.byte_61 = 0;
    f.scratch.positions.clear();
    assert!(f.run().unwrap_err().contains("scratch position"));
    assert_eq!(f.publication.calls, 1);
    assert_eq!(f.scratch.rotations.len(), 2);
}
#[test]
fn director_changes_child_matrix_after_channel_commit() {
    let mut f = Fixture::new();
    let mut d = BoneDirector { words: [0; 28] };
    d.words[18] = 1;
    d.words[19] = 1;
    d.words[14..18].copy_from_slice(&[20f32, 0., 0., 1.].map(f32::to_bits));
    f.directors.push(d);
    assert_eq!(f.run().unwrap().pose.directors, 1);
    assert_eq!(f.state.buffers.positions[0], pose(10.).position);
    assert_eq!(f.state.buffers.matrices[1][12], 22f32.to_bits());
    assert_eq!(f.state.bounds.byte_61, 1);
}
