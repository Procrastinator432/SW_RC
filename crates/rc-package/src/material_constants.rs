//! D3DDrv scalar constants: wrapped time, bounded portable waves/circle, shared flicker.
use serde::Serialize;
pub fn shader_time(engine_time: f32) -> Result<[f32; 4], String> {
    if !engine_time.is_finite() {
        return Err("Nonfinite engine time".into());
    }
    Ok([engine_time % 120.; 4])
}
/// Zero frequency means one in the original. Portable f64 cosine is diagnostic;
/// original FCOS precision is only asserted on independently checked probe inputs.
pub fn cos_time(engine_time: f32, frequency: f32) -> Result<[f32; 4], String> {
    if !frequency.is_finite() {
        return Err("Nonfinite cosine frequency".into());
    }
    let time = shader_time(engine_time)?[0];
    let frequency = if frequency == 0. { 1. } else { frequency };
    let argument = time as f64 * frequency as f64;
    if argument.abs() > 8192. {
        return Err("CosTime argument outside diagnostic range".into());
    }
    Ok([argument.cos() as f32; 4])
}
fn wave_argument(engine_time: f32, frequency: f32) -> Result<f64, String> {
    if !frequency.is_finite() {
        return Err("Nonfinite wave frequency".into());
    }
    let time = shader_time(engine_time)?[0];
    let frequency = if frequency == 0.0 { 1.0 } else { frequency };
    // A product of two f32 values fits exactly in f64, matching the x87 FMUL
    // argument before the transcendental instruction for these bounded inputs.
    let argument = time as f64 * frequency as f64;
    if argument.abs() > 8192.0 {
        return Err("Wave argument outside diagnostic range".into());
    }
    Ok(argument)
}
/// Case 10; portable sine, native FSIN parity only asserted for checked probes.
pub fn sin_time(engine_time: f32, frequency: f32) -> Result<[f32; 4], String> {
    Ok([wave_argument(engine_time, frequency)?.sin() as f32; 4])
}
/// Case 11; FPTAN's extra stack value 1 is discarded, leaving tangent.
pub fn tan_time(engine_time: f32, frequency: f32) -> Result<[f32; 4], String> {
    let value = wave_argument(engine_time, frequency)?.tan() as f32;
    if !value.is_finite() {
        return Err("Nonfinite tangent result excluded by host contract".into());
    }
    Ok([value; 4])
}
/// Case 13; fixed radius 500 from original D3DDrv address 10072688.
/// Scale before the f32 store; no supplied frequency or material plane is read.
pub fn xy_circle(engine_time: f32) -> Result<[f32; 4], String> {
    let time = shader_time(engine_time)?[0] as f64;
    Ok([
        (time.cos() * 500.0) as f32,
        (time.sin() * 500.0) as f32,
        0.0,
        1.0,
    ])
}
/// Case 34: a TWO-register rotation, with an f32 product before FCOS/FSIN.
/// Unlike the wave cases, a zero supplied rate stays zero.
pub fn rotator(engine_time: f32, frequency: f32) -> Result<[[u32; 4]; 2], String> {
    let time = shader_time(engine_time)?[0];
    let argument = time * frequency;
    if !frequency.is_finite() || !argument.is_finite() || argument.abs() > 8192.0 {
        return Err("Rotator argument outside finite diagnostic range".into());
    }
    let cosine = (argument as f64).cos() as f32;
    let sine = (argument as f64).sin();
    Ok([
        [cosine.to_bits(), (-sine as f32).to_bits(), 0, 0],
        [(sine as f32).to_bits(), cosine.to_bits(), 0, 0],
    ])
}
#[derive(Clone, Debug, Serialize)]
pub struct Flicker {
    last_time: Option<f32>,
    /// Native globals A/B/C at 100801a0/1008019c/10080198.
    samples: [f32; 3],
}
impl Default for Flicker {
    fn default() -> Self {
        Self {
            last_time: None,
            samples: [0.5; 3],
        }
    }
}
impl Flicker {
    /// Share ONE instance across all shader constants/materials, like the original DLL.
    /// Callback errors preserve consumed samples and the updated time; native rand cannot fail.
    pub fn evaluate(
        &mut self,
        engine_time: f32,
        plane: [f32; 4],
        random: &mut dyn FnMut() -> Result<u16, String>,
    ) -> Result<[f32; 4], String> {
        if !engine_time.is_finite() || !plane[0].is_finite() || !plane[1].is_finite() {
            return Err("Nonfinite flicker input".into());
        }
        let threshold = plane[0];
        let amplitude = plane[1];
        let base = 1. - amplitude;
        if self.last_time.is_none() {
            self.last_time = Some(engine_time);
        }
        if self.last_time != Some(engine_time) {
            self.last_time = Some(engine_time);
            for value in &mut self.samples {
                let r = random()?;
                if r > 32767 {
                    return Err("Flicker host rand outside CRT15 range".into());
                }
                *value = r as f32 * f32::from_bits(0x38000100);
            }
        }
        let [a, b, c] = self.samples;
        let evaluate = |test: f32, other: f32| {
            if test <= threshold {
                1.
            } else {
                let product = amplitude * other;
                product + base
            }
        };
        let result = [evaluate(a, c), evaluate(b, a), evaluate(c, b), 1.];
        if result.iter().any(|v| !v.is_finite()) {
            return Err("Flicker arithmetic overflow".into());
        }
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rotator_zero_rate_stays_zero_and_signs_are_preserved() {
        let identity = [
            [1f32.to_bits(), 0x80000000, 0, 0],
            [0, 1f32.to_bits(), 0, 0],
        ];
        assert_eq!(rotator(42., 0.).unwrap(), identity);
        let negative = rotator(42., -0.).unwrap();
        assert_eq!(negative[0][1], 0);
        assert_eq!(negative[1][0], 0x80000000);
        assert_eq!(rotator(120.5, 2.).unwrap(), rotator(0.5, 2.).unwrap());
    }
    #[test]
    fn rotator_rounds_product_before_trigonometry_and_rejects_invalid_arguments() {
        let time = 0.37f32;
        let rate = 33.3f32;
        let arg = time * rate;
        assert_eq!(
            rotator(time, rate).unwrap()[0][0],
            ((arg as f64).cos() as f32).to_bits()
        );
        assert_ne!(
            ((time as f64 * rate as f64).cos() as f32).to_bits(),
            rotator(time, rate).unwrap()[0][0]
        );
        assert!(rotator(1., f32::NAN).is_err());
        assert!(rotator(119., f32::MAX).is_err());
    }
    #[test]
    fn sine_defaults_wraps_and_retains_signed_zero() {
        assert_eq!(sin_time(120.5, -0.).unwrap(), sin_time(0.5, 1.).unwrap());
        assert_eq!(sin_time(-120., 0.).unwrap()[0].to_bits(), 0x80000000);
        assert_eq!(
            sin_time(0.5, -1.).unwrap()[0],
            -sin_time(0.5, 1.).unwrap()[0]
        );
        assert!(sin_time(0., f32::NAN).is_err());
        assert!(sin_time(f32::INFINITY, 1.).is_err());
        assert!(sin_time(119., f32::MAX).is_err());
    }
    #[test]
    fn tangent_is_unclamped_and_does_not_return_fptan_stack_one() {
        assert_eq!(tan_time(0., 0.).unwrap(), [0.; 4]);
        assert_eq!(tan_time(-120., -0.).unwrap()[0].to_bits(), 0x80000000);
        assert!(tan_time(std::f32::consts::FRAC_PI_2, 1.).unwrap()[0].abs() > 1_000_000.);
        assert_eq!(tan_time(120.5, 0.).unwrap(), tan_time(0.5, 1.).unwrap());
        assert!(tan_time(1., f32::INFINITY).is_err());
    }
    #[test]
    fn circle_has_fixed_radius_homogeneous_tail_and_signed_time() {
        assert_eq!(xy_circle(0.).unwrap(), [500., 0., 0., 1.]);
        assert_eq!(
            xy_circle(-120.).unwrap().map(f32::to_bits),
            [500f32.to_bits(), 0x80000000, 0, 1f32.to_bits()]
        );
        assert_eq!(xy_circle(120.5).unwrap(), xy_circle(0.5).unwrap());
        let p = xy_circle(1.25).unwrap();
        let n = xy_circle(-1.25).unwrap();
        assert_eq!(p[0], n[0]);
        assert_eq!(p[1], -n[1]);
        assert!(xy_circle(f32::NAN).is_err());
    }
    #[test]
    fn signed_time_remainder_and_nonfinite_contract() {
        assert_eq!(shader_time(240.5).unwrap(), [0.5; 4]);
        assert_eq!(shader_time(-121.25).unwrap(), [-1.25; 4]);
        assert_eq!(shader_time(-120.).unwrap()[0].to_bits(), (-0f32).to_bits());
        assert!(shader_time(f32::INFINITY).is_err());
    }
    #[test]
    fn cosine_zero_rate_defaults_and_uses_wrapped_time() {
        assert_eq!(cos_time(0., 0.).unwrap(), [1.; 4]);
        assert_eq!(cos_time(120.5, 0.).unwrap(), cos_time(0.5, 1.).unwrap());
        assert_eq!(cos_time(0.5, -1.).unwrap(), cos_time(0.5, 1.).unwrap());
        assert!(cos_time(119., f32::MAX).is_err());
        assert!(cos_time(0., f32::NAN).is_err());
    }
    #[test]
    fn cold_flicker_uses_half_samples_without_rand_and_ignores_zw() {
        let mut s = Flicker::default();
        let r = s
            .evaluate(42., [0.1, 0.4, f32::NAN, f32::INFINITY], &mut || {
                panic!("cold state draws no random")
            })
            .unwrap();
        assert_eq!(r, [0.8, 0.8, 0.8, 1.]);
        assert_eq!(
            s.evaluate(42., [0.5, 1., 0., 0.], &mut || panic!())
                .unwrap(),
            [1.; 4]
        );
    }
    #[test]
    fn shared_rand_only_on_time_change_and_component_cross_coupling() {
        let mut s = Flicker::default();
        s.evaluate(0., [0.; 4], &mut || panic!()).unwrap();
        let mut values = [0u16, 32767, 16384].into_iter();
        let out = s
            .evaluate(1., [0.25, 1., 0., 0.], &mut || Ok(values.next().unwrap()))
            .unwrap();
        assert_eq!(out, [1., 0., 1., 1.]);
        assert_eq!(values.next(), None);
        assert_eq!(
            s.evaluate(1., [0.9, 0., 0., 0.], &mut || panic!()).unwrap(),
            [1.; 4]
        );
        let mut count = 0;
        s.evaluate(-1., [0.; 4], &mut || {
            count += 1;
            Ok(0)
        })
        .unwrap();
        assert_eq!(count, 3);
        let mut calls = 0;
        s.evaluate(119., [0.; 4], &mut || {
            calls += 1;
            Ok(0)
        })
        .unwrap();
        s.evaluate(239., [0.; 4], &mut || {
            calls += 1;
            Ok(0)
        })
        .unwrap();
        assert_eq!(calls, 6); // Flicker compares full engine time, not 120s remainder.
    }
    #[test]
    fn signed_zero_time_equal_and_unclamped_amplitude() {
        let mut s = Flicker::default();
        s.evaluate(-0., [-1., 2., 0., 0.], &mut || panic!())
            .unwrap();
        let r = s.evaluate(0., [-1., 4., 0., 0.], &mut || panic!()).unwrap();
        assert_eq!(r, [-1., -1., -1., 1.]);
    }
    #[test]
    fn callback_errors_keep_partial_state_and_invalid_inputs_do_not_draw() {
        let mut s = Flicker::default();
        s.evaluate(0., [0.; 4], &mut || panic!()).unwrap();
        let mut count = 0;
        assert!(s
            .evaluate(1., [0.; 4], &mut || {
                count += 1;
                if count == 1 {
                    Ok(0)
                } else {
                    Err("host failure".into())
                }
            })
            .is_err());
        assert_eq!(s.samples, [0., 0.5, 0.5]);
        assert_eq!(s.last_time, Some(1.));
        s.evaluate(1., [0.; 4], &mut || panic!("cached time remains updated"))
            .unwrap();
        assert!(s.evaluate(f32::NAN, [0.; 4], &mut || panic!()).is_err());
    }
}
