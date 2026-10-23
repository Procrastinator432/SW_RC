//! Selected DC15Ammo/CTAmmoRecharging script path. The existing aiming and
//! hitscan adapters remain separate from energy accounting and trigger requests.
use crate::{Game, State as GameState};
use serde::{Deserialize, Serialize};

/// Selected human-player modes from Weapon.EFireMode. Burst and animation-driven
/// modes are intentionally unsupported rather than silently treated as automatic.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FireMode {
    SemiAuto,
    Automatic,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub provenance: serde_json::Value,
    pub maximum: u32,
    pub recharge_rate: f32,
    pub cooldown_time: f32,
    pub wait_for_recharge: bool,
    pub damage: u32,
    pub shot_delay: f32,
    /// Absent in historical scenes: retain their held-trigger adapter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fire_mode: Option<FireMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<crate::weapon_animation::Assets>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub charge: f32,
    pub clock: f32,
    pub cooldown_started: f32,
    pub no_recharge: bool,
    pub cool_and_recharge: bool,
    pub ticking: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub trigger_down: bool,
}
impl Assets {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(a) = &self.animation {
            a.validate()?;
        }
        if self.maximum == 0
            || self.maximum > crate::MAGAZINE
            || !self.recharge_rate.is_finite()
            || !(0.001..=100.).contains(&self.recharge_rate)
            || !self.cooldown_time.is_finite()
            || !(0. ..=60.).contains(&self.cooldown_time)
            || !self.shot_delay.is_finite()
            || !(0.02..=10.).contains(&self.shot_delay)
            || self.damage == 0
            || self.damage > 10000
        {
            return Err("Invalid DC15 energy profile".into());
        }
        Ok(())
    }
    pub fn initial(&self) -> State {
        State {
            charge: self.maximum as f32,
            clock: 0.,
            cooldown_started: 0.,
            no_recharge: false,
            cool_and_recharge: false,
            ticking: true,
            trigger_down: false,
        }
    }
}
impl State {
    /// Consume the request before weapon locks, cooldown and energy gates. A
    /// rejected semi-auto press is not queued until the weapon becomes ready.
    pub fn trigger_request(&mut self, a: &Assets, down: bool) -> bool {
        let request = down && (a.fire_mode != Some(FireMode::SemiAuto) || !self.trigger_down);
        // Keep historical checkpoint bytes and replay fingerprints unchanged.
        self.trigger_down = a.fire_mode.is_some() && down;
        request
    }
    pub fn validate(&self, a: &Assets) -> Result<(), String> {
        if !self.charge.is_finite()
            || !(0. ..=a.maximum as f32).contains(&self.charge)
            || !self.clock.is_finite()
            || !(0. ..=1e8).contains(&self.clock)
            || !self.cooldown_started.is_finite()
            || !(0. ..=self.clock).contains(&self.cooldown_started)
            || (!self.ticking && self.charge != a.maximum as f32)
        {
            return Err("Invalid DC15 charge/clock state".into());
        }
        Ok(())
    }
    pub fn can_fire(&self, a: &Assets) -> bool {
        self.charge >= 1. && (!self.cool_and_recharge || !a.wait_for_recharge)
    }
    pub fn use_charge(&mut self, a: &Assets) -> bool {
        if !self.can_fire(a) {
            return false;
        }
        self.charge = (self.charge - 1.).clamp(0., a.maximum as f32);
        self.ticking = true;
        if self.charge <= 0. {
            self.cool_and_recharge = true;
            self.cooldown_started = self.clock;
        }
        true
    }
    pub fn tick(&mut self, a: &Assets, dt: f32) {
        self.clock += dt;
        if !self.ticking {
            return;
        }
        // DC15Ammo.Tick, followed by CTAmmoRecharging.Tick, in script order.
        if !self.cool_and_recharge && a.wait_for_recharge {
            if self.charge >= 1. {
                self.no_recharge = true;
            }
        } else if self.clock - self.cooldown_started >= a.cooldown_time {
            self.no_recharge = false;
        }
        if self.charge == a.maximum as f32 {
            self.cool_and_recharge = false;
        }
        if !self.no_recharge {
            self.charge = (self.charge + a.recharge_rate * dt).clamp(0., a.maximum as f32);
            if self.charge >= a.maximum as f32 {
                self.ticking = false;
            }
        }
    }
}
pub(crate) fn validate_state(a: Option<&Assets>, s: &GameState) -> Result<(), String> {
    match (a, s.dc15.as_ref()) {
        (None, None) => Ok(()),
        (Some(a), Some(e)) => {
            e.validate(a)?;
            if s.ammo != e.charge.floor() as u32 || s.reserve != 0 || s.reload_remaining != 0. {
                return Err("DC15 energy and displayed ammunition disagree".into());
            }
            if !s.fire_remaining.is_finite()
                || !(0. ..=f64::from(a.shot_delay)).contains(&s.fire_remaining)
            {
                return Err("Invalid DC15 shot timer".into());
            }
            Ok(())
        }
        _ => Err("DC15 checkpoint module mismatch".into()),
    }
}
impl Game {
    pub(crate) fn dc15_player(&self, mut value: serde_json::Value) -> serde_json::Value {
        if let Some(s) = &self.state.dc15 {
            value["dc15"] = serde_json::to_value(s).expect("finite energy state");
            value["dc15_fire_remaining"] = self.state.fire_remaining.into();
        }
        value
    }
    pub(crate) fn dc15_handoff(
        &self,
        player: &serde_json::Value,
        ammo: u32,
        reserve: u32,
    ) -> Result<(Option<State>, f64), String> {
        match (&self.dc15, player.get("dc15")) {
            (None, None) => Ok((None, 0.)),
            (None, Some(_)) => {
                Err("DC15 handoff requires the matching destination scene package".into())
            }
            (Some(a), None) => {
                // Older handoffs contain integer ammunition only. Preserve its
                // immediately available shots within the new capacity.
                let mut s = a.initial();
                s.charge = ammo.min(a.maximum) as f32;
                Ok((Some(s), 0.))
            }
            (Some(a), Some(v)) => {
                let s: State = serde_json::from_value(v.clone()).map_err(|e| e.to_string())?;
                s.validate(a)?;
                let remaining = player["dc15_fire_remaining"]
                    .as_f64()
                    .filter(|v| v.is_finite() && (0. ..=f64::from(a.shot_delay)).contains(v))
                    .ok_or("Invalid DC15 handoff fire timer")?;
                if ammo != s.charge.floor() as u32 || reserve != 0 {
                    return Err("DC15 handoff ammunition mismatch".into());
                }
                Ok((Some(s), remaining))
            }
        }
    }
    pub fn attach_dc15(&mut self, a: Assets) -> Result<(), String> {
        a.validate()?;
        if self.dc15.is_some() || self.combat.is_some() || self.state.statistics.ticks != 0 {
            return Err("DC15 energy profile requires a fresh fallback-weapon scene".into());
        }
        if let Some(v) = &mut self.visuals {
            if let Some(profile) = &a.animation {
                profile.validate_mesh(&v.weapon)?;
            }
            let state = self
                .state
                .visuals
                .as_mut()
                .ok_or("Missing weapon visuals")?;
            v.weapon_animation = a.animation.is_some();
            v.weapon_sound_groups = a
                .animation
                .as_ref()
                .and_then(|a| a.foley.as_ref())
                .map_or(0, |f| f.groups.len());
            state.weapon_animation = v
                .weapon_animation
                .then(|| crate::weapon_animation::State::new(v.weapon_sound_groups));
        }
        self.state.dc15 = Some(a.initial());
        self.dc15 = Some(a);
        self.sync_dc15();
        self.initial = self.state.clone();
        Ok(())
    }
    pub(crate) fn sync_dc15(&mut self) {
        if let Some(s) = &self.state.dc15 {
            self.state.ammo = s.charge.floor() as u32;
            self.state.reserve = 0;
            self.state.reload_remaining = 0.;
        }
    }
    pub(crate) fn dc15_tick(&mut self) {
        if let (Some(a), Some(s)) = (&self.dc15, &mut self.state.dc15) {
            s.tick(a, crate::STEP as f32);
            self.state.fire_remaining = (self.state.fire_remaining - crate::STEP).max(0.);
            if self.state.fire_remaining < 1e-8 {
                self.state.fire_remaining = 0.;
            }
            self.sync_dc15();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn assets() -> Assets {
        Assets {
            provenance: serde_json::Value::Null,
            maximum: 5,
            recharge_rate: 1.5,
            cooldown_time: 0.,
            wait_for_recharge: false,
            damage: 55,
            shot_delay: 0.2,
            fire_mode: None,
            animation: None,
        }
    }
    #[test]
    fn five_original_charges_empty_gate_and_partial_recharge() {
        let a = assets();
        let mut s = a.initial();
        for _ in 0..5 {
            assert!(s.use_charge(&a));
        }
        assert_eq!(s.charge, 0.);
        assert!(s.cool_and_recharge);
        assert!(!s.use_charge(&a));
        for _ in 0..39 {
            s.tick(&a, crate::STEP as f32);
        }
        assert!(!s.can_fire(&a));
        for _ in 0..2 {
            s.tick(&a, crate::STEP as f32);
        }
        assert!(s.can_fire(&a));
        assert!(s.use_charge(&a));
        assert!(s.charge < 0.1);
        s.validate(&a).unwrap();
    }
    #[test]
    fn original_tick_order_cooldown_gate_full_clamp_and_disabled_tick() {
        let mut a = assets();
        a.cooldown_time = 1.;
        let mut s = a.initial();
        s.no_recharge = true;
        for _ in 0..5 {
            s.use_charge(&a);
        }
        s.tick(&a, 0.5);
        assert_eq!(s.charge, 0.);
        assert!(s.no_recharge);
        s.tick(&a, 0.5);
        assert_eq!(s.charge, 0.75);
        assert!(!s.no_recharge);
        for _ in 0..10 {
            s.tick(&a, 0.5);
        }
        assert_eq!(s.charge, 5.);
        assert!(!s.ticking);
        let charge = s.charge;
        s.tick(&a, 1.);
        assert_eq!(s.charge, charge);
    }
    #[test]
    fn wait_for_recharge_fire_gate_and_bad_energy_states() {
        let mut a = assets();
        a.wait_for_recharge = true;
        let mut s = a.initial();
        s.cool_and_recharge = true;
        assert!(!s.can_fire(&a));
        s.cool_and_recharge = false;
        assert!(s.can_fire(&a));
        s.tick(&a, 0.1);
        assert!(s.no_recharge);
        s.charge = f32::NAN;
        assert!(s.validate(&a).is_err());
        s = a.initial();
        s.cooldown_started = 1.;
        assert!(s.validate(&a).is_err());
        s = a.initial();
        s.charge = 6.;
        assert!(s.validate(&a).is_err());
    }
}
