//! Bounded native D3DDrv light slot, color, ambient and spotlight constants.
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Light {
    /// Native source +30/+34, preserved as raw words.
    pub radius: [u32; 2],
    /// Actor byte +2a. Native 0x13 selects a distant directional-light point.
    pub kind: u8,
    pub position: [u32; 3],
    /// Source pointer exists, but its first actor pointer can still be null.
    pub actor_present: bool,
    pub cone: u8,
    pub color: [u32; 4],
    pub brightness: u32,
    pub direction: [u32; 3],
    pub flags: [u32; 2],
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Lighting {
    pub slots: [Option<Light>; 4],
    /// GCubemapManager actor exists and its flags & 0x2000 == 0.
    pub alpha_gate: bool,
    /// Render state +148: bytes B,G,R,A. The alpha byte is ignored.
    pub ambient_bgra: u32,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub struct LightCache {
    pub count: usize,
    pub indices: [i32; 4],
}
/// Native cases 16/19/22/25, with an explicit RSQRTSS seed policy.
pub fn inverse_radius(
    light: Option<Light>,
    object: [[u32; 4]; 4],
    seed: Option<crate::shader_constants::scene::SqrtSeed>,
) -> Result<[u32; 4], String> {
    let Some(light) = light else {
        return Ok(missing_position());
    };
    let [outer, inner] = light.radius.map(f32::from_bits);
    let delta = outer - inner;
    if !outer.is_finite() || !inner.is_finite() || !delta.is_finite() {
        return Err("Nonfinite light radius excluded by host contract".into());
    }
    if light.kind == 0x13 {
        return Ok([0x322bcc77, 0x322bcc77, delta.to_bits(), 1f32.to_bits()]);
    }
    let row = [object[0][0], object[0][1], object[0][2]].map(f32::from_bits);
    let squared = (row[0] * row[0] + row[1] * row[1]) + row[2] * row[2];
    if row.iter().any(|v| !v.is_finite()) || !squared.is_finite() || delta == 0.0 {
        return Err("Invalid light radius scale excluded by host contract".into());
    }
    let seed = seed.ok_or("Unresolved light radius RSQRTSS seed policy")?(squared)?;
    let product = (seed * squared) * seed;
    let refined = ((3.0f32 - product) * (seed * 0.5f32)) * squared;
    // Original executes refinement for zero, then clears its bits with ANDPS.
    let length = if squared == 0.0 { 0.0 } else { refined };
    let value = length / delta;
    if !value.is_finite() {
        return Err("Nonfinite light radius result excluded by host contract".into());
    }
    Ok([
        value.to_bits(),
        value.to_bits(),
        light.radius[1],
        1f32.to_bits(),
    ])
}
impl LightCache {
    /// Native two passes count cone!=0 then cone==0, writing at SOURCE index.
    /// This is intentionally not a packed/sorted light list.
    pub fn new(lighting: Lighting) -> Self {
        let mut cache = Self {
            count: 0,
            indices: [-1; 4],
        };
        for spot in [true, false] {
            for (index, source) in lighting.slots.iter().enumerate() {
                if let Some(light) = source {
                    if light.actor_present && (light.cone != 0) == spot {
                        cache.indices[index] = index as i32;
                        cache.count += 1;
                    }
                }
            }
        }
        cache
    }
    pub fn select(self, lighting: Lighting, index: usize) -> Result<Option<Light>, String> {
        if index >= 4 {
            return Err("Light index exceeds native four slots".into());
        }
        if index >= self.count {
            return Ok(None);
        }
        let source = usize::try_from(self.indices[index])
            .map_err(|_| "Native sparse light table would address slot -1".to_owned())?;
        lighting
            .slots
            .get(source)
            .copied()
            .flatten()
            .filter(|light| light.actor_present)
            .map(Some)
            .ok_or_else(|| "Native cached light source no longer valid".into())
    }
}
pub fn color(light: Option<Light>, alpha_gate: bool) -> Result<[u32; 4], String> {
    let Some(light) = light else {
        return Ok([0; 4]);
    };
    let brightness = f32::from_bits(light.brightness);
    if !brightness.is_finite() {
        return Err("Nonfinite light brightness excluded by host contract".into());
    }
    let mut output = [0; 4];
    for (i, word) in output[..3].iter_mut().enumerate() {
        let component = f32::from_bits(light.color[i]);
        let doubled = component * 2.;
        let scaled = doubled * brightness;
        if !component.is_finite() || !doubled.is_finite() || !scaled.is_finite() {
            return Err("Nonfinite light color arithmetic excluded by host contract".into());
        }
        *word = scaled.to_bits();
    }
    output[3] = if !alpha_gate && light.flags == [0; 2] {
        1f32.to_bits()
    } else {
        0
    };
    Ok(output)
}
/// World point used by the position branch; directional point is not normalized.
pub fn position_point(light: Light, object: [[u32; 4]; 4]) -> Result<[f32; 3], String> {
    let point = if light.kind == 0x13 {
        std::array::from_fn(|i| {
            let offset = f32::from_bits(light.direction[i]) * 65365f32;
            f32::from_bits(object[3][i]) - offset
        })
    } else {
        light.position.map(f32::from_bits)
    };
    if point.iter().any(|v| !v.is_finite()) {
        return Err("Nonfinite light position point excluded by host contract".into());
    }
    Ok(point)
}
/// Original case-specific MULSS/ADDSS order, no perspective divide, W forced to one.
pub fn transform_position(
    point: [f32; 3],
    inverse: [[u32; 4]; 4],
    directional: bool,
) -> Result<[u32; 4], String> {
    let m = inverse.map(|row| row.map(f32::from_bits));
    let mut result = [0; 4];
    for col in 0..3 {
        let order = if col == 0 && !directional {
            [2, 1, 0]
        } else {
            [0, 2, 1]
        };
        let terms = order.map(|k| point[k] * m[k][col]);
        let first = terms[0] + terms[1];
        let second = first + terms[2];
        let out = second + m[3][col];
        if terms.iter().any(|v| !v.is_finite())
            || !first.is_finite()
            || !second.is_finite()
            || !out.is_finite()
        {
            return Err("Nonfinite light position transform excluded by host contract".into());
        }
        result[col] = out.to_bits();
    }
    result[3] = 1f32.to_bits();
    Ok(result)
}
pub fn missing_position() -> [u32; 4] {
    [
        10000000f32.to_bits(),
        10000000f32.to_bits(),
        10000000f32.to_bits(),
        0,
    ]
}
pub fn ambient(bgra: u32) -> [u32; 4] {
    let factor = f32::from_bits(0x3c008081); // Native 10072684, 2/255 as stored f32.
    let rgb = [
        ((bgra >> 16) & 255) as f32,
        ((bgra >> 8) & 255) as f32,
        (bgra & 255) as f32,
    ]
    .map(|v| (v * factor).to_bits());
    [rgb[0], rgb[1], rgb[2], 2f32.to_bits()]
}
pub fn spotlight_direction(light: Option<Light>) -> [u32; 4] {
    match light.filter(|light| light.cone != 0) {
        Some(light) => [
            light.direction[0],
            light.direction[1],
            light.direction[2],
            1f32.to_bits(),
        ],
        None => [1f32.to_bits(), 0, 0, 1f32.to_bits()],
    }
}
pub fn spotlight_cone(light: Option<Light>) -> [u32; 4] {
    let Some(light) = light.filter(|light| light.cone != 0) else {
        return [0, 0, 0, 1f32.to_bits()];
    };
    let scaled = light.cone as f32 * f32::from_bits(0x3a800142);
    let complement = 1.0f32 - scaled;
    let square = complement * complement;
    let x = square * f32::from_bits(0x3f89999a);
    let y = x * f32::from_bits(0x3dcccccd);
    [x, y, 1. / y, 1.].map(f32::to_bits)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn positional_and_directional_points_transform_with_translation_and_scale() {
        let object = [
            [2., 0., 0., 0.],
            [0., 4., 0., 0.],
            [0., 0., 8., 0.],
            [5., 6., 7., 1.],
        ]
        .map(|r| r.map(f32::to_bits));
        let inverse = [
            [0.5, 0., 0., 0.],
            [0., 0.25, 0., 0.],
            [0., 0., 0.125, 0.],
            [-2.5, -1.5, -0.875, 1.],
        ]
        .map(|r| r.map(f32::to_bits));
        let mut l = light(0);
        l.position = [9., 14., 23.].map(f32::to_bits);
        assert_eq!(
            transform_position(position_point(l, object).unwrap(), inverse, false).unwrap(),
            [2., 2., 2., 1.].map(f32::to_bits)
        );
        l.kind = 0x13;
        l.direction = [1., 2., 3.].map(f32::to_bits);
        assert_eq!(
            transform_position(position_point(l, object).unwrap(), inverse, true).unwrap(),
            [-32682.5, -32682.5, -24511.875, 1.].map(f32::to_bits)
        );
    }
    #[test]
    fn position_orders_differ_and_fourth_inverse_column_is_ignored() {
        let inverse = [
            [16777216., 0., 0., f32::NAN],
            [-16777216., 0., 0., f32::NAN],
            [1., 0., 0., f32::NAN],
            [0., 0., 0., f32::NAN],
        ]
        .map(|r| r.map(f32::to_bits));
        assert_eq!(
            transform_position([1.; 3], inverse, false).unwrap()[0],
            1f32.to_bits()
        );
        assert_eq!(transform_position([1.; 3], inverse, true).unwrap()[0], 0);
        assert_eq!(missing_position(), [0x4b189680, 0x4b189680, 0x4b189680, 0]);
        assert!(transform_position([f32::MAX; 3], inverse, false).is_err());
    }
    fn light(cone: u8) -> Light {
        Light {
            radius: [0; 2],
            kind: 0,
            position: [0; 3],
            actor_present: true,
            cone,
            color: [0.25f32.to_bits(); 4],
            brightness: 3f32.to_bits(),
            direction: [0x7fc12345, 0x80000000, 7],
            flags: [0; 2],
        }
    }
    #[test]
    fn inverse_radius_has_explicit_math_boundary_directional_and_zero_scale() {
        let mut light = light(0);
        light.radius = [4f32.to_bits(), 2f32.to_bits()];
        let object = [[0; 4]; 4];
        assert_eq!(
            inverse_radius(None, object, None).unwrap(),
            missing_position()
        );
        assert!(inverse_radius(Some(light), object, None).is_err());
        let zero = inverse_radius(
            Some(light),
            object,
            Some(crate::shader_constants::scene::portable_seed),
        )
        .unwrap();
        assert_eq!(zero, [0, 0, 2f32.to_bits(), 1f32.to_bits()]);
        light.kind = 19;
        assert_eq!(
            inverse_radius(Some(light), object, None).unwrap(),
            [0x322bcc77, 0x322bcc77, 2f32.to_bits(), 1f32.to_bits()]
        );
        light.kind = 7;
        light.radius = [0; 2];
        assert!(inverse_radius(
            Some(light),
            object,
            Some(crate::shader_constants::scene::portable_seed)
        )
        .is_err());
    }
    #[test]
    fn source_slots_are_not_packed_or_prioritized_and_holes_are_explicit_errors() {
        let lighting = Lighting {
            slots: [None, Some(light(0)), Some(light(12)), None],
            ..Lighting::default()
        };
        let cache = LightCache::new(lighting);
        assert_eq!(cache.count, 2);
        assert_eq!(cache.indices, [-1, 1, 2, -1]);
        assert!(cache.select(lighting, 0).is_err());
        assert_eq!(cache.select(lighting, 1).unwrap().unwrap().cone, 0);
        assert!(cache.select(lighting, 2).unwrap().is_none());
        let mut invalid = light(2);
        invalid.actor_present = false;
        assert_eq!(
            LightCache::new(Lighting {
                slots: [Some(invalid); 4],
                ..lighting
            })
            .count,
            0
        );
    }
    #[test]
    fn color_scales_twice_without_clamping_and_overwrites_alpha() {
        let mut light = light(0);
        light.color[3] = 0x7fc12345;
        assert_eq!(
            color(Some(light), false).unwrap(),
            [
                1.5f32.to_bits(),
                1.5f32.to_bits(),
                1.5f32.to_bits(),
                1f32.to_bits()
            ]
        );
        assert_eq!(color(Some(light), true).unwrap()[3], 0);
        light.flags = [0, 0x80000000];
        assert_eq!(color(Some(light), false).unwrap()[3], 0);
        assert_eq!(color(None, true).unwrap(), [0; 4]);
        light.color[0] = f32::MAX.to_bits();
        assert!(color(Some(light), false).is_err());
    }
    #[test]
    fn ambient_uses_bgr_bytes_and_forces_two_in_w() {
        assert_eq!(
            ambient(0x12ff0080),
            [
                2f32.to_bits(),
                0,
                (128.0f32 * f32::from_bits(0x3c008081)).to_bits(),
                2f32.to_bits()
            ]
        );
        assert_eq!(ambient(0x12ff0080), ambient(0xffff0080));
    }
    #[test]
    fn spotlight_copies_raw_direction_and_does_not_use_trigonometry_for_cone() {
        let l = light(255);
        assert_eq!(
            spotlight_direction(Some(l)),
            [0x7fc12345, 0x80000000, 7, 1f32.to_bits()]
        );
        assert_eq!(spotlight_cone(None), [0, 0, 0, 1f32.to_bits()]);
        assert_eq!(
            spotlight_direction(Some(light(0))),
            spotlight_direction(None)
        );
        let result = spotlight_cone(Some(l)).map(f32::from_bits);
        assert!(result[0] > 0.6 && result[0] < 0.61);
        assert_eq!(result[2], 1. / result[1]);
    }
}
