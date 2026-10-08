//! Native fog and object-space eye constants from supplied scene snapshots.
use super::Matrix;
pub type SqrtSeed = fn(f32) -> Result<f32, String>;
#[derive(Clone, Copy, Debug, Default)]
pub struct Scene {
    /// Actor selected through GCubemapManager +2c, then +1b4/+1b8/+1bc.
    /// None selects lengths from the ObjectToWorld matrix instead.
    pub actor_draw_scale: Option<[u32; 3]>,
    /// Render state +2f0/+2f4. IEEE division, including zero ranges, is retained.
    pub fog: [u32; 2],
    /// Editor viewport actor +138/+13c/+140, distinct from CameraToWorld.
    pub editor_eye: [u32; 3],
    /// Runtime camera +194/+198/+19c; None represents a null camera pointer.
    pub runtime_eye: Option<[u32; 3]>,
    /// Explicit CPU math boundary; absence never silently chooses a portable seed.
    pub reciprocal_sqrt_seed: Option<SqrtSeed>,
}
/// Usable portable seed, explicitly not an emulation of native RSQRTSS.
pub fn portable_seed(squared: f32) -> Result<f32, String> {
    Ok(1.0 / squared.sqrt())
}
/// Case 30; actor raw scale has W=0, matrix-derived row lengths have W=1.
pub fn draw_scale(scene: Scene, object: Matrix) -> Result<[u32; 4], String> {
    if let Some([x, y, z]) = scene.actor_draw_scale {
        return Ok([x, y, z, 0]);
    }
    let seed = scene
        .reciprocal_sqrt_seed
        .ok_or("Unresolved DrawScale3D RSQRTSS seed policy")?;
    let mut result = [0, 0, 0, 1f32.to_bits()];
    for row in 0..3 {
        let v = [object[row][0], object[row][1], object[row][2]].map(f32::from_bits);
        let squares = v.map(|x| x * x);
        let q = if row == 0 {
            (squares[1] + squares[2]) + squares[0]
        } else {
            (squares[0] + squares[1]) + squares[2]
        };
        if v.iter().any(|x| !x.is_finite()) || !q.is_finite() {
            return Err("Nonfinite DrawScale3D squared length excluded by host contract".into());
        }
        let s = seed(q)?;
        let product = (s * q) * s;
        let length = ((3.0f32 - product) * (s * 0.5f32)) * q;
        let length = if q == 0.0 { 0.0 } else { length };
        if !length.is_finite() {
            return Err("Nonfinite DrawScale3D length excluded by host contract".into());
        }
        result[row] = length.to_bits();
    }
    Ok(result)
}
pub fn fog([start, end]: [u32; 2]) -> [u32; 4] {
    let reciprocal = 1.0f32 / (f32::from_bits(end) - f32::from_bits(start));
    let scaled = f32::from_bits(end) * reciprocal;
    [start, end, scaled.to_bits(), reciprocal.to_bits()]
}
pub fn eye_position(eye: [u32; 3], inverse: Matrix, editor: bool) -> Result<[u32; 4], String> {
    let eye = eye.map(f32::from_bits);
    let order = if editor { [0, 2, 1] } else { [2, 1, 0] };
    let mut output = [0, 0, 0, 1f32.to_bits()];
    for col in 0..3 {
        let terms = order.map(|k| f32::from_bits(inverse[k][col]) * eye[k]);
        let translation = f32::from_bits(inverse[3][col]);
        let first = terms[0] + terms[1];
        let second = first + terms[2];
        let result = second + translation;
        if eye
            .iter()
            .chain(&terms)
            .chain(&[translation, first, second, result])
            .any(|v| !v.is_finite())
        {
            return Err("Nonfinite object-space eye arithmetic excluded by host contract".into());
        }
        output[col] = result.to_bits();
    }
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actor_draw_scale_is_raw_and_does_not_need_math_or_finite_matrix() {
        let scene = Scene {
            actor_draw_scale: Some([0x7fc12345, 0x80000000, 7]),
            ..Default::default()
        };
        assert_eq!(
            draw_scale(scene, [[f32::NAN.to_bits(); 4]; 4]).unwrap(),
            [0x7fc12345, 0x80000000, 7, 0]
        );
    }
    #[test]
    fn matrix_draw_scale_uses_row_lengths_zero_mask_and_homogeneous_one() {
        let object = [
            [3., 4., 0., f32::NAN],
            [0., 0., 0., f32::NAN],
            [0., 0., -2., f32::NAN],
            [f32::NAN; 4],
        ]
        .map(|r| r.map(f32::to_bits));
        assert!(draw_scale(Scene::default(), object).is_err());
        let scene = Scene {
            reciprocal_sqrt_seed: Some(portable_seed),
            ..Default::default()
        };
        assert_eq!(
            draw_scale(scene, object).unwrap(),
            [5., 0., 2., 1.].map(f32::to_bits)
        );
        let mut invalid = object;
        invalid[1][0] = f32::INFINITY.to_bits();
        assert!(draw_scale(scene, invalid).is_err());
    }
    #[test]
    fn eye_orders_differ_and_fourth_column_is_unused() {
        let inverse = [
            [1e20f32, 0., 0., f32::NAN],
            [-1e20, 0., 0., f32::NAN],
            [1., 0., 0., f32::NAN],
            [0., 0., 0., f32::NAN],
        ]
        .map(|r| r.map(f32::to_bits));
        let eye = [1f32.to_bits(); 3];
        assert_eq!(eye_position(eye, inverse, true).unwrap()[0], 0);
        assert_eq!(eye_position(eye, inverse, false).unwrap()[0], 0);
        let mut inverse = inverse;
        inverse[0][0] = 1f32.to_bits();
        inverse[2][0] = 1e20f32.to_bits();
        assert_eq!(eye_position(eye, inverse, true).unwrap()[0], 0);
        assert_eq!(
            eye_position(eye, inverse, false).unwrap()[0],
            1f32.to_bits()
        );
        assert!(eye_position([f32::NAN.to_bits(); 3], inverse, false).is_err());
    }
    #[test]
    fn fog_keeps_ieee_zero_reverse_and_raw_words() {
        assert_eq!(
            fog([0x80000000, 2f32.to_bits()]),
            [0x80000000, 2f32.to_bits(), 1f32.to_bits(), 0.5f32.to_bits()]
        );
        assert_eq!(
            fog([2f32.to_bits(), 0]),
            [2f32.to_bits(), 0, 0x80000000, (-0.5f32).to_bits()]
        );
        let zero = fog([0; 2]);
        assert!(f32::from_bits(zero[2]).is_nan());
        assert_eq!(zero[3], f32::INFINITY.to_bits());
    }
}
