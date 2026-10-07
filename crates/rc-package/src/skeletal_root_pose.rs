//! ApplyAnimation's root-only branch AFTER native buffer/cache/director preparation.
//! Track decoding and x87 frame-time multiplication remain explicit host operations.
use crate::{mesh_animation::AnimationChannel, move_coords::channel_active};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct RootTransform {
    pub rotation: [u32; 4],
    pub position: [u32; 3],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreparedRootPose {
    pub root: RootTransform,
    pub matrix: [u32; 16],
    pub byte_60: u8,
    pub byte_61: u8,
}
#[derive(Clone, Copy, Debug)]
pub struct RootSequence {
    pub token: u32,
    pub frames: i32,
    pub track_count_word: u32,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub struct RootSampleRequest {
    pub channel: usize,
    pub sequence_token: u32,
    pub track: i32,
    pub frames: i32,
    pub normalized_frame: f32,
}
pub trait RootPoseHost {
    fn sequence(
        &mut self,
        index: usize,
        channels: &mut [AnimationChannel],
    ) -> Result<Option<RootSequence>, String>;
    fn root_track(&mut self, sequence: RootSequence) -> Result<i32, String>;
    /// Native computes FILD(frames)*normalized_frame, stores f32, then GetRotPos.
    /// Host owns x87 precision, track data and partial output writes on failure.
    fn sample(
        &mut self,
        request: RootSampleRequest,
        root: &mut RootTransform,
    ) -> Result<(), String>;
}

/// Original core.dll FMatrix(FQuat,FVector): separate SSE operations, no normalization.
pub fn quaternion_translation_matrix(root: RootTransform) -> [u32; 16] {
    let [x, y, z, w] = root.rotation.map(f32::from_bits);
    let x2 = x * 2.0f32;
    let y2 = y * 2.0f32;
    let z2 = z * 2.0f32;
    let xx = x * x2;
    let yy = y2 * y;
    let zz = z2 * z;
    let wx = w * x2;
    let wy = w * y2;
    let wz = w * z2;
    let xy = x * y2;
    let xz = x * z2;
    let yz = z2 * y;
    let mut m = [0; 16];
    m[0] = (1.0f32 - (zz + yy)).to_bits();
    m[1] = (wz + xy).to_bits();
    m[2] = (xz - wy).to_bits();
    m[4] = (xy - wz).to_bits();
    m[5] = (1.0f32 - (zz + xx)).to_bits();
    m[6] = (wx + yz).to_bits();
    m[8] = (wy + xz).to_bits();
    m[9] = (yz - wx).to_bits();
    m[10] = (1.0f32 - (yy + xx)).to_bits();
    m[12..15].copy_from_slice(&root.position);
    m[15] = 1.0f32.to_bits();
    m
}
pub fn apply_root_prepared(
    state: &mut PreparedRootPose,
    reference: RootTransform,
    channels: &mut [AnimationChannel],
    word_11c: u32,
    host: &mut impl RootPoseHost,
) -> Result<usize, String> {
    state.root = reference;
    let mut sampled = 0;
    for index in 0..channels.len() {
        if !channel_active(channels, index as i32, -1)
            || word_11c != 0
            || channels[index].words[15] != 0
            || channels[index].words[16] as i32 <= 0
        {
            continue;
        }
        let Some(sequence) = host.sequence(index, channels)? else {
            continue;
        };
        if sequence.track_count_word & 0x1fffffff == 0 {
            continue;
        }
        let track = host.root_track(sequence)?;
        if track < 0 {
            continue;
        }
        let frame = f32::from_bits(channels[index].words[7]);
        let normalized = if frame.partial_cmp(&0.0).is_none() || frame < 0.0 {
            0.0
        } else if frame >= 1.0 {
            1.0
        } else {
            frame
        };
        host.sample(
            RootSampleRequest {
                channel: index,
                sequence_token: sequence.token,
                track,
                frames: sequence.frames,
                normalized_frame: normalized,
            },
            &mut state.root,
        )?;
        sampled += 1;
    }
    state.matrix = quaternion_translation_matrix(state.root);
    // Root-only exits before full-pose writes to instance bytes 60/61/179.
    Ok(sampled)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reference() -> RootTransform {
        RootTransform {
            rotation: [0, 0, 0, 1.0f32.to_bits()],
            position: [1.0f32.to_bits(), 2.0f32.to_bits(), 3.0f32.to_bits()],
        }
    }
    fn state() -> PreparedRootPose {
        PreparedRootPose {
            root: reference(),
            matrix: [7; 16],
            byte_60: 1,
            byte_61: 9,
        }
    }
    fn channel() -> AnimationChannel {
        let mut c = AnimationChannel::default();
        c.words[16] = 3;
        c.words[14] = 1.0f32.to_bits();
        c.words[11] = 0.5f32.to_bits();
        c.words[7] = 0.25f32.to_bits();
        c
    }
    struct Host {
        events: Vec<String>,
        sequence: bool,
        tracks: u32,
        track: i32,
        fail: bool,
        requests: Vec<RootSampleRequest>,
    }
    impl Default for Host {
        fn default() -> Self {
            Self {
                events: vec![],
                sequence: true,
                tracks: 1,
                track: 4,
                fail: false,
                requests: vec![],
            }
        }
    }
    impl RootPoseHost for Host {
        fn sequence(
            &mut self,
            i: usize,
            c: &mut [AnimationChannel],
        ) -> Result<Option<RootSequence>, String> {
            self.events.push(format!("seq{i}"));
            c[i].words[17] = 123;
            Ok(self.sequence.then_some(RootSequence {
                token: 123,
                frames: 10,
                track_count_word: self.tracks,
            }))
        }
        fn root_track(&mut self, _: RootSequence) -> Result<i32, String> {
            self.events.push("linkup".into());
            Ok(self.track)
        }
        fn sample(&mut self, r: RootSampleRequest, root: &mut RootTransform) -> Result<(), String> {
            self.events.push("sample".into());
            self.requests.push(r);
            root.position[0] = (10.0 + r.channel as f32).to_bits();
            if self.fail {
                Err("track sampling".into())
            } else {
                Ok(())
            }
        }
    }
    #[test]
    fn native_matrix_identity_translation_and_nonunit_quaternion() {
        let m = quaternion_translation_matrix(reference());
        assert_eq!(
            m,
            [
                1.0f32.to_bits(),
                0,
                0,
                0,
                0,
                1.0f32.to_bits(),
                0,
                0,
                0,
                0,
                1.0f32.to_bits(),
                0,
                1.0f32.to_bits(),
                2.0f32.to_bits(),
                3.0f32.to_bits(),
                1.0f32.to_bits()
            ]
        );
        let mut r = reference();
        r.rotation = [0, 0, 2.0f32.to_bits(), 0];
        let m = quaternion_translation_matrix(r);
        assert_eq!(f32::from_bits(m[0]), -7.0);
        assert_eq!(f32::from_bits(m[5]), -7.0);
        assert_eq!(f32::from_bits(m[10]), 1.0);
        r.position = [(-0.0f32).to_bits(), 0x7fc01234, 0x7f800000];
        assert_eq!(&quaternion_translation_matrix(r)[12..15], &r.position);
    }
    #[test]
    fn prepared_root_branch_samples_looping_zero_rate_and_preserves_flags() {
        let mut c = vec![channel()];
        c[0].words[1] = 1;
        c[0].words[6] = 0;
        let mut s = state();
        let mut h = Host::default();
        assert_eq!(
            apply_root_prepared(&mut s, reference(), &mut c, 0, &mut h).unwrap(),
            1
        );
        assert_eq!(h.events, ["seq0", "linkup", "sample"]);
        assert_eq!(s.root.position[0], 10.0f32.to_bits());
        assert_eq!((s.byte_60, s.byte_61), (1, 9));
        assert_eq!(s.matrix[12], 10.0f32.to_bits());
    }
    #[test]
    fn root_branch_gates_and_tagged_track_count() {
        for mode in 0..5 {
            let mut c = vec![channel()];
            let mut s = state();
            let mut h = Host::default();
            if mode == 1 {
                c[0].words[15] = 1;
            }
            if mode == 2 {
                c[0].words[16] = 0;
            }
            if mode == 3 {
                h.tracks = 0xe0000000;
            }
            if mode == 4 {
                h.track = -1;
            }
            assert_eq!(
                apply_root_prepared(&mut s, reference(), &mut c, u32::from(mode == 0), &mut h)
                    .unwrap(),
                0
            );
            assert_eq!(s.matrix, quaternion_translation_matrix(reference()));
            assert_eq!(
                h.events.len(),
                match mode {
                    3 => 1,
                    4 => 2,
                    _ => 0,
                }
            );
        }
    }
    #[test]
    fn frame_clamp_nan_and_negative_zero() {
        for (input, expected) in [(-1.0, 0.0f32), (2.0, 1.0), (f32::NAN, 0.0), (-0.0, -0.0)] {
            let mut c = vec![channel()];
            c[0].words[7] = input.to_bits();
            let mut h = Host::default();
            apply_root_prepared(&mut state(), reference(), &mut c, 0, &mut h).unwrap();
            assert_eq!(h.requests[0].normalized_frame.to_bits(), expected.to_bits());
        }
    }
    #[test]
    fn later_sample_wins_and_failure_keeps_partial_root_and_old_matrix() {
        let mut c = vec![channel(); 2];
        let mut s = state();
        let mut h = Host::default();
        apply_root_prepared(&mut s, reference(), &mut c, 0, &mut h).unwrap();
        assert_eq!(s.matrix[12], 11.0f32.to_bits());
        let mut s = state();
        let mut h = Host {
            fail: true,
            ..Host::default()
        };
        assert!(apply_root_prepared(&mut s, reference(), &mut c, 0, &mut h).is_err());
        assert_eq!(s.root.position[0], 10.0f32.to_bits());
        assert_eq!(s.matrix, [7; 16]);
        assert_eq!(s.byte_61, 9);
    }
}
