//! Selected player Weapon.GrenadeThrow gate. The host supplies the actual original
//! animation channel frames; release is an AnimNotify, not a delay from button press.
//! Fire, AltFire, ServerForceReload and ThrowGrenade are ignored while in this state.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub frames: i32,
    pub rate: f32,
    pub release_frame: f32,
}
impl Assets {
    pub fn validate(&self) -> Result<(), String> {
        if !(2..=10000).contains(&self.frames)
            || !self.rate.is_finite()
            || !(0. ..=1000.).contains(&self.rate)
            || self.rate == 0.
            || !self.release_frame.is_finite()
            || !(0. ..=1. - 1. / self.frames as f32).contains(&self.release_frame)
        {
            return Err("Invalid original grenade animation assets".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, serde::Deserialize)]
pub struct State {
    pub active: bool,
    pub released: bool,
    pub frame: f32,
    pub started: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    PlayThrowAnimation,
    EmptySound,
    ReleaseProjectile,
    ResumeWeapon,
}
impl State {
    /// Inventory stays intact until the release notify. Ammo type/zoom/HasAnim
    /// selection and the button edge are checked by the integrating host.
    pub fn begin(&mut self, ammunition: u32) -> Vec<Effect> {
        if self.active {
            return vec![];
        }
        if ammunition == 0 {
            return vec![Effect::EmptySound];
        }
        *self = Self {
            active: true,
            ..Self::default()
        };
        vec![Effect::PlayThrowAnimation]
    }
    /// Frames are normalized, nonlooping, monotonic values from channel zero.
    /// Inventory is consumed only once, if still available at the actual notify.
    pub fn animation(
        &mut self,
        assets: &Assets,
        next: f32,
        finished: bool,
        ammunition: &mut u32,
    ) -> Result<Vec<Effect>, String> {
        assets.validate()?;
        if !next.is_finite() || !(0. ..=1.).contains(&next) || (self.active && next < self.frame) {
            return Err("Invalid grenade animation frame".into());
        }
        if !self.active {
            return Ok(vec![]);
        }
        let old = if self.started {
            self.frame
        } else {
            -f32::EPSILON
        };
        self.frame = next;
        self.started = true;
        let mut effects = vec![];
        if !self.released && old < assets.release_frame && next >= assets.release_frame {
            self.released = true;
            if *ammunition > 0 {
                *ammunition -= 1;
                effects.push(Effect::ReleaseProjectile);
            }
        }
        if finished {
            self.active = false;
            effects.push(Effect::ResumeWeapon);
        }
        Ok(effects)
    }
    pub fn blocks_weapon(&self) -> bool {
        self.active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn assets() -> Assets {
        Assets {
            frames: 30,
            rate: 30.,
            release_frame: 0.5,
        }
    }
    #[test]
    fn inventory_is_spent_at_notify_not_press_and_gate_lasts_until_anim_end() {
        let mut s = State::default();
        let mut ammo = 5;
        assert_eq!(s.begin(ammo), [Effect::PlayThrowAnimation]);
        assert_eq!(ammo, 5);
        assert!(s.blocks_weapon());
        assert!(s.begin(ammo).is_empty());
        assert!(s
            .animation(&assets(), 0.49, false, &mut ammo)
            .unwrap()
            .is_empty());
        assert_eq!(
            s.animation(&assets(), 0.51, false, &mut ammo).unwrap(),
            [Effect::ReleaseProjectile]
        );
        assert_eq!(ammo, 4);
        assert!(s.blocks_weapon());
        assert_eq!(
            s.animation(&assets(), 0.96, true, &mut ammo).unwrap(),
            [Effect::ResumeWeapon]
        );
        assert!(!s.blocks_weapon());
    }
    #[test]
    fn held_notify_and_duplicate_end_do_not_release_again() {
        let mut s = State::default();
        let mut ammo = 5;
        s.begin(ammo);
        s.animation(&assets(), 0.5, false, &mut ammo).unwrap();
        assert!(s
            .animation(&assets(), 0.5, false, &mut ammo)
            .unwrap()
            .is_empty());
        s.animation(&assets(), 0.96, true, &mut ammo).unwrap();
        assert!(s
            .animation(&assets(), 0.96, true, &mut ammo)
            .unwrap()
            .is_empty());
        assert_eq!(ammo, 4);
    }
    #[test]
    fn empty_inventory_does_not_begin_and_disappearing_ammo_cannot_underflow() {
        let mut s = State::default();
        assert_eq!(s.begin(0), [Effect::EmptySound]);
        assert!(!s.blocks_weapon());
        s.begin(1);
        let mut ammo = 0;
        assert!(s
            .animation(&assets(), 0.5, false, &mut ammo)
            .unwrap()
            .is_empty());
        assert!(s.released);
        assert_eq!(ammo, 0);
    }
    #[test]
    fn invalid_frame_is_atomic_and_zero_notify_is_not_lost() {
        let mut s = State::default();
        let mut ammo = 1;
        s.begin(ammo);
        let before = s.clone();
        assert!(s.animation(&assets(), f32::NAN, false, &mut ammo).is_err());
        assert_eq!(s, before);
        let mut a = assets();
        a.release_frame = 0.;
        assert_eq!(
            s.animation(&a, 0., false, &mut ammo).unwrap(),
            [Effect::ReleaseProjectile]
        );
        assert_eq!(ammo, 0);
    }
}
