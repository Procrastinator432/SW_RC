//! Bounded geo_01a scripts 745/747 and original ammo pickup semantics.
//! This host emits effects only: it does not walk, draw, play audio or throw grenades.
use crate::{crash::Trigger, Point};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cue {
    pub source: String,
    pub seconds: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub provenance: serde_json::Value,
    pub dialog_trigger: Trigger,
    pub objective_trigger: Trigger,
    pub pickup: Trigger,
    pub initial_ammo: u32,
    pub maximum_ammo: u32,
    pub pickup_amount: u32,
    pub respawn_seconds: f64,
    /// Intro line 1, intro line 2, inventory confirmation, pickup sound.
    pub cues: Vec<Cue>,
    pub blink_seconds: f64,
    pub prompt_delay: f64,
    pub next_dialog_delay: f64,
}
impl Assets {
    pub fn validate(&self) -> Result<(), String> {
        let mut ids = std::collections::BTreeSet::new();
        for t in [&self.dialog_trigger, &self.objective_trigger, &self.pickup] {
            if !t.validate() || !ids.insert(t.original) {
                return Err("Invalid grenade pickup trigger".into());
            }
        }
        if !(1..=100).contains(&self.maximum_ammo)
            || self.initial_ammo > self.maximum_ammo
            || !(1..=100).contains(&self.pickup_amount)
            || !self.respawn_seconds.is_finite()
            || !(0. ..=120.).contains(&self.respawn_seconds)
            || [
                self.blink_seconds,
                self.prompt_delay,
                self.next_dialog_delay,
            ]
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=120.).contains(v))
            || self.cues.len() != 4
        {
            return Err("Invalid grenade pickup assets".into());
        }
        let mut sources = std::collections::BTreeSet::new();
        for cue in &self.cues {
            if cue.source.is_empty()
                || cue.source.len() > 256
                || !sources.insert(&cue.source)
                || !cue.seconds.is_finite()
                || !(0.001..=120.).contains(&cue.seconds)
            {
                return Err("Invalid grenade pickup sound".into());
            }
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
pub enum IntroStage {
    Waiting,
    FirstLine,
    SecondLine,
    Done,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
pub enum PickupStage {
    Waiting,
    Confirmation,
    PromptDelay,
    NextDialogDelay,
    Done,
}
#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
pub enum Effect {
    Event(String),
    Tracker(String),
    PlaySound(String),
    ObjectiveCompleted(u32),
    InventoryChanged { before: u32, after: u32 },
    BlinkHud(f64),
}
#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
pub struct State {
    pub intro: IntroStage,
    pub pickup_script: PickupStage,
    pub intro_remaining: f64,
    pub pickup_remaining: f64,
    pub respawn_remaining: f64,
    pub ammunition: u32,
    pub objective_completed: bool,
    pub reminder_disabled: bool,
    pub dialog_touching: bool,
    pub objective_touching: bool,
}
impl State {
    pub fn new(a: &Assets) -> Result<Self, String> {
        a.validate()?;
        Ok(Self {
            intro: IntroStage::Waiting,
            pickup_script: PickupStage::Waiting,
            intro_remaining: 0.,
            pickup_remaining: 0.,
            respawn_remaining: 0.,
            ammunition: a.initial_ammo,
            objective_completed: false,
            reminder_disabled: false,
            dialog_touching: false,
            objective_touching: false,
        })
    }
    /// Independent original contact cylinders. The taller Trigger34 completes the
    /// objective independently of Ammo.HandlePickupQuery; do not merge these events.
    /// Contacts are sampled at the start of the interval; timers consume its duration.
    /// Only a live player with the thermal inventory can receive this selected pickup.
    pub fn advance(
        &mut self,
        a: &Assets,
        seconds: f64,
        position: Point,
        extent: Point,
        live: bool,
        has_thermal_inventory: bool,
    ) -> Result<Vec<Effect>, String> {
        if !seconds.is_finite()
            || !(0. ..=1.).contains(&seconds)
            || position.iter().any(|v| !v.is_finite() || v.abs() > 1e8)
            || extent
                .iter()
                .any(|v| !v.is_finite() || !(0. ..=10000.).contains(v))
        {
            return Err("Invalid grenade pickup advance".into());
        }
        let mut effects = vec![];
        let dialog = live && a.dialog_trigger.contains(position, extent);
        let objective = live && a.objective_trigger.contains(position, extent);
        if dialog && !self.dialog_touching && self.intro == IntroStage::Waiting {
            effects.extend([
                Effect::Event("DialogGunnerPosition".into()),
                Effect::Tracker("ThermalDetonatorBox01".into()),
                Effect::PlaySound(a.cues[0].source.clone()),
            ]);
            self.intro = IntroStage::FirstLine;
            self.intro_remaining = a.cues[0].seconds;
        }
        // Ammunition.HandlePickupQuery caps the grant; a full inventory neither
        // announces nor sleeps. Pickup.Sleeping.CheckTouching allows refills while touching.
        let cooldown = self.respawn_remaining;
        self.respawn_remaining = (cooldown - seconds).max(0.);
        if live
            && has_thermal_inventory
            && cooldown <= seconds + 1e-9
            && a.pickup.contains(position, extent)
            && self.ammunition < a.maximum_ammo
        {
            let before = self.ammunition;
            self.ammunition = self
                .ammunition
                .saturating_add(a.pickup_amount)
                .min(a.maximum_ammo);
            self.respawn_remaining = (a.respawn_seconds - (seconds - cooldown).max(0.)).max(0.);
            effects.extend([
                Effect::InventoryChanged {
                    before,
                    after: self.ammunition,
                },
                Effect::PlaySound(a.cues[3].source.clone()),
            ]);
        }
        if objective && !self.objective_touching && self.pickup_script == PickupStage::Waiting {
            self.objective_completed = true;
            self.reminder_disabled = true;
            self.pickup_script = PickupStage::Confirmation;
            self.pickup_remaining = a.cues[2].seconds;
            effects.extend([
                Effect::Event("PickedUpGrenades".into()),
                Effect::ObjectiveCompleted(2),
                Effect::Event("MissionObjectives".into()),
                Effect::Tracker("GunnerTarget".into()),
                Effect::Event("TurnOffPickUpGrenadeReminder".into()),
                Effect::PlaySound(a.cues[2].source.clone()),
            ]);
        }
        self.dialog_touching = dialog;
        self.objective_touching = objective;
        let mut delta = seconds;
        while matches!(self.intro, IntroStage::FirstLine | IntroStage::SecondLine) {
            if delta + 1e-9 < self.intro_remaining {
                self.intro_remaining -= delta;
                break;
            }
            delta = (delta - self.intro_remaining).max(0.);
            if self.intro == IntroStage::FirstLine {
                self.intro = IntroStage::SecondLine;
                self.intro_remaining = a.cues[1].seconds;
                effects.push(Effect::PlaySound(a.cues[1].source.clone()));
            } else {
                self.intro = IntroStage::Done;
                self.intro_remaining = 0.;
                effects.push(Effect::Event("MissionObjectives".into()));
            }
        }
        let mut delta = seconds;
        while !matches!(self.pickup_script, PickupStage::Waiting | PickupStage::Done) {
            if delta + 1e-9 < self.pickup_remaining {
                self.pickup_remaining -= delta;
                break;
            }
            delta = (delta - self.pickup_remaining).max(0.);
            match self.pickup_script {
                PickupStage::Confirmation => {
                    self.pickup_script = PickupStage::PromptDelay;
                    self.pickup_remaining = a.prompt_delay;
                    effects.push(Effect::BlinkHud(a.blink_seconds));
                }
                PickupStage::PromptDelay => {
                    self.pickup_script = PickupStage::NextDialogDelay;
                    self.pickup_remaining = a.next_dialog_delay;
                    effects.push(Effect::Event("PopUpGrenades".into()));
                }
                PickupStage::NextDialogDelay => {
                    self.pickup_script = PickupStage::Done;
                    self.pickup_remaining = 0.;
                    effects.push(Effect::Event("DialogBlowUpGunnerDialog".into()));
                }
                _ => unreachable!(),
            }
        }
        Ok(effects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn assets() -> Assets {
        let trigger = |original, position, height| Trigger {
            original,
            position,
            radius: 45.,
            height,
        };
        Assets {
            provenance: serde_json::Value::Null,
            dialog_trigger: trigger(746, [1000., 0., 0.], 40.),
            objective_trigger: trigger(694, [0.; 3], 100.),
            pickup: trigger(610, [0.; 3], 44.),
            initial_ammo: 0,
            maximum_ammo: 5,
            pickup_amount: 5,
            respawn_seconds: 0.1,
            cues: (0..4)
                .map(|i| Cue {
                    source: format!("Sound.{i}"),
                    seconds: 0.25,
                })
                .collect(),
            blink_seconds: 10.,
            prompt_delay: 0.5,
            next_dialog_delay: 2.,
        }
    }
    fn tick(s: &mut State, a: &Assets, dt: f64, p: Point) -> Vec<Effect> {
        s.advance(a, dt, p, [22., 22., 60.], true, true).unwrap()
    }
    #[test]
    fn pickup_and_script_are_distinct_and_once_only() {
        let a = assets();
        let mut s = State::new(&a).unwrap();
        // In Trigger34's vertical reach but outside the actual inventory cylinder.
        let e = tick(&mut s, &a, 0., [0., 0., 150.]);
        assert!(e.contains(&Effect::ObjectiveCompleted(2)));
        assert_eq!(s.ammunition, 0);
        assert!(s.reminder_disabled);
        let e = tick(&mut s, &a, 0., [0.; 3]);
        assert!(e.contains(&Effect::InventoryChanged {
            before: 0,
            after: 5
        }));
        assert!(!e.contains(&Effect::ObjectiveCompleted(2)));
        for _ in 0..20 {
            assert!(!tick(&mut s, &a, 0.25, [0.; 3]).iter().any(|e| matches!(
                e,
                Effect::ObjectiveCompleted(_) | Effect::InventoryChanged { .. }
            )));
        }
        assert_eq!(s.pickup_script, PickupStage::Done);
        tick(&mut s, &a, 0., [5000.; 3]);
        assert!(tick(&mut s, &a, 0., [0.; 3]).is_empty());
    }
    #[test]
    fn original_delays_and_independent_script_order() {
        let a = assets();
        let mut s = State::new(&a).unwrap();
        let e = tick(&mut s, &a, 0., [1000., 0., 0.]);
        assert_eq!(
            e,
            vec![
                Effect::Event("DialogGunnerPosition".into()),
                Effect::Tracker("ThermalDetonatorBox01".into()),
                Effect::PlaySound("Sound.0".into())
            ]
        );
        let e = tick(&mut s, &a, 0.25, [0.; 3]);
        assert_eq!(
            e.iter()
                .filter(|e| matches!(e, Effect::PlaySound(_)))
                .count(),
            3
        );
        assert_eq!(s.intro, IntroStage::SecondLine);
        assert_eq!(s.pickup_script, PickupStage::PromptDelay);
        assert!(e.contains(&Effect::BlinkHud(10.)));
        assert!(!e.contains(&Effect::Event("PopUpGrenades".into())));
        let e = tick(&mut s, &a, 0.5, [5000.; 3]);
        assert_eq!(
            e,
            vec![
                Effect::Event("MissionObjectives".into()),
                Effect::Event("PopUpGrenades".into())
            ]
        );
        assert!(tick(&mut s, &a, 1., [5000.; 3]).is_empty());
        assert_eq!(
            tick(&mut s, &a, 1., [5000.; 3]),
            vec![Effect::Event("DialogBlowUpGunnerDialog".into())]
        );
    }
    #[test]
    fn partitioned_waits_produce_same_events() {
        let a = assets();
        let run = |step: f64| {
            let mut s = State::new(&a).unwrap();
            let mut e = tick(&mut s, &a, 0., [0.; 3]);
            for _ in 0..(4. / step).round() as usize {
                e.extend(tick(&mut s, &a, step, [5000.; 3]));
            }
            (s, e)
        };
        let (a, e) = run(1.);
        let (b, f) = run(1. / 120.);
        assert_eq!(e, f);
        assert_eq!(a, b);
    }
    #[test]
    fn live_inventory_cap_and_respawn_rules() {
        let a = assets();
        let mut s = State::new(&a).unwrap();
        assert!(s
            .advance(&a, 0., [0.; 3], [22., 22., 60.], false, true)
            .unwrap()
            .is_empty());
        let e = s
            .advance(&a, 0., [0.; 3], [22., 22., 60.], true, false)
            .unwrap();
        assert!(e.contains(&Effect::ObjectiveCompleted(2)));
        assert_eq!(s.ammunition, 0);
        tick(&mut s, &a, 0., [0.; 3]);
        assert_eq!(s.ammunition, 5);
        // A later inventory consumer may spend ammo; pickup cannot refill during Sleep.
        s.ammunition = 3;
        assert!(tick(&mut s, &a, 0.05, [0.; 3]).is_empty());
        assert_eq!(s.ammunition, 3);
        let e = tick(&mut s, &a, 0.05, [0.; 3]);
        assert!(e.contains(&Effect::InventoryChanged {
            before: 3,
            after: 5
        }));
        assert_eq!(s.ammunition, 5);
        assert!((s.respawn_remaining - 0.1).abs() < 1e-8);
    }
    #[test]
    fn invalid_input_does_not_mutate_state() {
        let a = assets();
        let mut s = State::new(&a).unwrap();
        let before = s.clone();
        for dt in [-1., 1.1, f64::NAN, f64::INFINITY] {
            assert!(s.advance(&a, dt, [0.; 3], [22.; 3], true, true).is_err());
            assert_eq!(s, before);
        }
        assert!(s
            .advance(&a, 0.1, [f64::NAN; 3], [22.; 3], true, true)
            .is_err());
        assert_eq!(s, before);
        assert!(s.advance(&a, 0.1, [0.; 3], [-1.; 3], true, true).is_err());
        assert_eq!(s, before);
    }
    #[test]
    fn invalid_assets_rejected_before_state_exists() {
        let mut a = assets();
        a.objective_trigger.original = a.pickup.original;
        assert!(State::new(&a).is_err());
        let mut a = assets();
        a.cues.pop();
        assert!(State::new(&a).is_err());
        let mut a = assets();
        a.cues[0].seconds = f64::NAN;
        assert!(State::new(&a).is_err());
        let mut a = assets();
        a.initial_ammo = 6;
        assert!(State::new(&a).is_err());
        let mut a = assets();
        a.pickup.height = 0.;
        assert!(State::new(&a).is_err());
    }
}
