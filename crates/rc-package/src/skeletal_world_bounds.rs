//! ApplyAnimation actor primitive +48 publication, 1050af1e..1050b276.
use crate::{
    quaternion_animation::QuaternionMath,
    skeletal_bounds::{PoseBounds, PoseBoundsHost},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldPoseBounds {
    pub minimum: [u32; 3],
    pub maximum: [u32; 3],
    pub valid: u8,
    pub sphere: [u32; 4],
}
/// Preserves FBox.operator+= ties/unordered behavior, including replacement by NaN.
pub fn add_world_box_point(state: &mut WorldPoseBounds, p: [u32; 3]) {
    if state.valid == 0 {
        state.minimum = p;
        state.maximum = p;
        state.valid = 1;
        return;
    }
    for (i, &word) in p.iter().enumerate() {
        let point = f32::from_bits(word);
        let low = f32::from_bits(state.minimum[i]);
        let high = f32::from_bits(state.maximum[i]);
        if point
            .partial_cmp(&low)
            .is_none_or(|o| o == std::cmp::Ordering::Less)
        {
            state.minimum[i] = word;
        }
        if high
            .partial_cmp(&point)
            .is_none_or(|o| o == std::cmp::Ordering::Less)
        {
            state.maximum[i] = word;
        }
    }
}
/// Input is the supplied matrix at actor primitive +8. No projective divide.
/// Box is published before the radius seed; failed math leaves the prior sphere.
pub fn publish_world_bounds(
    local: &PoseBounds,
    matrix: [u32; 16],
    state: &mut WorldPoseBounds,
    math: &mut impl QuaternionMath,
) -> Result<(), String> {
    let m = matrix.map(f32::from_bits);
    let low = local.minimum.map(f32::from_bits);
    let high = local.maximum.map(f32::from_bits);
    let mut box_state = WorldPoseBounds {
        minimum: [0; 3],
        maximum: [0; 3],
        valid: 0,
        sphere: state.sphere,
    };
    // Original x/y/z extrema loops, each choosing independently from minimum/maximum.
    for ix in 0..2 {
        for iy in 0..2 {
            for iz in 0..2 {
                let x = if ix == 0 { low[0] } else { high[0] };
                let y = if iy == 0 { low[1] } else { high[1] };
                let z = if iz == 0 { low[2] } else { high[2] };
                let p = [
                    ((m[4] * y + m[8] * z) + x * m[0]) + m[12],
                    ((m[1] * x + m[5] * y) + m[9] * z) + m[13],
                    ((m[2] * x + m[6] * y) + m[10] * z) + m[14],
                ];
                add_world_box_point(&mut box_state, p.map(f32::to_bits));
            }
        }
    }
    state.minimum = box_state.minimum;
    state.maximum = box_state.maximum;
    state.valid = box_state.valid;
    let [x, y, z, radius] = local.sphere.map(f32::from_bits);
    let center = [
        ((m[4] * y + m[0] * x) + z * m[8]) + m[12],
        ((m[9] * z + m[1] * x) + m[5] * y) + m[13],
        ((m[6] * y + m[10] * z) + m[2] * x) + m[14],
    ];
    let squared =
        |row: usize| (m[row + 2] * m[row + 2] + m[row + 1] * m[row + 1]) + m[row] * m[row];
    let mut scale_squared = squared(8);
    for row in [4, 0] {
        let next = squared(row);
        // Original CMP/JC compares unsigned raw float bits, not a floating max.
        if next.to_bits() >= scale_squared.to_bits() {
            scale_squared = next;
        }
    }
    let seed = math.reciprocal_sqrt_seed(scale_squared)?;
    let scale = ((3.0f32 - (seed * scale_squared) * seed) * (seed * 0.5f32)) * scale_squared;
    let scale = if scale_squared == 0.0 { 0.0 } else { scale };
    state.sphere = [
        center[0].to_bits(),
        center[1].to_bits(),
        center[2].to_bits(),
        (radius * scale).to_bits(),
    ];
    Ok(())
}

/// Connects local bounds completion to optional actor world publication.
pub struct WorldPoseBoundsHost<'a, M> {
    pub math: M,
    /// None models a null actor primitive; Some supplies its matrix and output fields.
    pub actor: Option<([u32; 16], &'a mut WorldPoseBounds)>,
}
impl<M: QuaternionMath> PoseBoundsHost for WorldPoseBoundsHost<'_, M> {
    fn reciprocal_sqrt_seed(&mut self, n: f32) -> Result<f32, String> {
        self.math.reciprocal_sqrt_seed(n)
    }
    fn publish(&mut self, b: &PoseBounds) -> Result<(), String> {
        if let Some((matrix, state)) = &mut self.actor {
            publish_world_bounds(b, *matrix, state, &mut self.math)?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::quaternion_animation::PortableQuaternionMath;
    fn local() -> PoseBounds {
        PoseBounds {
            minimum: [-1f32, -2., -3.].map(f32::to_bits),
            maximum: [1f32, 2., 3.].map(f32::to_bits),
            sphere: [0, 0, 0, 4f32.to_bits()],
            byte_60: 7,
            byte_61: 8,
            byte_179: 9,
        }
    }
    fn state() -> WorldPoseBounds {
        WorldPoseBounds {
            minimum: [99; 3],
            maximum: [99; 3],
            valid: 0,
            sphere: [88; 4],
        }
    }
    fn identity() -> [u32; 16] {
        let mut m = [0; 16];
        for i in [0, 5, 10, 15] {
            m[i] = 1f32.to_bits();
        }
        m
    }
    #[test]
    fn reflection_nonuniform_scale_and_translation() {
        let mut m = identity();
        m[0] = (-2f32).to_bits();
        m[5] = 3f32.to_bits();
        m[10] = 0.5f32.to_bits();
        m[12] = 10f32.to_bits();
        let mut s = state();
        publish_world_bounds(&local(), m, &mut s, &mut PortableQuaternionMath).unwrap();
        assert_eq!(s.minimum, [8f32, -6., -1.5].map(f32::to_bits));
        assert_eq!(s.maximum, [12f32, 6., 1.5].map(f32::to_bits));
        assert_eq!(s.valid, 1);
        assert_eq!(s.sphere, [10f32, 0., 0., 12.].map(f32::to_bits));
    }
    #[test]
    fn sphere_center_and_corner_transform_keep_different_sum_orders() {
        let mut b = local();
        b.minimum = [1f32.to_bits(); 3];
        b.maximum = b.minimum;
        b.sphere = [1f32.to_bits(); 4];
        let mut m = [0; 16];
        m[0] = 1e20f32.to_bits();
        m[4] = (-1e20f32).to_bits();
        m[8] = 1f32.to_bits();
        let mut s = state();
        publish_world_bounds(&b, m, &mut s, &mut PortableQuaternionMath).unwrap();
        assert_eq!(s.minimum[0], 0);
        assert_eq!(s.sphere[0], 1f32.to_bits());
    }
    struct Math {
        seen: Vec<f32>,
        fail: bool,
    }
    impl QuaternionMath for Math {
        fn reciprocal_sqrt_seed(&mut self, n: f32) -> Result<f32, String> {
            self.seen.push(n);
            if self.fail {
                Err("seed".into())
            } else {
                Ok(0.125)
            }
        }
        fn spherical_weights(&mut self, _: f32, _: f32) -> Result<[f32; 2], String> {
            unreachable!()
        }
    }
    #[test]
    fn seed_failure_keeps_new_box_old_sphere() {
        let mut s = state();
        let mut math = Math {
            seen: vec![],
            fail: true,
        };
        assert!(publish_world_bounds(&local(), identity(), &mut s, &mut math).is_err());
        assert_eq!(s.minimum, local().minimum);
        assert_eq!(s.sphere, [88; 4]);
        assert_eq!(math.seen, [1.]);
    }
    #[test]
    fn zero_scale_seed_is_called_before_radius_mask() {
        let mut s = state();
        let mut math = Math {
            seen: vec![],
            fail: false,
        };
        publish_world_bounds(&local(), [0; 16], &mut s, &mut math).unwrap();
        assert_eq!(math.seen, [0.]);
        assert_eq!(s.sphere, [0; 4]);
    }
    #[test]
    fn box_nan_replaces_both_endpoints_ties_keep_existing_bits() {
        let mut s = state();
        add_world_box_point(&mut s, [(-0f32).to_bits(); 3]);
        add_world_box_point(&mut s, [0; 3]);
        assert_eq!(s.minimum, [(-0f32).to_bits(); 3]);
        let nan = 0x7fc00123;
        add_world_box_point(&mut s, [nan; 3]);
        assert_eq!(s.minimum, [nan; 3]);
        assert_eq!(s.maximum, [nan; 3]);
        add_world_box_point(&mut s, [1f32.to_bits(); 3]);
        assert_eq!(s.minimum, [1f32.to_bits(); 3]);
    }
    #[test]
    fn local_completion_waits_for_world_publication_and_flags() {
        use crate::skeletal_bounds::{finish_prepared_bounds, BoundsPadding};
        let mut b = local();
        let mut s = state();
        let mut m = identity();
        m[12] = 2f32.to_bits();
        let p = BoundsPadding {
            minimum: [0; 3],
            maximum: [0; 3],
            k_one: [1f32.to_bits(); 3],
        };
        let mut host = WorldPoseBoundsHost {
            math: PortableQuaternionMath,
            actor: Some((m, &mut s)),
        };
        finish_prepared_bounds(&mut b, &[identity()], -1, &p, &mut host).unwrap();
        assert_eq!((b.byte_60, b.byte_61, b.byte_179), (1, 1, 0));
        assert_eq!(s.minimum[0], 1f32.to_bits());
    }
}
