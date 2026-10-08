use super::*;
use crate::{
    quaternion_animation::PortableQuaternionMath,
    skeletal_director_rotation::PortableDirectorRotationHost,
    skeletal_root_pose::quaternion_translation_matrix,
};
fn pose(x: f32) -> RootTransform {
    RootTransform {
        rotation: [0, 0, 0, 1f32.to_bits()],
        position: [x.to_bits(), 0, 0],
    }
}
struct Channels {
    calls: Vec<usize>,
    fail: bool,
}
impl PreparedChannelStackHost for Channels {
    fn apply(
        &mut self,
        i: usize,
        c: &mut [AnimationChannel],
        previous: &[RootTransform],
        scratch: &mut [RootTransform],
    ) -> Result<bool, String> {
        self.calls.push(i);
        assert_eq!(previous[0].position, pose(7.).position);
        c[i].words[12] = 42;
        scratch[0] = pose(10. + i as f32);
        if self.fail && i == 1 {
            Err("channel failure".into())
        } else {
            Ok(true)
        }
    }
}
struct Publish {
    calls: usize,
    fail: bool,
    seen_cache: Option<u8>,
}
impl PoseBoundsHost for Publish {
    fn reciprocal_sqrt_seed(&mut self, n: f32) -> Result<f32, String> {
        Ok(1. / n.sqrt())
    }
    fn publish(&mut self, b: &PoseBounds) -> Result<(), String> {
        self.calls += 1;
        self.seen_cache = Some(b.byte_61);
        if self.fail {
            Err("publication failure".into())
        } else {
            Ok(())
        }
    }
}
struct Fixture {
    state: PreparedFullPose,
    channels: Vec<AnimationChannel>,
    directors: Vec<BoneDirector>,
    h: BoneHierarchy,
    pad: BoundsPadding,
    host: Channels,
    pubhost: Publish,
    editor: bool,
    disabled: u32,
}
impl Fixture {
    fn new() -> Self {
        let mut c = AnimationChannel::default();
        c.words[16] = 2;
        c.words[11] = 1f32.to_bits();
        Self {
            state: PreparedFullPose {
                local: vec![pose(7.), pose(1.)],
                scratch: vec![pose(9.), pose(2.), pose(88.)],
                matrices: vec![[123; 16]],
                bounds: PoseBounds {
                    minimum: [99; 3],
                    maximum: [100; 3],
                    sphere: [123; 4],
                    byte_60: 7,
                    byte_61: 0,
                    byte_179: 9,
                },
            },
            channels: vec![c],
            directors: vec![],
            h: BoneHierarchy {
                parents: vec![-1, 0],
                descendants: vec![1, 0],
                depths: vec![0, 1],
                move_bone: -1,
            },
            pad: BoundsPadding {
                minimum: [0; 3],
                maximum: [0; 3],
                k_one: [1f32.to_bits(); 3],
            },
            host: Channels {
                calls: vec![],
                fail: false,
            },
            pubhost: Publish {
                calls: 0,
                fail: false,
                seen_cache: None,
            },
            editor: false,
            disabled: 0,
        }
    }
    fn run(&mut self) -> Result<FullPoseResult, String> {
        apply_full_pose_prepared(
            &mut self.state,
            FullPoseInput {
                reference: &[pose(3.), pose(1.)],
                hierarchy: &self.h,
                editor: self.editor,
                word_11c: self.disabled,
                mesh_to_world: quaternion_translation_matrix(pose(0.)),
                actor_scale: [1f32.to_bits(); 3],
                padding: &self.pad,
            },
            &mut self.channels,
            &mut self.directors,
            FullPoseHosts {
                channels: &mut self.host,
                rotation: &mut PortableDirectorRotationHost {
                    math: PortableQuaternionMath,
                },
                publication: &mut self.pubhost,
            },
        )
    }
}
#[test]
fn cache_returns_before_buffer_validation_and_all_hosts() {
    let mut f = Fixture::new();
    f.state.bounds.byte_61 = 1;
    f.state.local.clear();
    f.state.scratch.clear();
    f.h.parents.clear();
    assert_eq!(
        f.run().unwrap(),
        FullPoseResult {
            channels: ChannelStackResult::Cached,
            directors: 0
        }
    );
    assert!(f.host.calls.is_empty());
    assert_eq!(f.pubhost.calls, 0);
    assert_eq!(f.state.matrices, vec![[123; 16]]);
    assert_eq!(f.state.bounds.sphere, [123; 4]);
}
#[test]
fn successful_completion_owns_next_call_cache_and_preserves_scratch_tail() {
    let mut f = Fixture::new();
    let result = f.run().unwrap();
    assert_eq!(
        result.channels,
        ChannelStackResult::Channels {
            called: 1,
            applied: 1
        }
    );
    assert_eq!(f.state.local[0].position, pose(10.).position);
    assert_eq!(f.state.matrices[1][12], 12f32.to_bits());
    assert_eq!(f.state.scratch[2].position, pose(88.).position);
    assert_eq!(f.pubhost.seen_cache, Some(0));
    assert_eq!(
        (
            f.state.bounds.byte_60,
            f.state.bounds.byte_61,
            f.state.bounds.byte_179
        ),
        (1, 1, 0)
    );
    assert_eq!(f.run().unwrap().channels, ChannelStackResult::Cached);
    assert_eq!(f.host.calls, [0]);
    assert_eq!(f.pubhost.calls, 1);
}
#[test]
fn editor_bypasses_nonzero_cache_and_disabled_channels_use_reference() {
    let mut f = Fixture::new();
    f.state.bounds.byte_61 = 8;
    f.editor = true;
    f.disabled = 1;
    assert_eq!(f.run().unwrap().channels, ChannelStackResult::Reference);
    assert!(f.host.calls.is_empty());
    assert_eq!(f.state.local[0].position, pose(3.).position);
    assert_eq!(f.pubhost.seen_cache, Some(8));
    assert_eq!(f.state.bounds.byte_61, 1);
}
#[test]
fn channel_error_retains_history_and_scratch_before_commit_and_matrices() {
    let mut f = Fixture::new();
    f.channels.push(f.channels[0].clone());
    f.channels[1].words[14] = 0.5f32.to_bits();
    f.host.fail = true;
    assert!(f.run().is_err());
    assert_eq!(f.host.calls, [0, 1]);
    assert_eq!(f.channels[0].words[12], 42);
    assert_eq!(f.channels[1].words[12], 42);
    assert_eq!(f.state.scratch[0].position, pose(11.).position);
    assert_eq!(f.state.local[0].position, pose(7.).position);
    assert_eq!(f.state.matrices, vec![[123; 16]]);
    assert_eq!(f.state.bounds.byte_61, 0);
    assert_eq!(f.pubhost.calls, 0);
}
#[test]
fn publication_error_preserves_committed_channel_pose_and_incomplete_flags() {
    let mut f = Fixture::new();
    f.pubhost.fail = true;
    assert!(f.run().is_err());
    assert_eq!(f.state.local[0].position, pose(10.).position);
    assert_eq!(f.channels[0].words[12], 42);
    assert_eq!(f.state.matrices[1][12], 12f32.to_bits());
    assert_ne!(f.state.bounds.sphere, [123; 4]);
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
fn director_changes_channel_result_before_child_and_bounds() {
    let mut f = Fixture::new();
    let mut d = BoneDirector { words: [0; 28] };
    d.words[18] = 1;
    d.words[19] = 1;
    d.words[14..18].copy_from_slice(&[20f32, 0., 0., 1.].map(f32::to_bits));
    f.directors.push(d);
    assert_eq!(f.run().unwrap().directors, 1);
    assert_eq!(f.state.local[0].position, pose(10.).position);
    assert_eq!(f.state.matrices[0][12], 20f32.to_bits());
    assert_eq!(f.state.matrices[1][12], 22f32.to_bits());
    assert!(f32::from_bits(f.state.bounds.maximum[0]) > 22.);
}
