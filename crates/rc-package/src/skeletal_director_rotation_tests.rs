use super::*;
use crate::quaternion_animation::PortableQuaternionMath;
fn q(w: f32) -> [u32; 4] {
    [0, 0, 0, w.to_bits()]
}
fn matrix() -> [u32; 16] {
    quaternion_translation_matrix(RootTransform {
        rotation: q(1.),
        position: [1f32, 2., 3.].map(f32::to_bits),
    })
}
fn director() -> BoneDirector {
    let mut words = [0; 28];
    words[18] = 0x100;
    words[20] = (-1f32).to_bits();
    words[21] = (-1f32).to_bits();
    words[26] = 0.5f32.to_bits();
    words[27] = 0xaabbcc00;
    BoneDirector { words }
}
#[derive(Default)]
struct Host {
    events: Vec<String>,
    fail: usize,
}
impl Host {
    fn event(&mut self, s: String) -> Result<(), String> {
        self.events.push(s);
        if self.events.len() == self.fail {
            Err("boundary".into())
        } else {
            Ok(())
        }
    }
}
impl DirectorRotationHost for Host {
    fn matrix_quaternion(&mut self, _: [u32; 16]) -> Result<[u32; 4], String> {
        self.event("matrix".into())?;
        Ok(q(0.75))
    }
    fn angle_difference(&mut self, _: [u32; 4], _: [u32; 4]) -> Result<f32, String> {
        self.event("difference".into())?;
        Ok(2.)
    }
    fn slerp(&mut self, _: [u32; 4], _: [u32; 4], t: f32) -> Result<[u32; 4], String> {
        self.event(format!("slerp:{t}"))?;
        Ok(q(0.5))
    }
    fn rotation_angle_fast(&mut self, _: [u32; 4]) -> Result<f32, String> {
        self.event("angle".into())?;
        Ok(4.)
    }
    fn power(&mut self, _: [u32; 4], t: f32) -> Result<[u32; 4], String> {
        self.event(format!("power:{t}"))?;
        Ok(q(0.25))
    }
}
#[test]
fn initializes_history_byte_preserving_padding_and_keeps_unlimited_matrix() {
    let mut d = director();
    let mut h = Host::default();
    let supplied = matrix();
    let scale = [2f32.to_bits(); 3];
    let result = prepare_director_rotation(&mut d, matrix(), supplied, scale, &mut h).unwrap();
    assert_eq!(h.events, vec!["matrix", "matrix"]);
    assert_eq!(d.words[27], 0xaabbcc01);
    assert_eq!(&d.words[22..26], &q(0.75));
    assert_eq!(d.words[26], 0.5f32.to_bits());
    assert_eq!(result.matrix, supplied);
    assert_eq!(result.scale, scale);
}
#[test]
fn temporal_then_absolute_rebuilds_preserve_translation_reset_scale_and_commit_history() {
    let mut d = director();
    d.words[21] = 0;
    d.words[20] = 1f32.to_bits();
    let mut h = Host::default();
    let out =
        prepare_director_rotation(&mut d, matrix(), matrix(), [2f32.to_bits(); 3], &mut h).unwrap();
    assert_eq!(
        h.events,
        vec![
            "matrix",
            "matrix",
            "difference",
            "slerp:0.25",
            "angle",
            "power:0.25"
        ]
    );
    assert!(out.temporal_limited && out.absolute_limited);
    assert_eq!(out.scale, [1f32.to_bits(); 3]);
    assert_eq!(out.quaternion, q(0.25));
    assert_eq!(
        out.matrix,
        quaternion_translation_matrix(RootTransform {
            rotation: q(0.25),
            position: [1f32, 2., 3.].map(f32::to_bits)
        })
    );
    assert_eq!(d.words[26], 0);
    assert_eq!(&d.words[22..26], &q(0.25));
}
#[test]
fn active_budget_is_cleared_even_when_no_limiting_occurs() {
    let mut d = director();
    d.words[21] = (-0f32).to_bits();
    d.words[26] = 2f32.to_bits();
    d.words[20] = 4f32.to_bits();
    let mut h = Host::default();
    let out = prepare_director_rotation(&mut d, matrix(), matrix(), [0; 3], &mut h).unwrap();
    assert!(!out.temporal_limited && !out.absolute_limited);
    assert_eq!(d.words[26], 0);
    assert_eq!(out.scale, [0; 3]);
    assert_eq!(h.events, vec!["matrix", "matrix", "difference", "angle"]);
}
#[test]
fn nan_limits_skip_calls_and_existing_history_skips_initialization() {
    let mut d = director();
    d.words[27] = 0x12345602;
    d.words[20] = f32::NAN.to_bits();
    d.words[21] = f32::NAN.to_bits();
    let mut h = Host::default();
    prepare_director_rotation(&mut d, matrix(), matrix(), [0; 3], &mut h).unwrap();
    assert_eq!(h.events, vec!["matrix"]);
    assert_eq!(d.words[27], 0x12345602);
    assert_eq!(d.words[26], 0.5f32.to_bits());
}
#[test]
fn failure_snapshots_preserve_completed_native_writes() {
    for fail in 1..=6 {
        let mut d = director();
        d.words[21] = 0;
        d.words[20] = 1f32.to_bits();
        let before = d.words;
        let mut h = Host {
            fail,
            ..Default::default()
        };
        assert!(prepare_director_rotation(&mut d, matrix(), matrix(), [0; 3], &mut h).is_err());
        let mut expected = before;
        if fail > 1 {
            expected[22..26].copy_from_slice(&q(0.75));
            expected[27] = 0xaabbcc01;
        }
        if fail > 4 {
            expected[26] = 0;
        }
        assert_eq!(d.words, expected, "fail stage {fail}");
        assert_eq!(h.events.len(), fail);
    }
}
#[test]
fn disabled_rotation_rejects_before_host_calls_or_state_writes() {
    let mut d = director();
    d.words[18] = 0;
    let before = d.words;
    let mut h = Host::default();
    assert!(prepare_director_rotation(&mut d, matrix(), matrix(), [0; 3], &mut h).is_err());
    assert_eq!(d.words, before);
    assert!(h.events.is_empty());
}
#[test]
fn portable_fast_angles_clamp_abs_and_unordered() {
    let mut h = PortableDirectorRotationHost {
        math: PortableQuaternionMath,
    };
    assert_eq!(h.rotation_angle_fast(q(2.)).unwrap(), 0.);
    assert_eq!(h.rotation_angle_fast(q(f32::NAN)).unwrap(), 0.);
    assert_eq!(h.angle_difference(q(f32::NAN), q(1.)).unwrap(), 0.);
    assert!((h.rotation_angle_fast(q(-0.5)).unwrap() - 2.0943952).abs() < 1e-6);
    assert!((h.angle_difference(q(-0.5), q(1.)).unwrap() - 2.0943952).abs() < 1e-6);
}
#[test]
fn portable_power_uses_vector_angle_preserves_zero_and_scales_finite_turn() {
    let mut h = PortableDirectorRotationHost {
        math: PortableQuaternionMath,
    };
    let turn = [0f32, 0., 0.6, 0.8].map(f32::to_bits);
    let half = h.power(turn, 0.5).unwrap().map(f32::from_bits);
    assert!((half[2] - 0.31622776).abs() < 2e-6);
    assert!((half[3] - 0.9486833).abs() < 2e-6);
    assert_eq!(h.power(q(-1.), 0.5).unwrap(), q(-1.));
    // RotationAngle for power ignores W, unlike RotationAngleFast.
    let another = [turn[0], turn[1], turn[2], 0];
    assert_eq!(h.power(another, 0.5).unwrap(), h.power(turn, 0.5).unwrap());
}
