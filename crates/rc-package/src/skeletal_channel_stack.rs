//! ApplyAnimation 1050a615..1050a719/1050ab6f..1050abe4, after preparation.
//! Scratch is supplied and retained, not implicitly initialized to the reference pose.
use crate::{
    mesh_animation::AnimationChannel, move_coords::channel_active,
    skeletal_root_pose::RootTransform,
};
use serde::Serialize;

pub trait PreparedChannelStackHost {
    /// GetSequence/ApplyAnimChannel may update channel words. Array length/identity must remain stable.
    /// Previous instance pose stays unchanged until the complete channel loop succeeds.
    fn apply(
        &mut self,
        index: usize,
        channels: &mut [AnimationChannel],
        previous: &[RootTransform],
        scratch: &mut [RootTransform],
    ) -> Result<bool, String>;
}
#[derive(Debug, Serialize, PartialEq, Eq)]
pub enum ChannelStackResult {
    Cached,
    Reference,
    Channels { called: usize, applied: usize },
}
pub struct PreparedChannelPose {
    pub local: Vec<RootTransform>,
    /// Persistent external/native global scratch, including untouched bones.
    pub scratch: Vec<RootTransform>,
    pub byte_61: u8,
}
pub struct PreparedChannelBuffers<'a> {
    pub local: &'a mut [RootTransform],
    pub scratch: &'a mut [RootTransform],
    pub byte_61: u8,
}
pub fn apply_channels_prepared(
    state: &mut PreparedChannelPose,
    reference: &[RootTransform],
    channels: &mut [AnimationChannel],
    editor: bool,
    word_11c: u32,
    host: &mut impl PreparedChannelStackHost,
) -> Result<ChannelStackResult, String> {
    apply_channels_to_buffers(
        PreparedChannelBuffers {
            local: &mut state.local,
            scratch: &mut state.scratch,
            byte_61: state.byte_61,
        },
        reference,
        channels,
        editor,
        word_11c,
        host,
    )
}
pub fn apply_channels_to_buffers(
    state: PreparedChannelBuffers<'_>,
    reference: &[RootTransform],
    channels: &mut [AnimationChannel],
    editor: bool,
    word_11c: u32,
    host: &mut impl PreparedChannelStackHost,
) -> Result<ChannelStackResult, String> {
    if state.byte_61 != 0 && !editor {
        return Ok(ChannelStackResult::Cached);
    }
    if state.local.len() != reference.len() || state.scratch.len() < state.local.len() {
        return Err("channel stack buffers must be prepared".into());
    }
    let mut called = 0;
    let mut applied = 0;
    if word_11c == 0 {
        for index in 0..channels.len() {
            if channel_active(channels, index as i32, -1) {
                called += 1;
                if host.apply(index, channels, state.local, state.scratch)? {
                    applied += 1;
                }
            }
        }
    }
    if applied != 0 {
        let n = state.local.len();
        state.local.copy_from_slice(&state.scratch[..n]);
        Ok(ChannelStackResult::Channels { called, applied })
    } else {
        state.local.copy_from_slice(reference);
        Ok(ChannelStackResult::Reference)
    }
    // Cache completion flags belong AFTER matrices/directors/bounds and are not set here.
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pose(x: u32) -> RootTransform {
        RootTransform {
            rotation: [0; 4],
            position: [x; 3],
        }
    }
    fn c(start: u32, end: u32, weight: f32, blend: f32) -> AnimationChannel {
        let mut c = AnimationChannel::default();
        c.words[15] = start;
        c.words[16] = end;
        c.words[14] = weight.to_bits();
        c.words[11] = blend.to_bits();
        c
    }
    struct Host {
        seen: Vec<(usize, u32, u32)>,
        responses: Vec<Result<bool, String>>,
    }
    impl PreparedChannelStackHost for Host {
        fn apply(
            &mut self,
            i: usize,
            c: &mut [AnimationChannel],
            p: &[RootTransform],
            s: &mut [RootTransform],
        ) -> Result<bool, String> {
            self.seen.push((i, p[0].position[0], s[0].position[0]));
            if c[i].words[15] < c[i].words[16] {
                s[0] = pose(100 + i as u32);
            }
            c[i].words[12] = 42;
            self.responses[i].clone()
        }
    }
    fn state() -> PreparedChannelPose {
        PreparedChannelPose {
            local: vec![pose(7)],
            scratch: vec![pose(9), pose(88)],
            byte_61: 0,
        }
    }
    #[test]
    fn loop_preserves_previous_pose_and_shares_scratch_until_commit() {
        let mut s = state();
        let mut cs = [c(0, 1, 0., 1.), c(0, 1, 0.5, 1.)];
        let mut h = Host {
            seen: vec![],
            responses: vec![Ok(true), Ok(true)],
        };
        assert_eq!(
            apply_channels_prepared(&mut s, &[pose(3)], &mut cs, false, 0, &mut h).unwrap(),
            ChannelStackResult::Channels {
                called: 2,
                applied: 2
            }
        );
        assert_eq!(h.seen, [(0, 7, 9), (1, 7, 100)]);
        assert_eq!(s.local[0].position, [101; 3]);
        assert_eq!(s.scratch[1].position, [88; 3]);
        assert_eq!(s.byte_61, 0);
    }
    #[test]
    fn later_full_weight_occludes_earlier_even_when_missing_sequence() {
        let mut s = state();
        let mut cs = [c(0, 1, 1., 1.), c(0, 1, 1., 1.)];
        let mut h = Host {
            seen: vec![],
            responses: vec![Ok(true), Ok(false)],
        };
        assert_eq!(
            apply_channels_prepared(&mut s, &[pose(3)], &mut cs, false, 0, &mut h).unwrap(),
            ChannelStackResult::Reference
        );
        assert_eq!(h.seen, [(1, 7, 9)]);
        assert_eq!(s.local[0].position, [3; 3]);
        assert_eq!(cs[0].words[12], 0);
    }
    #[test]
    fn cached_and_disabled_paths_do_not_call_or_clear_scratch() {
        let mut s = state();
        s.byte_61 = 2;
        let mut cs = [c(0, 1, 1., 1.)];
        let mut h = Host {
            seen: vec![],
            responses: vec![],
        };
        assert_eq!(
            apply_channels_prepared(&mut s, &[], &mut cs, false, 0, &mut h).unwrap(),
            ChannelStackResult::Cached
        );
        assert_eq!(s.local[0].position, [7; 3]);
        assert_eq!(
            apply_channels_prepared(&mut s, &[pose(3)], &mut cs, true, 1, &mut h).unwrap(),
            ChannelStackResult::Reference
        );
        assert_eq!(s.scratch[0].position, [9; 3]);
        assert_eq!(s.byte_61, 2);
        assert!(h.seen.is_empty());
    }
    #[test]
    fn error_keeps_old_local_and_partial_scratch_and_history() {
        let mut s = state();
        let mut cs = [c(0, 1, 1., 1.), c(0, 1, 0.5, 1.)];
        let mut h = Host {
            seen: vec![],
            responses: vec![Ok(true), Err("boundary".into())],
        };
        assert!(apply_channels_prepared(&mut s, &[pose(3)], &mut cs, false, 0, &mut h).is_err());
        assert_eq!(s.local[0].position, [7; 3]);
        assert_eq!(s.scratch[0].position, [101; 3]);
        assert_eq!(cs[0].words[12], 42);
    }
    #[test]
    fn empty_interval_success_still_commits_supplied_scratch() {
        let mut s = state();
        let mut cs = [c(0, 0, 0., 1.)];
        let mut h = Host {
            seen: vec![],
            responses: vec![Ok(true)],
        };
        assert_eq!(
            apply_channels_prepared(&mut s, &[pose(3)], &mut cs, false, 0, &mut h).unwrap(),
            ChannelStackResult::Channels {
                called: 1,
                applied: 1
            }
        );
        assert_eq!(s.local[0].position, [9; 3]);
        assert!(apply_channels_prepared(&mut s, &[], &mut cs, false, 0, &mut h).is_err());
    }
}
