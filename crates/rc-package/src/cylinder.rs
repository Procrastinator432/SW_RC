//! Finite upright actor-cylinder against an upright circular pawn/point sweep.
//! World BSP continues to use its existing hull query.
use crate::hull::SweepHit;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cylinder {
    pub center: [f64; 3],
    pub radius: f64,
    pub height: f64,
}
impl Cylinder {
    pub fn validate(&self) -> Result<(), String> {
        if self
            .center
            .iter()
            .chain([&self.radius, &self.height])
            .any(|v| !v.is_finite() || v.abs() > 1e7)
            || self.radius <= 0.
            || self.height <= 0.
        {
            return Err("Invalid actor cylinder".into());
        }
        Ok(())
    }
    pub fn sweep(
        &self,
        start: [f64; 3],
        end: [f64; 3],
        extent: [f64; 3],
        motion: bool,
    ) -> Result<Option<SweepHit>, String> {
        self.validate()?;
        if start
            .iter()
            .chain(end.iter())
            .chain(extent.iter())
            .any(|v| !v.is_finite() || v.abs() > 1e9)
            || extent.iter().any(|v| *v < 0.)
            || extent[0] != extent[1]
        {
            return Err("Cylinder query requires a finite circular pawn extent".into());
        }
        let p: [f64; 3] = std::array::from_fn(|i| start[i] - self.center[i]);
        let d: [f64; 3] = std::array::from_fn(|i| end[i] - start[i]);
        let radius = self.radius + extent[0];
        let height = self.height + extent[2];
        let a = d[0] * d[0] + d[1] * d[1];
        let b = p[0] * d[0] + p[1] * d[1];
        let c = p[0] * p[0] + p[1] * p[1] - radius * radius;
        let mut enter = f64::NEG_INFINITY;
        let mut exit = f64::INFINITY;
        let mut normal = [0.; 3];
        let interior = c < -1e-8 && p[2].abs() < height - 1e-8;
        if a == 0. {
            if c > 1e-8 || (motion && c >= -1e-8) {
                return Ok(None);
            }
        } else {
            let disc = b * b - a * c;
            if disc < 0. || (motion && disc <= 1e-8) {
                return Ok(None);
            }
            let root = disc.max(0.).sqrt();
            enter = (-b - root) / a;
            exit = (-b + root) / a;
            normal = [
                (p[0] + d[0] * enter) / radius,
                (p[1] + d[1] * enter) / radius,
                0.,
            ];
        }
        if d[2] == 0. {
            if p[2].abs() > height + 1e-8 || (motion && p[2].abs() >= height - 1e-8) {
                return Ok(None);
            }
        } else {
            let t0 = (-height - p[2]) / d[2];
            let t1 = (height - p[2]) / d[2];
            let lo = t0.min(t1);
            if lo > enter {
                enter = lo;
                normal = [0., 0., -d[2].signum()];
            }
            exit = exit.min(t0.max(t1));
        }
        if enter > exit || exit < 0. || enter > 1. || (motion && !interior && exit <= 1e-10) {
            return Ok(None);
        }
        Ok(Some(SweepHit {
            hull_offset: None,
            fraction: enter.max(0.),
            exit_fraction: exit.clamp(0., 1.),
            normal,
            start_overlapping: interior,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn cylinder() -> Cylinder {
        Cylinder {
            center: [0.; 3],
            radius: 10.,
            height: 20.,
        }
    }
    #[test]
    fn diagonal_station_approach_clears_the_old_box_corner() {
        let c = cylinder();
        assert!(c
            .sweep([11., 11., 0.], [11., 11., 0.], [2., 2., 5.], true)
            .unwrap()
            .is_none());
        let h = c
            .sweep([20., 20., 0.], [0., 0., 0.], [2., 2., 5.], true)
            .unwrap()
            .unwrap();
        assert!((h.fraction - (1. - 12. / 800_f64.sqrt())).abs() < 1e-12);
        assert!((h.normal[0] - 0.5_f64.sqrt()).abs() < 1e-12);
    }
    #[test]
    fn contact_departure_tangent_and_penetration_are_distinct() {
        let c = cylinder();
        for end in [[15., 0., 0.], [10., 10., 0.], [10., 0., 0.]] {
            assert!(c
                .sweep([10., 0., 0.], end, [0.; 3], true)
                .unwrap()
                .is_none());
        }
        assert!(c
            .sweep([10., 0., 0.], [9., 0., 0.], [0.; 3], true)
            .unwrap()
            .is_some());
        assert!(c
            .sweep([10., 0., 0.], [10., 0., 0.], [0.; 3], false)
            .unwrap()
            .is_some());
        assert!(
            c.sweep([0.; 3], [0.; 3], [0.; 3], true)
                .unwrap()
                .unwrap()
                .start_overlapping
        );
        assert!(c
            .sweep([-20., 10., 0.], [20., 10., 0.], [0.; 3], true)
            .unwrap()
            .is_none());
    }
    #[test]
    fn end_caps_and_separated_intervals() {
        let c = cylinder();
        let hit = c
            .sweep([0., 0., 40.], [0., 0., 0.], [2., 2., 5.], true)
            .unwrap()
            .unwrap();
        assert_eq!(hit.fraction, 0.375);
        assert_eq!(hit.normal, [0., 0., 1.]);
        assert!(c
            .sweep([-30., 0., 50.], [30., 0., 30.], [0.; 3], true)
            .unwrap()
            .is_none());
        assert!(c.sweep([0.; 3], [0.; 3], [2., 3., 5.], true).is_err());
        assert!(c.sweep([f64::NAN, 0., 0.], [0.; 3], [0.; 3], true).is_err());
    }
}
