//! ApplyAnimation point bounds, local sphere and completion after supplied world publication.
//! Supports per-bone collection and completion with explicit publication policy.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoseBounds {
    pub minimum: [u32; 3],
    pub maximum: [u32; 3],
    pub sphere: [u32; 4],
    pub byte_60: u8,
    pub byte_61: u8,
    pub byte_179: u8,
}
pub trait PoseBoundsHost {
    fn reciprocal_sqrt_seed(&mut self, squared: f32) -> Result<f32, String>;
    /// Perform optional actor-space bounds publication, or no-op when actor +48 is null.
    /// Completion flags are written only after this operation succeeds.
    fn publish(&mut self, bounds: &PoseBounds) -> Result<(), String>;
}
pub struct BoundsPadding {
    /// Runtime mesh vectors at +29c and +2a8; deliberately supplied, not guessed from the mesh prefix.
    pub minimum: [u32; 3],
    pub maximum: [u32; 3],
    /// Runtime imported FVector::kOne. Caller supplies the resolved global value.
    pub k_one: [u32; 3],
}
pub const BOUNDS_SCALE: f32 = f32::from_bits(0x3f99999a);
/// ApplyAnimation 1050ac31..1050acf8, after the current bone's director finishes.
pub fn accumulate_pose_point(state: &mut PoseBounds, bone: usize, move_bone: i32, point: [u32; 3]) {
    let first = move_bone.wrapping_add(1);
    if bone as i32 == first {
        state.minimum = point;
        state.maximum = point;
    } else if bone as i32 > first {
        for (axis, &v) in point.iter().enumerate() {
            let point = f32::from_bits(v);
            // COMISS/JBE ignores unordered rather than f32::min/max NaN rules.
            if f32::from_bits(state.minimum[axis]) > point {
                state.minimum[axis] = v;
            } else if point > f32::from_bits(state.maximum[axis]) {
                state.maximum[axis] = v;
            }
        }
    }
}
/// Convenience completion for callers whose matrices are already fully evaluated.
pub fn finish_prepared_bounds(
    state: &mut PoseBounds,
    matrices: &[[u32; 16]],
    move_bone: i32,
    padding: &BoundsPadding,
    host: &mut impl PoseBoundsHost,
) -> Result<(), String> {
    for (bone, m) in matrices.iter().enumerate() {
        accumulate_pose_point(state, bone, move_bone, [m[12], m[13], m[14]]);
    }
    finish_accumulated_bounds(state, padding, host)
}
// Keep original ADDSS operand order, including its axis-dependent differences.
#[allow(clippy::if_same_then_else)]
pub fn finish_accumulated_bounds(
    state: &mut PoseBounds,
    padding: &BoundsPadding,
    host: &mut impl PoseBoundsHost,
) -> Result<(), String> {
    let low = padding.minimum.map(f32::from_bits);
    let high = padding.maximum.map(f32::from_bits);
    let one = padding.k_one.map(f32::from_bits);
    state.minimum =
        std::array::from_fn(|i| (f32::from_bits(state.minimum[i]) * BOUNDS_SCALE).to_bits());
    state.maximum =
        std::array::from_fn(|i| (f32::from_bits(state.maximum[i]) * BOUNDS_SCALE).to_bits());
    for i in 0..3 {
        let offset = if i == 0 {
            one[i] + low[i]
        } else {
            low[i] + one[i]
        };
        state.minimum[i] = (f32::from_bits(state.minimum[i]) - offset).to_bits();
        let offset = high[i] + one[i];
        let max = f32::from_bits(state.maximum[i]);
        state.maximum[i] = (if i == 2 { max + offset } else { offset + max }).to_bits();
    }
    let min = state.minimum.map(f32::from_bits);
    let max = state.maximum.map(f32::from_bits);
    let d: [f32; 3] = std::array::from_fn(|i| max[i] - min[i]);
    let squared = (d[0] * d[0] + d[1] * d[1]) + d[2] * d[2];
    let r = host.reciprocal_sqrt_seed(squared)?;
    let distance = ((3.0f32 - (r * squared) * r) * (r * 0.5f32)) * squared;
    let distance = if squared == 0.0 { 0.0 } else { distance };
    let center: [f32; 3] = std::array::from_fn(|i| {
        let sum = if i == 1 {
            min[i] + max[i]
        } else {
            max[i] + min[i]
        };
        sum * 0.5f32
    });
    state.sphere = [
        center[0].to_bits(),
        center[1].to_bits(),
        center[2].to_bits(),
        (distance * 0.5f32).to_bits(),
    ];
    host.publish(state)?;
    state.byte_60 = 1;
    state.byte_61 = 1;
    state.byte_179 = 0;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Host {
        calls: Vec<f32>,
        fail_seed: bool,
        fail_publish: bool,
        published: Option<PoseBounds>,
    }
    impl PoseBoundsHost for Host {
        fn reciprocal_sqrt_seed(&mut self, n: f32) -> Result<f32, String> {
            self.calls.push(n);
            if self.fail_seed {
                Err("seed".into())
            } else {
                Ok(1. / n.sqrt())
            }
        }
        fn publish(&mut self, b: &PoseBounds) -> Result<(), String> {
            self.published = Some(b.clone());
            if self.fail_publish {
                Err("publish".into())
            } else {
                Ok(())
            }
        }
    }
    fn state() -> PoseBounds {
        PoseBounds {
            minimum: [0; 3],
            maximum: [0; 3],
            sphere: [99; 4],
            byte_60: 7,
            byte_61: 8,
            byte_179: 9,
        }
    }
    fn pad() -> BoundsPadding {
        BoundsPadding {
            minimum: [0; 3],
            maximum: [0; 3],
            k_one: [1f32.to_bits(); 3],
        }
    }
    fn m(p: [f32; 3]) -> [u32; 16] {
        let mut m = [0; 16];
        m[12..15].copy_from_slice(&p.map(f32::to_bits));
        m
    }
    #[test]
    fn move_root_is_excluded_and_bounds_expand_about_origin() {
        let mut s = state();
        let mut h = Host::default();
        finish_prepared_bounds(
            &mut s,
            &[m([100.; 3]), m([2., -3., 4.]), m([-2., 5., 1.])],
            0,
            &pad(),
            &mut h,
        )
        .unwrap();
        assert_eq!(
            s.minimum,
            [-3.4f32, -4.6000004, 0.20000005].map(f32::to_bits)
        );
        assert_eq!(s.maximum, [3.4f32, 7., 5.8].map(f32::to_bits));
        assert_eq!((s.byte_60, s.byte_61, s.byte_179), (1, 1, 0));
        assert_eq!(h.published.unwrap().byte_61, 8);
    }
    #[test]
    fn absent_initialization_bone_keeps_and_expands_prior_bounds() {
        let mut s = state();
        s.minimum = [2f32.to_bits(); 3];
        s.maximum = [3f32.to_bits(); 3];
        finish_prepared_bounds(&mut s, &[m([100.; 3])], 0, &pad(), &mut Host::default()).unwrap();
        assert_eq!(s.minimum, [1.4000001f32.to_bits(); 3]);
        assert_eq!(s.maximum, [4.6000004f32.to_bits(); 3]);
    }
    #[test]
    fn seed_failure_keeps_new_box_old_sphere_and_flags() {
        let mut s = state();
        let mut h = Host {
            fail_seed: true,
            ..Host::default()
        };
        assert!(finish_prepared_bounds(&mut s, &[m([0.; 3])], -1, &pad(), &mut h).is_err());
        assert_eq!(s.minimum, [(-1f32).to_bits(); 3]);
        assert_eq!(s.sphere, [99; 4]);
        assert_eq!(s.byte_61, 8);
        assert!(h.published.is_none());
    }
    #[test]
    fn publish_failure_keeps_new_sphere_and_old_flags() {
        let mut s = state();
        let mut h = Host {
            fail_publish: true,
            ..Host::default()
        };
        assert!(finish_prepared_bounds(&mut s, &[m([0.; 3])], -1, &pad(), &mut h).is_err());
        assert_eq!(s.sphere[..3], [0; 3]);
        assert!((f32::from_bits(s.sphere[3]) - 3f32.sqrt()).abs() < 1e-6);
        assert_eq!((s.byte_60, s.byte_61, s.byte_179), (7, 8, 9));
    }
    #[test]
    fn zero_extent_masks_distance_after_seed_and_nan_point_is_ignored() {
        let mut s = state();
        let mut p = pad();
        p.k_one = [0; 3];
        let mut h = Host::default();
        finish_prepared_bounds(&mut s, &[m([0.; 3]), m([f32::NAN; 3])], -1, &p, &mut h).unwrap();
        assert_eq!(h.calls, [0.]);
        assert_eq!(s.sphere, [0; 4]);
    }
}
