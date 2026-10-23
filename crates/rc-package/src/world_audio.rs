//! Selected original PC world-source arithmetic, independent of a playback host.
//!
//! Geometry, duck/fade controllers and source allocation remain caller-owned.
//! In particular, SoundPriority is admission priority, not audible attenuation.
use serde::{Deserialize, Serialize};

const MAX_INPUT: f32 = 1e6;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Priority {
    pub location: [f32; 3],
    pub listener: [f32; 3],
    pub volume: f32,
    pub radius: f32,
    pub radius_multiplier: f32,
    pub flags: u32,
    /// Already decoded `(original_slot >> 1) & 7`, not AudioParams.Category.
    pub slot: u8,
}

impl Priority {
    /// UAudioSubsystem::SoundPriority, engine.dll 103d64e0.
    /// The distance part uses SSE binary32; the final sum uses x87.
    pub fn admission(&self) -> Result<f32, String> {
        if self
            .location
            .iter()
            .chain(&self.listener)
            .any(|v| !v.is_finite() || v.abs() > MAX_INPUT)
            || !bounded(self.volume, -16., 16.)
            || !bounded(self.radius, 0., MAX_INPUT)
            || !bounded(self.radius_multiplier, 0.0001, MAX_INPUT)
            || self.slot > 7
        {
            return Err("Invalid original sound priority input".into());
        }
        let distance = if self.radius == 0. || self.flags & 0x100 != 0 {
            1.
        } else {
            let [x, y, z] = std::array::from_fn(|i| self.location[i] - self.listener[i]);
            let squared = (z * z + y * y) + x * x;
            let radius = self.radius_multiplier * self.radius;
            (1. - squared / (radius * radius)).clamp(0.01, 1.)
        };
        let volume = self.volume.clamp(0., 1.);
        // Despite the decompiler's integer temporary this is a FLOAT store.
        let weight = if volume == 0. {
            0.
        } else {
            volume * 0.15 + 0.85
        };
        let flags = f64::from((self.flags >> 8) & 1) + f64::from((self.flags >> 2) & 1);
        let result = (flags
            + f64::from(weight) * f64::from(distance)
            + if self.slot == 7 { 3. } else { 0. }
            + if self.slot == 5 { 2. } else { 0. }
            + if self.slot == 0 { 1. } else { 0. }) as f32;
        if result.is_finite() {
            Ok(result)
        } else {
            Err("Original sound priority overflow".into())
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Radii {
    /// Source+78: radius used by the separate running-source priority path.
    pub priority_radius: f32,
    /// Source+7c: smoothed radius.
    pub current: f32,
    /// Source+80 and +84: previous target and engine time of its last change.
    pub target: f32,
    pub changed_at: f64,
}

impl Radii {
    /// UpdateRadii 103d267f..103d2771, excluding profiling instructions.
    /// `None` means local/nonpositional or a clear geometry query; Some(scale)
    /// is the ORIGINAL geometry callback result, not a guessed wall factor.
    pub fn update(
        &self,
        base_radius: f32,
        occlusion_scale: Option<f32>,
        now: f32,
    ) -> Result<Self, String> {
        if !bounded(base_radius, 0., MAX_INPUT)
            || occlusion_scale.is_some_and(|v| !bounded(v, 0., 1.))
            || !self.priority_radius.is_finite()
            || !self.current.is_finite()
            || !self.target.is_finite()
            || !self.changed_at.is_finite()
            || !now.is_finite()
        {
            return Err("Invalid original sound radius input".into());
        }
        let priority_radius = occlusion_scale.map_or(base_radius, |s| s * base_radius);
        let target = if occlusion_scale.is_some() {
            priority_radius * 0.35
        } else {
            priority_radius
        };
        let elapsed = f64::from(now) - self.changed_at;
        // Preserve this surprising original branch. A positive change time
        // selects interpolation even after one second; do not clamp elapsed.
        let current = if self.changed_at > 0. || elapsed < 1. {
            ((f64::from(target) - f64::from(self.current)) * elapsed + f64::from(self.current))
                as f32
        } else {
            target
        };
        if !current.is_finite() {
            return Err("Original sound radius overflow".into());
        }
        Ok(Self {
            priority_radius,
            current,
            target,
            changed_at: if self.target != target {
                f64::from(now)
            } else {
                self.changed_at
            },
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Volume {
    pub base_volume: f32,
    pub duck: f32,
    pub fade: f32,
    pub held_fade: f32,
    pub mode_before: u8,
    pub mode_after: u8,
    pub flags: u32,
    pub paused: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VolumeUpdate {
    pub volume: f32,
    pub held_fade: f32,
    pub stop: bool,
}

impl Volume {
    /// FAbstractSource::UpdateVolume 103d0c60. `duck`, `fade` and mode_after
    /// are supplied callback outputs; this does not implement those controllers.
    pub fn update(&self) -> Result<VolumeUpdate, String> {
        if !bounded(self.base_volume, 0., 16.)
            || [self.duck, self.fade, self.held_fade]
                .iter()
                .any(|v| !bounded(*v, 0., 1.))
            || self.mode_before > 2
            || self.mode_after > 2
        {
            return Err("Invalid original sound volume input".into());
        }
        let volume = self.base_volume * self.duck;
        let held_fade = if self.mode_before != 0 {
            self.fade
        } else {
            self.held_fade
        };
        let stop =
            self.flags & 0x4000 == 0 && held_fade <= 0.0001 && self.mode_after != 1 && !self.paused;
        Ok(VolumeUpdate {
            volume: if stop { volume } else { held_fade * volume },
            held_fade,
            stop,
        })
    }
}

fn bounded(v: f32, min: f32, max: f32) -> bool {
    v.is_finite() && (min..=max).contains(&v)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Deserialize)]
    struct Data {
        priority: Vec<serde_json::Value>,
        radii: Vec<serde_json::Value>,
        volumes: Vec<serde_json::Value>,
    }
    fn data() -> Data {
        serde_json::from_str(include_str!("../../../fixtures/world-audio-native.json")).unwrap()
    }
    #[test]
    fn admission_matches_1000_original_calls() {
        let cases = data().priority;
        assert_eq!(cases.len(), 1000);
        for (i, mut c) in cases.into_iter().enumerate() {
            let expected = c.as_object_mut().unwrap().remove("expected_bits").unwrap();
            let p: Priority = serde_json::from_value(c).unwrap();
            assert_eq!(
                u64::from(p.admission().unwrap().to_bits()),
                expected.as_u64().unwrap(),
                "case {i}"
            );
        }
    }
    #[test]
    fn radii_match_1000_original_regions() {
        #[derive(Deserialize)]
        struct Case {
            base_radius: f32,
            occlusion_scale: Option<f32>,
            current: f32,
            previous_target: f32,
            changed_at: f64,
            now: f32,
            expected: serde_json::Value,
        }
        let cases = data().radii;
        assert_eq!(cases.len(), 1000);
        for (i, c) in cases.into_iter().enumerate() {
            let c: Case = serde_json::from_value(c).unwrap();
            let old = Radii {
                priority_radius: 0.,
                current: c.current,
                target: c.previous_target,
                changed_at: c.changed_at,
            };
            let result = old.update(c.base_radius, c.occlusion_scale, c.now).unwrap();
            for (key, value) in [
                ("priority_radius_bits", result.priority_radius),
                ("current_bits", result.current),
                ("target_bits", result.target),
            ] {
                assert_eq!(
                    u64::from(value.to_bits()),
                    c.expected[key].as_u64().unwrap(),
                    "case {i}: {key}"
                );
            }
            assert_eq!(
                result.changed_at,
                c.expected["changed_at"].as_f64().unwrap(),
                "case {i}"
            );
        }
    }
    #[test]
    fn volume_matches_1000_original_calls() {
        let cases = data().volumes;
        assert_eq!(cases.len(), 1000);
        for (i, mut c) in cases.into_iter().enumerate() {
            let expected = c.as_object_mut().unwrap().remove("expected").unwrap();
            let p: Volume = serde_json::from_value(c).unwrap();
            let result = p.update().unwrap();
            assert_eq!(
                u64::from(result.volume.to_bits()),
                expected["volume_bits"].as_u64().unwrap(),
                "case {i}"
            );
            assert_eq!(
                u64::from(result.held_fade.to_bits()),
                expected["held_fade_bits"].as_u64().unwrap(),
                "case {i}"
            );
            assert_eq!(result.stop, expected["stop"].as_bool().unwrap(), "case {i}");
        }
    }
    #[test]
    fn invalid_inputs_are_rejected_without_changing_radius_state() {
        let state = Radii {
            priority_radius: 100.,
            current: 100.,
            target: 100.,
            changed_at: 0.,
        };
        let saved = serde_json::to_vec(&state).unwrap();
        for v in [f32::NAN, f32::INFINITY, -1.] {
            assert!(state.update(v, None, 1.).is_err());
            assert!(state.update(100., Some(v), 1.).is_err());
        }
        assert!(state.update(100., None, f32::NAN).is_err());
        assert_eq!(serde_json::to_vec(&state).unwrap(), saved);
        let restored: Radii = serde_json::from_slice(&saved).unwrap();
        assert_eq!(
            state.update(200., None, 0.5).unwrap(),
            restored.update(200., None, 0.5).unwrap()
        );
    }
    #[test]
    fn admission_floor_is_not_audible_attenuation_and_flags_remain_independent() {
        let mut p = Priority {
            location: [1000., 0., 0.],
            listener: [0.; 3],
            volume: 1.,
            radius: 100.,
            radius_multiplier: 1.,
            flags: 0,
            slot: 2,
        };
        assert_eq!(p.admission().unwrap(), 0.01);
        p.flags = 0x100;
        assert_eq!(p.admission().unwrap(), 2.);
        p.flags |= 4;
        assert_eq!(p.admission().unwrap(), 3.);
        p.volume = 0.;
        assert_eq!(p.admission().unwrap(), 2.);
        p.slot = 7;
        assert_eq!(p.admission().unwrap(), 5.);
    }
    #[test]
    fn radius_keeps_original_interpolation_branch_and_geometry_scale_separate() {
        let state = Radii {
            priority_radius: 100.,
            current: 100.,
            target: 100.,
            changed_at: 1.,
        };
        let out = state.update(200., Some(0.5), 3.).unwrap();
        assert_eq!(out.priority_radius, 100.);
        assert_eq!(out.target, 35.);
        // Original elapsed=2 extrapolates; replacing it with clamp(0,1) is wrong.
        assert_eq!(out.current, -30.);
        assert_eq!(out.changed_at, 3.);
    }
    #[test]
    fn invalid_priority_and_volume_boundaries_are_rejected() {
        let mut p: Priority = serde_json::from_value({
            let mut c = data().priority.remove(0);
            c.as_object_mut().unwrap().remove("expected_bits");
            c
        })
        .unwrap();
        for v in [f32::NAN, f32::INFINITY, -1.] {
            p.radius = v;
            assert!(p.admission().is_err());
        }
        p.radius = 100.;
        p.slot = 8;
        assert!(p.admission().is_err());
        p.slot = 2;
        p.radius_multiplier = 0.;
        assert!(p.admission().is_err());
        p.radius_multiplier = 1.;
        p.radius = f32::from_bits(1);
        p.flags = 0;
        p.location = p.listener;
        assert!(p.admission().is_err());
        let mut v: Volume = serde_json::from_value({
            let mut c = data().volumes.remove(0);
            c.as_object_mut().unwrap().remove("expected");
            c
        })
        .unwrap();
        for bad in [f32::NAN, f32::INFINITY, -1., 2.] {
            v.duck = bad;
            assert!(v.update().is_err());
        }
        v.duck = 1.;
        v.mode_before = 3;
        assert!(v.update().is_err());
    }
}
