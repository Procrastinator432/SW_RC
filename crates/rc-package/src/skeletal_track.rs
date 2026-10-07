//! FSkelAnimSeq.GetRotPos (10500fc0), on explicitly supplied track snapshots.
//! QuaternionTrackHost provides decode/Slerp with an explicit CPU math policy.
//! Upstream x87 frame multiplication remains a host boundary.
use crate::skeletal_root_pose::RootTransform;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimationTrack {
    pub rotations: Vec<[u16; 3]>,
    pub rotation_count_word: u32,
    pub positions: Vec<[i16; 3]>,
    pub position_count_word: u32,
    /// Native MOVSS at track+10: this is a float bit pattern, not a signed integer.
    pub position_scale_bits: u32,
    pub durations: Vec<u8>,
    pub duration_count_word: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct KeySelection {
    pub current: usize,
    pub next: usize,
    /// None means direct key; Some holds the interpolation factor's f32 bits.
    pub alpha_bits: Option<u32>,
}

pub trait TrackRotationHost {
    fn decode(&mut self, compressed: [u16; 3]) -> Result<[u32; 4], String>;
    fn slerp(&mut self, current: [u32; 4], next: [u32; 4], alpha: f32) -> Result<[u32; 4], String>;
}

/// The native code traverses the durations ONCE; it does not use time modulo.
pub fn select_keys(track: &AnimationTrack, time: f32) -> Result<KeySelection, String> {
    let count = ((track.duration_count_word << 3) as i32) >> 3;
    if count <= 1 {
        return Ok(KeySelection {
            current: 0,
            next: 0,
            alpha_bits: None,
        });
    }
    let count = count as usize;
    let mut residual = time;
    let mut index = 0;
    let duration = loop {
        let duration = *track.durations.get(index).ok_or("missing duration key")? as f32;
        let after = residual - duration;
        if after < 0.0 {
            break duration;
        }
        residual = after;
        index += 1;
        if index == count {
            index = 0;
            break track.durations[0] as f32;
        }
    };
    // COMISS/JBE also takes the direct-key path for unordered comparisons.
    if residual.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
        Ok(KeySelection {
            current: index,
            next: index,
            alpha_bits: None,
        })
    } else {
        Ok(KeySelection {
            current: index,
            next: if index + 1 < count { index + 1 } else { 0 },
            alpha_bits: Some((residual / duration).to_bits()),
        })
    }
}

fn position(track: &AnimationTrack, index: usize) -> Result<[f32; 3], String> {
    let key = track.positions.get(index).ok_or("missing position key")?;
    let scale = f32::from_bits(track.position_scale_bits) * f32::from_bits(0x38000100);
    Ok(key.map(|v| v as f32 * scale))
}

/// Rotation is committed before position reads, preserving native partial writes.
/// Invalid snapshots return errors instead of reproducing native out-of-bounds reads.
pub fn sample_track(
    track: &AnimationTrack,
    time: f32,
    output: &mut RootTransform,
    host: &mut impl TrackRotationHost,
) -> Result<KeySelection, String> {
    let keys = select_keys(track, time)?;
    let rotation_index = if track.rotation_count_word & 0x1fffffff == 1 {
        0
    } else {
        keys.current
    };
    let rotation = |index: usize| {
        track
            .rotations
            .get(index)
            .copied()
            .ok_or_else(|| "missing rotation key".to_owned())
    };
    output.rotation = if let Some(alpha) = keys
        .alpha_bits
        .filter(|_| track.rotation_count_word & 0x1fffffff != 1)
    {
        // The native calls decode(next), then decode(current), then Slerp(current,next).
        let next = host.decode(rotation(keys.next)?)?;
        let current = host.decode(rotation(rotation_index)?)?;
        host.slerp(current, next, f32::from_bits(alpha))?
    } else {
        host.decode(rotation(rotation_index)?)?
    };
    let singleton = track.position_count_word & 0x1fffffff == 1;
    output.position = if let Some(alpha) = keys.alpha_bits.filter(|_| !singleton) {
        let next = position(track, keys.next)?;
        let current = position(track, keys.current)?;
        let alpha = f32::from_bits(alpha);
        std::array::from_fn(|i| ((next[i] - current[i]) * alpha + current[i]).to_bits())
    } else {
        position(track, if singleton { 0 } else { keys.current })?.map(f32::to_bits)
    };
    Ok(keys)
}

/// GetLinkupFromSeq compares the opaque sequence+64 key against linkup word+4.
/// Supplied entries retain native order; no speculative interpretation of the key.
pub fn find_linkup(entries: &[[u32; 4]], sequence_key: Option<u32>) -> Option<usize> {
    let key = sequence_key?;
    entries.iter().position(|entry| entry[1] == key)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn track() -> AnimationTrack {
        AnimationTrack {
            rotations: vec![[10, 0, 0], [20, 0, 0], [30, 0, 0]],
            rotation_count_word: 3,
            positions: vec![[0, -32767, 32767], [32767, 0, -32767], [-32767, 32767, 0]],
            position_count_word: 3,
            position_scale_bits: 2f32.to_bits(),
            durations: vec![2, 3, 5],
            duration_count_word: 3,
        }
    }
    #[derive(Default)]
    struct Host {
        events: Vec<u16>,
        fail: bool,
    }
    impl TrackRotationHost for Host {
        fn decode(&mut self, key: [u16; 3]) -> Result<[u32; 4], String> {
            self.events.push(key[0]);
            Ok([key[0] as u32; 4])
        }
        fn slerp(&mut self, a: [u32; 4], b: [u32; 4], t: f32) -> Result<[u32; 4], String> {
            self.events.push(99);
            if self.fail {
                Err("slerp".into())
            } else {
                Ok([a[0], b[0], t.to_bits(), 0])
            }
        }
    }
    fn root() -> RootTransform {
        RootTransform {
            rotation: [7; 4],
            position: [8; 3],
        }
    }
    #[test]
    fn boundaries_and_single_pass_wrap() {
        let t = track();
        for (time, index, alpha) in [
            (-1., 0, None),
            (0., 0, None),
            (1., 0, Some(0.5)),
            (2., 1, None),
            (5., 2, None),
            (7.5, 2, Some(0.5)),
            (10., 0, None),
            (15., 0, Some(2.5)),
        ] {
            let k = select_keys(&t, time).unwrap();
            assert_eq!(k.current, index);
            assert_eq!(k.alpha_bits, alpha.map(f32::to_bits));
        }
    }
    #[test]
    fn nan_zero_durations_and_count_tags() {
        let mut t = track();
        t.duration_count_word = 0xa0000003;
        assert_eq!(select_keys(&t, f32::NAN).unwrap().current, 0);
        assert_eq!(select_keys(&t, f32::NAN).unwrap().alpha_bits, None);
        t.durations = vec![0, 0, 0];
        assert_eq!(
            select_keys(&t, 1.).unwrap().alpha_bits,
            Some(f32::INFINITY.to_bits())
        );
        t.duration_count_word = 0x1fffffff;
        t.durations.clear();
        assert_eq!(select_keys(&t, 9.).unwrap().alpha_bits, None);
    }
    #[test]
    fn float_scale_and_interpolation_decode_order() {
        let t = track();
        let mut r = root();
        let mut h = Host::default();
        sample_track(&t, 1., &mut r, &mut h).unwrap();
        assert_eq!(h.events, vec![20, 10, 99]);
        assert_eq!(r.rotation, [10, 20, 0.5f32.to_bits(), 0]);
        assert_eq!(r.position, [1., -1., 0.].map(f32::to_bits));
    }
    #[test]
    fn singleton_keys_ignore_selected_index() {
        let mut t = track();
        t.rotation_count_word = 0xe0000001;
        t.position_count_word = 0xa0000001;
        t.rotations.truncate(1);
        t.positions.truncate(1);
        let mut r = root();
        let mut h = Host::default();
        sample_track(&t, 7., &mut r, &mut h).unwrap();
        assert_eq!(h.events, vec![10]);
        assert_eq!(r.position, [0., -2., 2.].map(f32::to_bits));
    }
    #[test]
    fn short_duration_branch_reads_first_even_zero_counts() {
        let mut t = track();
        t.duration_count_word = 0;
        t.durations.clear();
        t.rotation_count_word = 0;
        t.position_count_word = 0;
        let mut r = root();
        sample_track(&t, f32::INFINITY, &mut r, &mut Host::default()).unwrap();
        assert_eq!(r.rotation, [10; 4]);
        assert_eq!(r.position, [0., -2., 2.].map(f32::to_bits));
    }
    #[test]
    fn error_preserves_native_output_order() {
        let mut t = track();
        let mut r = root();
        t.positions.clear();
        assert!(sample_track(&t, 0., &mut r, &mut Host::default()).is_err());
        assert_eq!(r.rotation, [10; 4]);
        assert_eq!(r.position, [8; 3]);
        let mut r = root();
        let mut h = Host {
            fail: true,
            ..Default::default()
        };
        assert!(sample_track(&track(), 1., &mut r, &mut h).is_err());
        assert_eq!(r.rotation, [7; 4]);
        assert_eq!(r.position, [8; 3]);
        t.durations.truncate(1);
        let mut r = root();
        assert!(sample_track(&t, 3., &mut r, &mut h).is_err());
        assert_eq!(r.rotation, [7; 4]);
    }
    #[test]
    fn linkup_is_first_match_and_none_is_no_sequence() {
        let entries = [[1, 0, 4, 0], [2, 7, 8, 0], [3, 7, 9, 0]];
        assert_eq!(find_linkup(&entries, Some(7)), Some(1));
        assert_eq!(find_linkup(&entries, Some(0)), Some(0));
        assert_eq!(find_linkup(&entries, None), None);
        assert_eq!(find_linkup(&entries, Some(6)), None);
    }

    #[test]
    fn prepared_root_can_sample_supplied_track_and_build_matrix() {
        use crate::{mesh_animation::AnimationChannel, skeletal_root_pose::*};
        struct Adapter;
        impl TrackRotationHost for Adapter {
            fn decode(&mut self, _: [u16; 3]) -> Result<[u32; 4], String> {
                Ok([0, 0, 0, 1f32.to_bits()])
            }
            fn slerp(&mut self, a: [u32; 4], _: [u32; 4], _: f32) -> Result<[u32; 4], String> {
                Ok(a)
            }
        }
        impl RootPoseHost for Adapter {
            fn sequence(
                &mut self,
                _: usize,
                _: &mut [AnimationChannel],
            ) -> Result<Option<RootSequence>, String> {
                Ok(Some(RootSequence {
                    token: 123,
                    frames: 2,
                    track_count_word: 1,
                }))
            }
            fn root_track(&mut self, _: RootSequence) -> Result<i32, String> {
                Ok(find_linkup(&[[0, 99, 0, 0]], Some(99)).unwrap() as i32)
            }
            fn sample(
                &mut self,
                request: RootSampleRequest,
                root: &mut RootTransform,
            ) -> Result<(), String> {
                // This fixture's 2 * 0.5 = 1 is exact at all relevant precisions.
                assert_eq!(request.normalized_frame, 0.5);
                assert_eq!(request.frames, 2);
                sample_track(&track(), 1., root, self).map(|_| ())
            }
        }
        let mut channel = AnimationChannel::default();
        channel.words[7] = 0.5f32.to_bits();
        channel.words[16] = 3;
        let mut state = PreparedRootPose {
            root: root(),
            matrix: [7; 16],
            byte_60: 1,
            byte_61: 9,
        };
        assert_eq!(
            apply_root_prepared(&mut state, root(), &mut [channel], 0, &mut Adapter).unwrap(),
            1
        );
        assert_eq!(state.root.position, [1., -1., 0.].map(f32::to_bits));
        assert_eq!(&state.matrix[12..15], &state.root.position);
        assert_eq!((state.byte_60, state.byte_61), (1, 9));
    }
}
