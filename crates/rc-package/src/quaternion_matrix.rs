//! FQuat::FQuat(FMatrix), core 10144cb0. Preserves native formulas, including
//! the nonstandard nonpositive-trace branches; no normalization or scale removal.
use crate::quaternion_animation::QuaternionMath;

/// Native scalar SSE operations with the RSQRTSS seed supplied by the math policy.
/// All four components are returned together; a seed error produces no quaternion.
pub fn quaternion_from_matrix(
    input: [u32; 16],
    math: &mut impl QuaternionMath,
) -> Result<[u32; 4], String> {
    let m = input.map(f32::from_bits);
    let trace = ((m[5] + m[0]) + m[10]) + 1.0f32;
    let branch = if trace > 0.0 {
        3
    } else if m[0] > m[5] && m[0] > m[10] {
        0
    } else if m[5] > m[10] {
        1
    } else {
        2
    };
    let squared = match branch {
        3 => trace,
        0 => ((m[0] + 1.0f32) - m[5]) - m[10],
        1 => ((m[5] + 1.0f32) - m[0]) - m[10],
        _ => ((m[10] + 1.0f32) - m[0]) - m[5],
    };
    let seed = math.reciprocal_sqrt_seed(squared)?;
    let product = (seed * squared) * seed;
    let refined = (3.0f32 - product) * (seed * 0.5f32);
    let root = refined * squared;
    // CMPNEQSS/ANDPS clears exactly zero only, after performing the arithmetic.
    let root = if squared == 0.0 { 0.0f32 } else { root };
    let factor = 0.5f32 / root;
    let q = match branch {
        3 => [
            (m[6] - m[9]) * factor,
            (m[8] - m[2]) * factor,
            (m[1] - m[4]) * factor,
            0.25f32 / factor,
        ],
        0 => [
            factor * 0.5f32,
            (m[4] + m[1]) * factor,
            (m[8] + m[2]) * factor,
            (m[9] + m[6]) * factor,
        ],
        1 => [
            (m[4] + m[1]) * factor,
            factor * 0.5f32,
            (m[9] + m[6]) * factor,
            (m[8] + m[2]) * factor,
        ],
        _ => [
            (m[8] + m[2]) * factor,
            (m[9] + m[6]) * factor,
            factor * 0.5f32,
            (m[4] + m[1]) * factor,
        ],
    };
    Ok(q.map(f32::to_bits))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quaternion_animation::PortableQuaternionMath;
    #[derive(Default)]
    struct Seed {
        seen: Vec<f32>,
        fail: bool,
    }
    impl QuaternionMath for Seed {
        fn reciprocal_sqrt_seed(&mut self, squared: f32) -> Result<f32, String> {
            self.seen.push(squared);
            if self.fail {
                Err("seed boundary".into())
            } else {
                Ok(0.125)
            }
        }
        fn spherical_weights(&mut self, _: f32, _: f32) -> Result<[f32; 2], String> {
            panic!("matrix conversion does not use spherical weights")
        }
    }
    fn diagonal(d: [f32; 3]) -> [u32; 16] {
        let mut m = [0; 16];
        for (i, v) in [0, 5, 10].into_iter().zip(d) {
            m[i] = v.to_bits();
        }
        m[15] = 1f32.to_bits();
        m
    }
    #[test]
    fn identity_and_translation_do_not_change_rotation() {
        let mut m = diagonal([1.; 3]);
        let q = quaternion_from_matrix(m, &mut PortableQuaternionMath).unwrap();
        assert_eq!(q, [0, 0, 0, 1f32.to_bits()]);
        for i in [3, 7, 11, 12, 13, 14, 15] {
            m[i] = f32::NAN.to_bits();
        }
        assert_eq!(
            quaternion_from_matrix(m, &mut PortableQuaternionMath).unwrap(),
            q
        );
    }
    #[test]
    fn half_turns_keep_native_nonstandard_dominant_component() {
        for (axis, d) in [[1., -1., -1.], [-1., 1., -1.], [-1., -1., 1.]]
            .into_iter()
            .enumerate()
        {
            let q = quaternion_from_matrix(diagonal(d), &mut PortableQuaternionMath).unwrap();
            let mut expected = [0; 4];
            expected[axis] = 0.125f32.to_bits();
            assert_eq!(q, expected);
        }
    }
    #[test]
    fn diagonal_ties_fall_through_to_z_then_y() {
        let mut math = Seed::default();
        let q = quaternion_from_matrix(diagonal([-1.; 3]), &mut math).unwrap();
        assert_eq!(math.seen, vec![2.]);
        assert!(q[2] != 0 && q[0] == 0 && q[1] == 0);
        let q = quaternion_from_matrix(diagonal([0., 0., -2.]), &mut math).unwrap();
        assert_eq!(math.seen, vec![2., 3.]);
        assert!(q[1] != 0 && q[0] == 0 && q[2] == 0);
    }
    #[test]
    fn dominant_branch_uses_symmetric_sum_for_w() {
        let mut m = diagonal([1., -1., -1.]);
        m[6] = 2f32.to_bits();
        m[9] = 3f32.to_bits();
        let q = quaternion_from_matrix(m, &mut PortableQuaternionMath)
            .unwrap()
            .map(f32::from_bits);
        assert_eq!(q, [0.125, 0., 0., 1.25]);
    }
    #[test]
    fn unordered_trace_follows_native_ordered_diagonal_tests() {
        let mut math = Seed::default();
        let q = quaternion_from_matrix(diagonal([f32::NAN, -1., -1.]), &mut math).unwrap();
        assert!(math.seen[0].is_nan());
        assert!(q.iter().all(|&v| f32::from_bits(v).is_nan()));
    }
    #[test]
    fn math_failure_propagates_after_one_seed_request() {
        let mut math = Seed {
            fail: true,
            ..Default::default()
        };
        assert_eq!(
            quaternion_from_matrix(diagonal([1.; 3]), &mut math),
            Err("seed boundary".into())
        );
        assert_eq!(math.seen, vec![4.]);
    }
}
