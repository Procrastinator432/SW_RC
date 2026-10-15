//! Selected geo_01a trigger621 and scripts325/754. The caller supplies actual
//! projectile movement clipped at world collisions and applies emitted effects.
use crate::{crash::Trigger, Point};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub contact: Trigger,
    pub destruction_delay: f64,
    pub confirmation_delay: f64,
    pub confirmation_seconds: f64,
    pub after_confirmation_delay: f64,
    pub bunker_health: u32,
    pub bunker_script_damage: u32,
}
impl Assets {
    pub fn validate(&self) -> Result<(), String> {
        if !self.contact.validate()
            || self.contact.original != 621
            || [
                self.destruction_delay,
                self.confirmation_delay,
                self.confirmation_seconds,
                self.after_confirmation_delay,
            ]
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=120.).contains(v))
            || self.confirmation_seconds == 0.
            || self.bunker_health == 0
            || self.bunker_script_damage < self.bunker_health
        {
            return Err("Invalid selected gun-position assets".into());
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContactClass {
    Pawn,
    Bullet,
    GrenadeProjectile,
}
#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
pub enum Effect {
    Event(String),
    Tracker(String),
    ObjectiveCompleted(u32),
    HurtActor { tag: String, damage: u32 },
    DestroyActor(String),
    DisableCollision(String),
    StopFactory(String),
    ConfirmationSound,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, serde::Deserialize)]
pub struct State {
    pub elapsed: Option<f64>,
    pub objective_completed: bool,
    pub bunker_destroyed: bool,
    pub confirmation_started: bool,
    pub confirmation_finished: bool,
    pub completed: bool,
}
/// Continuous point-projectile/cylinder contact. The caller must stop the segment
/// at the first blocking collision; this function cannot see or bypass a wall.
fn crosses(t: &Trigger, from: Point, to: Point) -> bool {
    let p = std::array::from_fn::<_, 3, _>(|i| from[i] - t.position[i]);
    let d = std::array::from_fn::<_, 3, _>(|i| to[i] - from[i]);
    let mut low: f64 = 0.;
    let mut high: f64 = 1.;
    if d[2].abs() < 1e-12 {
        if p[2].abs() > t.height {
            return false;
        }
    } else {
        let a = (-t.height - p[2]) / d[2];
        let b = (t.height - p[2]) / d[2];
        low = low.max(a.min(b));
        high = high.min(a.max(b));
    }
    let a = d[0] * d[0] + d[1] * d[1];
    let c = p[0] * p[0] + p[1] * p[1] - t.radius * t.radius;
    if a < 1e-12 {
        if c > 0. {
            return false;
        }
    } else {
        let b = 2. * (p[0] * d[0] + p[1] * d[1]);
        let disc = b * b - 4. * a * c;
        if disc < 0. {
            return false;
        }
        low = low.max((-b - disc.sqrt()) / (2. * a));
        high = high.min((-b + disc.sqrt()) / (2. * a));
    }
    low <= high
}
impl State {
    pub fn observe(
        &mut self,
        a: &Assets,
        from: Point,
        to: Point,
        class: ContactClass,
    ) -> Result<Vec<Effect>, String> {
        a.validate()?;
        if from
            .iter()
            .chain(&to)
            .any(|v| !v.is_finite() || v.abs() > 1e8)
        {
            return Err("Invalid projectile contact segment".into());
        }
        if self.elapsed.is_some()
            || class != ContactClass::GrenadeProjectile
            || !crosses(&a.contact, from, to)
        {
            return Ok(vec![]);
        }
        self.elapsed = Some(0.);
        self.objective_completed = true;
        // Script754 does not wait for script325's one-second destruction delay.
        Ok(vec![
            Effect::Event("MachineGunBunkerDestroyed".into()),
            Effect::Event("UnblockBunkerPath".into()),
            Effect::Tracker("TrackerTarget09".into()),
            Effect::ObjectiveCompleted(3),
        ])
    }
    pub fn advance(&mut self, a: &Assets, dt: f64) -> Result<Vec<Effect>, String> {
        a.validate()?;
        if !dt.is_finite() || !(0. ..=1.).contains(&dt) {
            return Err("Invalid gun-position time step".into());
        }
        let Some(elapsed) = &mut self.elapsed else {
            return Ok(vec![]);
        };
        if self.completed {
            return Ok(vec![]);
        }
        *elapsed += dt;
        let now = *elapsed;
        let mut effects = vec![];
        if !self.bunker_destroyed && now + 1e-9 >= a.destruction_delay {
            self.bunker_destroyed = true;
            effects.extend([
                Effect::Event("SmallBunkerExplosion".into()),
                Effect::Event("SpawnSmallBunker".into()),
                Effect::StopFactory("SmallBunkerDroidProjectileEFX".into()),
                Effect::HurtActor {
                    tag: "SmallBunker".into(),
                    damage: a.bunker_script_damage,
                },
                Effect::DestroyActor("SmallBunkerHatch".into()),
                Effect::DisableCollision("BunkerCollision".into()),
            ]);
        }
        if !self.confirmation_started && now + 1e-9 >= a.confirmation_delay {
            self.confirmation_started = true;
            effects.push(Effect::ConfirmationSound);
        }
        let finished = a.confirmation_delay + a.confirmation_seconds;
        if !self.confirmation_finished && now + 1e-9 >= finished {
            self.confirmation_finished = true;
            effects.push(Effect::Event("BunkerProjectileEFX".into()));
        }
        if self.bunker_destroyed && now + 1e-9 >= finished + a.after_confirmation_delay {
            self.completed = true;
            effects.extend([
                Effect::Event("MissionObjectives".into()),
                Effect::Event("Conversation04".into()),
            ]);
        }
        Ok(effects)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn assets() -> Assets {
        Assets {
            contact: Trigger {
                original: 621,
                position: [0.; 3],
                radius: 100.,
                height: 40.,
            },
            destruction_delay: 1.,
            confirmation_delay: 2.,
            confirmation_seconds: 1.5,
            after_confirmation_delay: 2.,
            bunker_health: 49000,
            bunker_script_damage: 50000,
        }
    }
    #[test]
    fn grenade_class_contact_is_required_and_once() {
        let a = assets();
        let mut s = State::default();
        for class in [ContactClass::Pawn, ContactClass::Bullet] {
            assert!(s
                .observe(&a, [-200., 0., 0.], [200., 0., 0.], class)
                .unwrap()
                .is_empty());
        }
        assert!(!s.objective_completed);
        let e = s
            .observe(
                &a,
                [-200., 0., 0.],
                [200., 0., 0.],
                ContactClass::GrenadeProjectile,
            )
            .unwrap();
        assert_eq!(e.last(), Some(&Effect::ObjectiveCompleted(3)));
        assert!(s.objective_completed && !s.bunker_destroyed);
        assert!(s
            .observe(&a, [0.; 3], [0.; 3], ContactClass::GrenadeProjectile)
            .unwrap()
            .is_empty());
    }
    #[test]
    fn cylinder_requires_simultaneous_vertical_and_horizontal_overlap() {
        let a = assets();
        for (from, to) in [
            ([-200., 0., 41.], [200., 0., 41.]),
            ([-200., 101., 0.], [200., 101., 0.]),
            ([-200., 0., 0.], [-101., 0., 0.]),
            ([-200., 0., 0.], [0., 0., 200.]),
        ] {
            assert!(!crosses(&a.contact, from, to));
        }
        assert!(crosses(&a.contact, [-200., 0., 200.], [200., 0., -200.]));
        assert!(crosses(&a.contact, [0.; 3], [0.; 3]));
    }
    #[test]
    fn objective_and_delayed_geometry_confirmation_are_independent() {
        let a = assets();
        let mut s = State::default();
        s.observe(&a, [0.; 3], [0.; 3], ContactClass::GrenadeProjectile)
            .unwrap();
        assert!(s.advance(&a, 0.9).unwrap().is_empty());
        let e = s.advance(&a, 0.1).unwrap();
        assert!(e.contains(&Effect::HurtActor {
            tag: "SmallBunker".into(),
            damage: 50000
        }));
        assert_eq!(s.advance(&a, 1.).unwrap(), vec![Effect::ConfirmationSound]);
        s.advance(&a, 1.).unwrap();
        assert_eq!(
            s.advance(&a, 0.5).unwrap(),
            vec![Effect::Event("BunkerProjectileEFX".into())]
        );
        s.advance(&a, 1.).unwrap();
        assert!(!s.completed);
        assert_eq!(
            s.advance(&a, 1.).unwrap(),
            vec![
                Effect::Event("MissionObjectives".into()),
                Effect::Event("Conversation04".into())
            ]
        );
        assert!(s.completed && s.bunker_destroyed);
        assert!(s.advance(&a, 1.).unwrap().is_empty());
    }
    #[test]
    fn partitioned_time_preserves_effect_order() {
        let a = assets();
        let mut s = State::default();
        s.observe(&a, [0.; 3], [0.; 3], ContactClass::GrenadeProjectile)
            .unwrap();
        let mut other = s.clone();
        let mut left = vec![];
        let mut right = vec![];
        for _ in 0..6 {
            left.extend(s.advance(&a, 1.).unwrap());
        }
        for _ in 0..720 {
            right.extend(other.advance(&a, 1. / 120.).unwrap());
        }
        assert_eq!(left, right);
        assert_eq!(s.completed, other.completed);
    }
    #[test]
    fn invalid_data_keeps_state_unchanged() {
        let a = assets();
        let mut s = State::default();
        let before = s.clone();
        assert!(s
            .observe(&a, [f64::NAN; 3], [0.; 3], ContactClass::GrenadeProjectile)
            .is_err());
        assert_eq!(s, before);
        assert!(s.advance(&a, -1.).is_err());
        assert_eq!(s, before);
        let mut bad = a;
        bad.bunker_script_damage = 400;
        assert!(s
            .observe(&bad, [0.; 3], [0.; 3], ContactClass::GrenadeProjectile)
            .is_err());
        assert_eq!(s, before);
    }
}
