//! ApplyAnimChannel (engine 10501420) on prepared buffers and resolved sequence/linkup.
//! Loaded tracks are connected by TrackChannelHost; native x87 precision remains a policy boundary.
use crate::{mesh_animation::AnimationChannel, skeletal_root_pose::RootTransform};

/// Policy boundary for the original x87 angular comparison and f32 ratio store.
pub trait ChannelAngularMath {
    fn limit(
        &mut self,
        old: [u32; 4],
        current: [u32; 4],
        previous: [u32; 4],
        progress: f32,
    ) -> Result<Option<f32>, String>;
}

/// f64 approximation of x87 AngleDiffFast; not instruction/rounding-mode emulation.
#[derive(Default)]
pub struct PortableChannelAngularMath;
impl ChannelAngularMath for PortableChannelAngularMath {
    fn limit(
        &mut self,
        old: [u32; 4],
        current: [u32; 4],
        previous: [u32; 4],
        progress: f32,
    ) -> Result<Option<f32>, String> {
        fn angle(a: [u32; 4], b: [u32; 4]) -> f64 {
            let a = a.map(|v| f32::from_bits(v) as f64);
            let b = b.map(|v| f32::from_bits(v) as f64);
            let mut dot = (((a[3] * b[3]) + a[2] * b[2]) + a[1] * b[1]) + a[0] * b[0];
            if dot < 0.0 {
                dot = -dot;
            }
            if dot > 1.0 {
                dot = 1.0;
            }
            2.0 * dot.acos()
        }
        // First call is stored to f32, second call remains extended in the native sum.
        let distance = angle(old, current) as f32;
        let allowed = angle(current, previous) + distance as f64 * progress as f64;
        Ok(if allowed < distance as f64 {
            Some((allowed as f32) / distance)
        } else {
            None
        })
    }
}

/// Connects loaded original tracks and the existing quaternion implementation.
pub struct TrackChannelHost<'a, M, A> {
    pub tracks: &'a [crate::skeletal_track::AnimationTrack],
    pub math: M,
    pub angular: A,
}
impl<M: crate::quaternion_animation::QuaternionMath, A: ChannelAngularMath> SkeletalChannelHost
    for TrackChannelHost<'_, M, A>
{
    fn sample(&mut self, track: usize, time: f32) -> Result<RootTransform, String> {
        let track = self.tracks.get(track).ok_or("channel track unavailable")?;
        let mut out = RootTransform {
            rotation: [0; 4],
            position: [0; 3],
        };
        // Borrow the configured math policy; do not silently replace it per sample.
        struct Rotation<'a, M>(&'a mut M);
        impl<M: crate::quaternion_animation::QuaternionMath>
            crate::skeletal_track::TrackRotationHost for Rotation<'_, M>
        {
            fn decode(&mut self, key: [u16; 3]) -> Result<[u32; 4], String> {
                crate::quaternion_animation::decode_rotation(key, self.0)
            }
            fn slerp(&mut self, a: [u32; 4], b: [u32; 4], t: f32) -> Result<[u32; 4], String> {
                crate::quaternion_animation::slerp_rotation(a, b, t, self.0)
            }
        }
        crate::skeletal_track::sample_track(track, time, &mut out, &mut Rotation(&mut self.math))?;
        Ok(out)
    }
    fn slerp(&mut self, a: [u32; 4], b: [u32; 4], t: f32) -> Result<[u32; 4], String> {
        crate::quaternion_animation::slerp_rotation(a, b, t, &mut self.math)
    }
    fn angular_limit(
        &mut self,
        a: [u32; 4],
        b: [u32; 4],
        p: [u32; 4],
        t: f32,
    ) -> Result<Option<f32>, String> {
        self.angular.limit(a, b, p, t)
    }
    fn reciprocal_sqrt_seed(&mut self, n: f32) -> Result<f32, String> {
        self.math.reciprocal_sqrt_seed(n)
    }
}

pub trait SkeletalChannelHost {
    fn sample(&mut self, track: usize, time: f32) -> Result<RootTransform, String>;
    fn slerp(&mut self, a: [u32; 4], b: [u32; 4], alpha: f32) -> Result<[u32; 4], String>;
    /// AngleDiffFast(old,current)*progress + AngleDiffFast(current,previous).
    /// Return its ratio to the first angle only if strictly smaller (ordered).
    /// Original comparison uses the unrounded x87 sum; ratio uses its f32 store.
    fn angular_limit(
        &mut self,
        old: [u32; 4],
        current: [u32; 4],
        previous: [u32; 4],
        progress: f32,
    ) -> Result<Option<f32>, String>;
    fn reciprocal_sqrt_seed(&mut self, squared: f32) -> Result<f32, String>;
}

/// FCOMIP(.5,t)/JC: unordered follows the upper arm too.
pub fn channel_ease(t: f32) -> f32 {
    if t.partial_cmp(&0.5) != Some(std::cmp::Ordering::Less) && t != 0.5 {
        let rest = 1.0f32 - t;
        1.0f32 - (rest * rest) * 2.0f32
    } else {
        (t * t) * 2.0f32
    }
}
pub fn blend_progress(current: f32, previous: f32) -> f32 {
    if current == 1.0 {
        1.0
    } else {
        (channel_ease(current) - channel_ease(previous)) / (1.0 - channel_ease(previous))
    }
}
fn frame_time(frame: f32, frames: i32) -> f32 {
    let clamped = if frame.partial_cmp(&0.0) == Some(std::cmp::Ordering::Less) || frame.is_nan() {
        0.0
    } else if frame >= 1.0 {
        1.0
    } else {
        frame
    };
    frames as f32 * clamped
}
fn length(delta: [f32; 3], host: &mut impl SkeletalChannelHost) -> Result<f32, String> {
    let [x, y, z] = delta;
    let n = (z * z + y * y) + x * x;
    let r = host.reciprocal_sqrt_seed(n)?;
    let result = ((3.0f32 - (r * n) * r) * (r * 0.5f32)) * n;
    Ok(if n == 0.0 { 0.0 } else { result })
}

pub struct ChannelPoseInput<'a> {
    /// None means missing sequence or masked track count zero.
    pub frames: Option<i32>,
    pub mapping: &'a [i32],
    pub reference: &'a [RootTransform],
    pub previous: &'a [RootTransform],
    pub move_bone: i32,
}

/// Caller has already resolved GetSequence and GetLinkupFromSeq and allocated scratch.
/// Later buffer/host errors preserve earlier bone writes, without updating frame history.
pub fn apply_channel(
    channel: &mut AnimationChannel,
    channel_index: usize,
    input: ChannelPoseInput<'_>,
    scratch: &mut [RootTransform],
    host: &mut impl SkeletalChannelHost,
) -> Result<bool, String> {
    let Some(frames) = input.frames else {
        channel.words[13] = channel.words[11];
        channel.words[12] = channel.words[7];
        return Ok(false);
    };
    let weight = if channel_index == 0 {
        1.0
    } else {
        f32::from_bits(channel.words[14])
    };
    let current_time = frame_time(f32::from_bits(channel.words[7]), frames);
    let previous_time = frame_time(f32::from_bits(channel.words[12]), frames);
    let blend = f32::from_bits(channel.words[11]);
    let progress = blend_progress(blend, f32::from_bits(channel.words[13]));
    let start = channel.words[15] as i32;
    let end = channel.words[16] as i32;
    for signed in start..end {
        let bone = usize::try_from(signed).map_err(|_| "negative channel bone")?;
        let track = *input.mapping.get(bone).ok_or("missing bone mapping")?;
        let out = scratch.get_mut(bone).ok_or("missing scratch bone")?;
        if track < 0 {
            *out = *input.reference.get(bone).ok_or("missing reference bone")?;
            continue;
        }
        if blend == 0.0 {
            *out = *input.previous.get(bone).ok_or("missing previous bone")?;
            continue;
        }
        let mut target = host.sample(track as usize, current_time)?;
        // COMISS(1,blend)/JBE skips for >=1 and unordered.
        if blend < 1.0 && signed > input.move_bone {
            let sampled_previous = host.sample(track as usize, previous_time)?;
            let old = *input.previous.get(bone).ok_or("missing previous bone")?;
            if let Some(alpha) = host.angular_limit(
                old.rotation,
                target.rotation,
                sampled_previous.rotation,
                progress,
            )? {
                target.rotation = host.slerp(old.rotation, target.rotation, alpha)?;
            }
            let a = old.position.map(f32::from_bits);
            let b = target.position.map(f32::from_bits);
            let p = sampled_previous.position.map(f32::from_bits);
            let delta = std::array::from_fn(|i| b[i] - a[i]);
            let distance = length(delta, host)?;
            let motion = length(std::array::from_fn(|i| p[i] - b[i]), host)?;
            let allowed = distance * progress + motion;
            if allowed < distance {
                let alpha = allowed / distance;
                target.position = std::array::from_fn(|i| (delta[i] * alpha + a[i]).to_bits());
            }
        }
        // COMISS(1,weight)/JBE also copies directly on NaN.
        if weight.partial_cmp(&1.0) != Some(std::cmp::Ordering::Less) {
            *out = target;
        } else {
            out.rotation =
                host.slerp(out.rotation, target.rotation, channel_ease(blend) * weight)?;
            let a = out.position.map(f32::from_bits);
            let b = target.position.map(f32::from_bits);
            // Preserve original ADDSS operand order; NaN payloads may distinguish it.
            #[allow(clippy::if_same_then_else)]
            let position = std::array::from_fn(|i| {
                let d = (b[i] - a[i]) * weight;
                (if i == 0 { a[i] + d } else { d + a[i] }).to_bits()
            });
            out.position = position;
        }
    }
    channel.words[13] = channel.words[11];
    channel.words[12] = channel.words[7];
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portable_angular_budget_shortest_path_clamp_and_limit() {
        let id = [0., 0., 0., 1.].map(f32::to_bits);
        let turn = [0., 0., 0.6, 0.8].map(f32::to_bits);
        let mut math = PortableChannelAngularMath;
        let alpha = math.limit(id, turn, turn, 0.25).unwrap().unwrap();
        assert!((alpha - 0.25).abs() < 1e-6);
        assert!(math.limit(id, turn, id, 0.25).unwrap().is_none());
        assert!(math.limit(id, id, id, 0.).unwrap().is_none());
        assert!(math.limit(id, turn, turn, f32::NAN).unwrap().is_none());
        let negative = turn.map(|v| (-f32::from_bits(v)).to_bits());
        assert!((math.limit(id, negative, negative, 0.25).unwrap().unwrap() - alpha).abs() < 1e-6);
        let oversized = [0., 0., 0., 2.].map(f32::to_bits);
        assert!(math.limit(id, oversized, oversized, 0.).unwrap().is_none());
    }
    #[test]
    fn track_adapter_decodes_real_format_and_propagates_missing_keys() {
        use crate::{
            quaternion_animation::{decode_rotation, PortableQuaternionMath},
            skeletal_track::AnimationTrack,
        };
        let track = AnimationTrack {
            rotations: vec![[0, 0, 0]],
            rotation_count_word: 1,
            positions: vec![[32767, -32767, 0]],
            position_count_word: 1,
            position_scale_bits: 2f32.to_bits(),
            durations: vec![1],
            duration_count_word: 1,
        };
        let mut host = TrackChannelHost {
            tracks: std::slice::from_ref(&track),
            math: PortableQuaternionMath,
            angular: PortableChannelAngularMath,
        };
        let out = host.sample(0, 0.).unwrap();
        assert_eq!(
            out.rotation,
            decode_rotation([0, 0, 0], &mut PortableQuaternionMath).unwrap()
        );
        assert_eq!(out.position, [2f32.to_bits(), (-2f32).to_bits(), 0]);
        assert!(host.sample(1, 0.).is_err());
        let mut bad = track;
        bad.positions.clear();
        let mut host = TrackChannelHost {
            tracks: &[bad],
            math: PortableQuaternionMath,
            angular: PortableChannelAngularMath,
        };
        assert!(host.sample(0, 0.).is_err());
    }
    #[derive(Default)]
    struct Host {
        events: Vec<(usize, u32)>,
        alphas: Vec<f32>,
        fail: bool,
    }
    fn pose(x: f32) -> RootTransform {
        RootTransform {
            rotation: [0, 0, 0, 1f32.to_bits()],
            position: [x.to_bits(), 0, 0],
        }
    }
    impl SkeletalChannelHost for Host {
        fn sample(&mut self, t: usize, time: f32) -> Result<RootTransform, String> {
            self.events.push((t, time.to_bits()));
            if self.fail {
                Err("sample".into())
            } else {
                Ok(pose(10.0))
            }
        }
        fn slerp(&mut self, a: [u32; 4], _: [u32; 4], alpha: f32) -> Result<[u32; 4], String> {
            self.alphas.push(alpha);
            Ok(a)
        }
        fn angular_limit(
            &mut self,
            _: [u32; 4],
            _: [u32; 4],
            _: [u32; 4],
            _: f32,
        ) -> Result<Option<f32>, String> {
            Ok(None)
        }
        fn reciprocal_sqrt_seed(&mut self, n: f32) -> Result<f32, String> {
            Ok(1.0 / n.sqrt())
        }
    }
    fn channel(blend: f32, weight: f32) -> AnimationChannel {
        let mut c = AnimationChannel::default();
        c.words[11] = blend.to_bits();
        c.words[14] = weight.to_bits();
        c.words[7] = 0.75f32.to_bits();
        c.words[12] = 0.25f32.to_bits();
        c.words[16] = 1;
        c
    }
    fn run(
        c: &mut AnimationChannel,
        index: usize,
        map: &[i32],
        frames: Option<i32>,
        out: &mut [RootTransform],
        host: &mut Host,
    ) -> Result<bool, String> {
        apply_channel(
            c,
            index,
            ChannelPoseInput {
                frames,
                mapping: map,
                reference: &[pose(20.)],
                previous: &[pose(0.)],
                move_bone: 0,
            },
            out,
            host,
        )
    }
    #[test]
    fn empty_sequence_still_updates_raw_history() {
        let mut c = channel(f32::NAN, 0.);
        let mut h = Host::default();
        assert!(!run(&mut c, 0, &[], None, &mut [], &mut h).unwrap());
        assert_eq!(c.words[13], c.words[11]);
        assert_eq!(c.words[12], c.words[7]);
        assert!(h.events.is_empty());
    }
    #[test]
    fn missing_mapping_overrides_weight_and_zero_blend() {
        let mut c = channel(0., 0.);
        let mut out = [pose(7.)];
        let mut h = Host::default();
        run(&mut c, 1, &[-1], Some(8), &mut out, &mut h).unwrap();
        assert_eq!(out[0].position, pose(20.).position);
        run(&mut c, 1, &[0], Some(8), &mut out, &mut h).unwrap();
        assert_eq!(out[0].position, pose(0.).position);
        assert!(h.events.is_empty());
    }
    #[test]
    fn root_weight_and_secondary_rotation_easing_differ() {
        let mut c = channel(0.25, 0.5);
        let mut out = [pose(0.)];
        let mut h = Host::default();
        run(&mut c, 1, &[0], Some(8), &mut out, &mut h).unwrap();
        assert_eq!(h.alphas, [0.0625]);
        assert_eq!(out[0].position, pose(5.).position);
        run(&mut c, 0, &[0], Some(8), &mut out, &mut h).unwrap();
        assert_eq!(out[0].position, pose(10.).position);
    }
    #[test]
    fn frame_clamps_and_nan_weight_direct_copy() {
        for (frame, time) in [(f32::NAN, 0f32), (-1., 0.), (2., 8.), (-0., -0.)] {
            let mut c = channel(1., f32::NAN);
            c.words[7] = frame.to_bits();
            let mut h = Host::default();
            let mut out = [pose(0.)];
            run(&mut c, 1, &[0], Some(8), &mut out, &mut h).unwrap();
            assert_eq!(h.events, [(0, time.to_bits())]);
            assert!(h.alphas.is_empty());
        }
    }
    #[test]
    fn completed_bones_survive_failure_without_history_update() {
        let mut c = channel(1., 1.);
        c.words[16] = 2;
        let mut out = [pose(7.); 2];
        let mut h = Host {
            fail: true,
            ..Host::default()
        };
        assert!(run(&mut c, 0, &[-1, 0], Some(8), &mut out, &mut h).is_err());
        assert_eq!(out[0].position, pose(20.).position);
        assert_eq!(out[1].position, pose(7.).position);
        assert_eq!(c.words[12], 0.25f32.to_bits());
        assert_eq!(c.words[13], 0);
    }
    #[test]
    fn transition_limits_translation_and_samples_current_before_previous() {
        let mut c = channel(0.5, 1.);
        let mut out = [pose(0.)];
        let mut h = Host::default();
        apply_channel(
            &mut c,
            0,
            ChannelPoseInput {
                frames: Some(8),
                mapping: &[0],
                reference: &[],
                previous: &[pose(0.)],
                move_bone: -1,
            },
            &mut out,
            &mut h,
        )
        .unwrap();
        assert_eq!(h.events, [(0, 6f32.to_bits()), (0, 2f32.to_bits())]);
        assert!((f32::from_bits(out[0].position[0]) - 5.).abs() < 1e-6);
    }
    #[test]
    fn ease_and_progress_are_unclamped() {
        assert_eq!(channel_ease(0.25), 0.125);
        assert_eq!(channel_ease(0.75), 0.875);
        assert_eq!(channel_ease(2.), -1.);
        assert_eq!(blend_progress(1., 1.), 1.);
        assert!(blend_progress(0.5, 1.).is_infinite());
        assert!(channel_ease(f32::NAN).is_nan());
    }
}
