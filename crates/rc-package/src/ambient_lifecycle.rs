//! Selected original PC ambient admission/release and non-streamed linear fade.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub radius_multiplier: f32,
    pub release_margin: f32,
    pub fade_seconds: f32,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            radius_multiplier: 100.,
            release_margin: 10.,
            fade_seconds: 0.3,
        }
    }
}
impl Policy {
    pub fn validate(&self) -> Result<(), String> {
        if self.radius_multiplier != 100. || self.release_margin != 10. || self.fade_seconds != 0.3
        {
            return Err("Unsupported original ambient lifecycle policy".into());
        }
        Ok(())
    }
    pub fn distances(
        &self,
        location: [f32; 3],
        listener: [f32; 3],
        radius: f32,
        occlusion: f32,
    ) -> Result<Distances, String> {
        self.validate()?;
        if location
            .iter()
            .chain(&listener)
            .any(|v| !v.is_finite() || v.abs() > 1e6)
            || !radius.is_finite()
            || !(0. ..=1e6).contains(&radius)
            || !occlusion.is_finite()
            || !(0. ..=1.).contains(&occlusion)
        {
            return Err("Invalid ambient distance input".into());
        }
        let [x, y, z] = std::array::from_fn(|i| location[i] - listener[i]);
        let squared = (z * z + y * y) + x * x;
        let effective = (radius * self.radius_multiplier) * occlusion;
        Ok(Distances {
            admit: squared <= effective * effective,
            release: squared
                > (effective + self.release_margin) * (effective + self.release_margin),
            release_queued: squared > effective * effective + self.release_margin,
        })
    }
}
#[derive(Debug)]
pub struct Distances {
    pub admit: bool,
    pub release: bool,
    pub release_queued: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fade {
    pub started: u64,
    pub elapsed: f32,
    pub value: f32,
}
impl Fade {
    pub fn new(started: u64) -> Self {
        Self {
            started,
            elapsed: 0.,
            value: 1.,
        }
    }
    /// GetFadeMultiplier 103d2810; curve zero for non-streamed StopSound.
    pub fn step(&mut self) -> bool {
        let dt = 1f32 / 60.;
        self.elapsed += dt;
        self.value = (self.value + (-1f32 / 0.3) * dt).max(0.);
        if self.elapsed >= 0.3 {
            self.value = 0.;
            true
        } else {
            false
        }
    }
    pub fn validate(&self, voice_started: u64, tick: u64) -> Result<(), String> {
        if self.started < voice_started || self.started > tick || tick - self.started >= 18 {
            return Err("Invalid ambient fade time".into());
        }
        let mut expected = Self::new(self.started);
        for _ in 0..tick - self.started {
            expected.step();
        }
        if self.elapsed.to_bits() != expected.elapsed.to_bits()
            || self.value.to_bits() != expected.value.to_bits()
        {
            return Err("Invalid saved ambient fade curve".into());
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn active_and_queued_release_keep_distinct_original_boundaries() {
        let p = Policy::default();
        assert!(
            p.distances([1000., 0., 0.], [0.; 3], 10., 1.)
                .unwrap()
                .admit
        );
        let d = p.distances([1001., 0., 0.], [0.; 3], 10., 1.).unwrap();
        assert!(!d.admit && !d.release && d.release_queued);
        assert!(
            !p.distances([1010., 0., 0.], [0.; 3], 10., 1.)
                .unwrap()
                .release
        );
        assert!(
            p.distances([1010.001, 0., 0.], [0.; 3], 10., 1.)
                .unwrap()
                .release
        );
    }
    #[test]
    fn invalid_policy_and_geometry_are_rejected() {
        assert!(Policy {
            radius_multiplier: f32::NAN,
            ..Default::default()
        }
        .validate()
        .is_err());
        for radius in [f32::NAN, f32::INFINITY, -1.] {
            assert!(Policy::default()
                .distances([0.; 3], [0.; 3], radius, 1.)
                .is_err());
        }
        assert!(Policy::default()
            .distances([0.; 3], [0.; 3], 10., 1.01)
            .is_err());
    }
    #[test]
    fn saved_fade_is_exact_and_rejects_modified_value_and_future_time() {
        let mut f = Fade::new(100);
        for tick in 101..118 {
            assert!(!f.step());
            f.validate(10, tick).unwrap();
        }
        let b = serde_json::to_vec(&f).unwrap();
        let mut restored: Fade = serde_json::from_slice(&b).unwrap();
        assert!(f.step() && restored.step());
        assert_eq!(f.value, 0.);
        assert_eq!(restored.elapsed, f.elapsed);
        restored.value = 0.5;
        assert!(restored.validate(10, 118).is_err());
        assert!(Fade::new(120).validate(10, 119).is_err());
    }
    #[test]
    fn matches_original_distance_and_fade_instruction_cases() {
        let data: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/ambient-lifecycle-native.json"
        ))
        .unwrap();
        for c in data["distances"].as_array().unwrap() {
            let location = serde_json::from_value(c["location"].clone()).unwrap();
            let listener = serde_json::from_value(c["listener"].clone()).unwrap();
            let d = Policy::default()
                .distances(
                    location,
                    listener,
                    c["radius"].as_f64().unwrap() as f32,
                    c["occlusion"].as_f64().unwrap() as f32,
                )
                .unwrap();
            assert_eq!(d.admit, c["admit"].as_bool().unwrap());
            assert_eq!(d.release, c["release"].as_bool().unwrap());
            assert_eq!(d.release_queued, c["release_queued"].as_bool().unwrap());
        }
        let mut f = Fade::new(0);
        for row in data["fade"].as_array().unwrap() {
            f.step();
            assert_eq!(
                u64::from(f.value.to_bits()),
                row["value_bits"].as_u64().unwrap()
            );
            assert_eq!(
                u64::from(f.elapsed.to_bits()),
                row["elapsed_bits"].as_u64().unwrap()
            );
        }
    }
}
