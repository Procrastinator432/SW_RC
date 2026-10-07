//! Original FAnimRot decode and FQuat.Slerp, with explicit CPU math policy.
use crate::skeletal_track::TrackRotationHost;

/// Post-initialization engine scale: 1 / trunc(32767 / f32(sqrt(0.5))).
pub const ROTATION_SCALE: f32 = 1.0f32 / 46339.0f32;
/// Original core.dll threshold (10186f30), not the common 0.9995 constant.
pub const SLERP_LINEAR_THRESHOLD: f32 = f32::from_bits(0x3f0ccccd);

pub trait QuaternionMath {
    /// Native RSQRTSS seed, before its one Newton correction.
    fn reciprocal_sqrt_seed(&mut self, squared: f32) -> Result<f32, String>;
    /// Native x87 acos/FSIN weights, each stored to f32 before combination.
    fn spherical_weights(&mut self, absolute_dot: f32, alpha: f32) -> Result<[f32; 2], String>;
}

/// Usable portable math policy; not bit-identical emulation of RSQRTSS/x87.
#[derive(Default)]
pub struct PortableQuaternionMath;
impl QuaternionMath for PortableQuaternionMath {
    fn reciprocal_sqrt_seed(&mut self, squared: f32) -> Result<f32, String> {
        Ok(1.0f32 / squared.sqrt())
    }
    fn spherical_weights(&mut self, dot: f32, alpha: f32) -> Result<[f32; 2], String> {
        let angle = (dot as f64).acos();
        let inverse = 1.0 / angle.sin();
        Ok([
            (((1.0 - alpha as f64) * angle).sin() * inverse) as f32,
            ((alpha as f64 * angle).sin() * inverse) as f32,
        ])
    }
}

fn refined_inverse_sqrt(squared: f32, math: &mut impl QuaternionMath) -> Result<f32, String> {
    let seed = math.reciprocal_sqrt_seed(squared)?;
    let product = (seed * squared) * seed;
    let half_seed = seed * 0.5f32;
    Ok((3.0f32 - product) * half_seed)
}

pub fn decode_rotation(key: [u16; 3], math: &mut impl QuaternionMath) -> Result<[u32; 4], String> {
    let [a, b, c] = key.map(|word| ((word & 0xfffe) as i16) as f32 * ROTATION_SCALE);
    let squared = ((1.0f32 - a * a) - b * b) - c * c;
    // Native still executes RSQRTSS/refinement for zero, then clears result bits.
    let reconstructed = refined_inverse_sqrt(squared, math)? * squared;
    let mut d = if squared == 0.0 {
        0.0f32
    } else {
        reconstructed
    };
    if key[2] & 1 != 0 {
        d = 0.0f32 - d;
    }
    let components = match (key[0] & 1) | ((key[1] & 1) << 1) {
        0 => [a, b, c, d],
        1 => [b, c, d, a],
        2 => [c, d, a, b],
        _ => [d, a, b, c],
    };
    Ok(components.map(f32::to_bits))
}

pub fn slerp_rotation(
    current: [u32; 4],
    next: [u32; 4],
    alpha: f32,
    math: &mut impl QuaternionMath,
) -> Result<[u32; 4], String> {
    let a = current.map(f32::from_bits);
    let b = next.map(f32::from_bits);
    let dot = ((a[0] * b[0] + a[2] * b[2]) + a[1] * b[1]) + b[3] * a[3];
    let absolute = if dot < 0.0 { 0.0f32 - dot } else { dot };
    // Native JBE also selects linear for unordered threshold comparison.
    let linear = absolute.partial_cmp(&SLERP_LINEAR_THRESHOLD) != Some(std::cmp::Ordering::Less);
    let [left, mut right] = if linear {
        [1.0f32 - alpha, alpha]
    } else {
        math.spherical_weights(absolute, alpha)?
    };
    if dot < 0.0 {
        right = 0.0f32 - right;
    }
    let mut result = std::array::from_fn::<_, 4, _>(|i| a[i] * left + b[i] * right);
    if linear {
        let squared = ((result[3] * result[3] + result[2] * result[2]) + result[1] * result[1])
            + result[0] * result[0];
        let inverse = refined_inverse_sqrt(squared, math)?;
        for v in &mut result {
            *v *= inverse;
        }
    }
    Ok(result.map(f32::to_bits))
}

/// Connects actual compressed rotations and interpolation to the track sampler.
pub struct QuaternionTrackHost<M: QuaternionMath> {
    pub math: M,
}
impl<M: QuaternionMath> TrackRotationHost for QuaternionTrackHost<M> {
    fn decode(&mut self, compressed: [u16; 3]) -> Result<[u32; 4], String> {
        decode_rotation(compressed, &mut self.math)
    }
    fn slerp(&mut self, current: [u32; 4], next: [u32; 4], alpha: f32) -> Result<[u32; 4], String> {
        slerp_rotation(current, next, alpha, &mut self.math)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Math {
        seeds: Vec<f32>,
        weights: Vec<(f32, f32)>,
        fail: bool,
    }
    impl QuaternionMath for Math {
        fn reciprocal_sqrt_seed(&mut self, n: f32) -> Result<f32, String> {
            self.seeds.push(n);
            if self.fail {
                Err("seed".into())
            } else {
                Ok(1. / n.sqrt())
            }
        }
        fn spherical_weights(&mut self, d: f32, t: f32) -> Result<[f32; 2], String> {
            self.weights.push((d, t));
            if self.fail {
                Err("weights".into())
            } else {
                Ok([0.25, 0.75])
            }
        }
    }
    #[test]
    fn decode_permutations_sign_and_signed_components() {
        let key = [12000, (-8000i16) as u16, 4000];
        let base = decode_rotation(key, &mut PortableQuaternionMath).unwrap();
        for flags in 0..8 {
            let k = [
                key[0] | (flags & 1),
                key[1] | ((flags >> 1) & 1),
                key[2] | ((flags >> 2) & 1),
            ];
            let mut expected = base.map(f32::from_bits);
            if flags & 4 != 0 {
                expected[3] = 0. - expected[3];
            }
            expected.rotate_left((flags & 3) as usize);
            assert_eq!(
                decode_rotation(k, &mut PortableQuaternionMath).unwrap(),
                expected.map(f32::to_bits)
            );
        }
        assert!(f32::from_bits(base[1]) < 0.);
    }
    #[test]
    fn threshold_and_shortest_path() {
        let a = [0., 0., 0., 1.].map(f32::to_bits);
        for d in [0.54f32, SLERP_LINEAR_THRESHOLD, 0.56] {
            let mut m = Math::default();
            slerp_rotation(a, [0., 0., 0., d].map(f32::to_bits), 0.25, &mut m).unwrap();
            assert_eq!(m.weights.len(), usize::from(d < SLERP_LINEAR_THRESHOLD));
            assert_eq!(m.seeds.len(), usize::from(d >= SLERP_LINEAR_THRESHOLD));
        }
        let positive = slerp_rotation(a, a, 0.5, &mut PortableQuaternionMath).unwrap();
        let negative = slerp_rotation(
            a,
            [0., 0., 0., -1.].map(f32::to_bits),
            0.5,
            &mut PortableQuaternionMath,
        )
        .unwrap();
        assert_eq!(positive, negative);
    }
    #[test]
    fn spherical_weights_are_not_normalized_or_clamped() {
        let mut m = Math::default();
        let out = slerp_rotation(
            [1., 0., 0., 0.].map(f32::to_bits),
            [0., 0., 1., 0.].map(f32::to_bits),
            2.5,
            &mut m,
        )
        .unwrap();
        assert_eq!(out, [0.25, 0., 0.75, 0.].map(f32::to_bits));
        assert_eq!(m.weights, vec![(0., 2.5)]);
        assert!(m.seeds.is_empty());
    }
    #[test]
    fn unordered_dot_selects_linear_and_math_errors_propagate() {
        let mut m = Math::default();
        slerp_rotation(
            [f32::NAN, 0., 0., 1.].map(f32::to_bits),
            [0., 0., 0., 1.].map(f32::to_bits),
            0.5,
            &mut m,
        )
        .unwrap();
        assert!(m.weights.is_empty());
        assert!(m.seeds[0].is_nan());
        m.fail = true;
        assert!(decode_rotation([0; 3], &mut m).is_err());
        assert!(slerp_rotation(
            [1., 0., 0., 0.].map(f32::to_bits),
            [0., 0., 1., 0.].map(f32::to_bits),
            0.5,
            &mut m
        )
        .is_err());
    }
    #[test]
    fn portable_orthogonal_midpoint_and_nonunit_linear_normalization() {
        let out = slerp_rotation(
            [1., 0., 0., 0.].map(f32::to_bits),
            [0., 0., 1., 0.].map(f32::to_bits),
            0.5,
            &mut PortableQuaternionMath,
        )
        .unwrap()
        .map(f32::from_bits);
        assert!((out[0] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
        assert_eq!(out[0], out[2]);
        let out = slerp_rotation(
            [0., 0., 0., 2.].map(f32::to_bits),
            [0., 0., 0., 2.].map(f32::to_bits),
            0.5,
            &mut PortableQuaternionMath,
        )
        .unwrap()
        .map(f32::from_bits);
        assert_eq!(out, [0., 0., 0., 1.]);
    }
    #[test]
    fn track_integration_uses_compressed_keys() {
        use crate::{skeletal_root_pose::RootTransform, skeletal_track::*};
        let track = AnimationTrack {
            rotations: vec![[0, 0, 0], [0, 1, 0]],
            rotation_count_word: 2,
            positions: vec![[0; 3]],
            position_count_word: 1,
            position_scale_bits: 1f32.to_bits(),
            durations: vec![2, 2],
            duration_count_word: 2,
        };
        let mut root = RootTransform {
            rotation: [7; 4],
            position: [8; 3],
        };
        sample_track(
            &track,
            1.,
            &mut root,
            &mut QuaternionTrackHost {
                math: PortableQuaternionMath,
            },
        )
        .unwrap();
        let q = root.rotation.map(f32::from_bits);
        assert!((q[1] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
        assert!((q[3] - q[1]).abs() < 1e-6);
        assert_eq!(root.position, [0; 3]);
    }
}
