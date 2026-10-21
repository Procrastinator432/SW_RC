//! Selected non-zoom Weapon.PlayIdleAnim / PlayFiring / AnimEnd paths.
//! Tweening, pawn animations, network roles and the complete weapon state machine
//! remain separate; channels use the existing prepared native pose host.
use crate::visual::{Player, PreparedMesh};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub fire_anim: String,
    pub idle_delay: f32,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub idle_started: Option<u64>,
    pub playing_idle: bool,
    pub fire: Option<Player>,
}
impl Assets {
    pub fn validate(&self) -> Result<(), String> {
        if self.fire_anim.is_empty()
            || self.fire_anim.len() > 128
            || !self.idle_delay.is_finite()
            || !(0.1..=3600.).contains(&self.idle_delay)
            || [
                "ActionBreathe",
                "Walk",
                "Run",
                "Idle",
                "Reload",
                "ThrowGrenade",
            ]
            .iter()
            .any(|n| self.fire_anim.eq_ignore_ascii_case(n))
        {
            return Err("Invalid weapon animation profile".into());
        }
        Ok(())
    }
    pub fn validate_mesh(&self, mesh: &PreparedMesh) -> Result<(), String> {
        self.validate()?;
        for name in ["ActionBreathe", "Walk", "Run", "Idle", &self.fire_anim] {
            mesh.clip(name)?;
        }
        Ok(())
    }
}
/// Original 3D VelocitySq thresholds; translation between frames is not velocity.
pub fn locomotion(velocity: [f64; 3]) -> &'static str {
    let v = velocity.map(|v| v as f32);
    let speed_sq = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    if speed_sq >= 51076. {
        "Run"
    } else if speed_sq >= 0.04 {
        "Walk"
    } else {
        "ActionBreathe"
    }
}
impl State {
    pub fn validate(
        &self,
        a: &Assets,
        mesh: &PreparedMesh,
        base: &Player,
        tick: u64,
    ) -> Result<(), String> {
        validate_player(mesh, base)?;
        let name = &mesh.mesh.clips[base.clip].sequence.name.name;
        if self.idle_started.is_some_and(|t| t > tick)
            || ![
                "ActionBreathe",
                "Walk",
                "Run",
                "Idle",
                "Reload",
                "ThrowGrenade",
            ]
            .iter()
            .any(|n| name.eq_ignore_ascii_case(n))
            || self.playing_idle && (!name.eq_ignore_ascii_case("Idle") || base.finished)
        {
            return Err("Invalid weapon idle checkpoint".into());
        }
        if let Some(fire) = &self.fire {
            validate_player(mesh, fire)?;
            if fire.clip != mesh.clip(&a.fire_anim)? || !fire.started || self.idle_started.is_some()
            {
                return Err("Invalid weapon fire channel checkpoint".into());
            }
        }
        Ok(())
    }
    pub fn tick(
        &mut self,
        a: &Assets,
        mesh: &PreparedMesh,
        base: &mut Player,
        now: &crate::State,
        old: &crate::State,
        blocked: bool,
    ) -> Result<(), String> {
        let shot = now.statistics.shots > old.statistics.shots;
        let throwing = now.thermal.as_ref().is_some_and(|t| t.gate.active);
        let reload = now.reload_remaining > 0.;
        if throwing || reload || blocked {
            self.fire = None;
            self.idle_started = None;
            self.playing_idle = false;
            let clip = if throwing {
                "ThrowGrenade"
            } else if reload {
                "Reload"
            } else {
                "ActionBreathe"
            };
            base.select(
                mesh,
                clip,
                now.statistics.reloads > old.statistics.reloads || base.finished,
            )?;
            base.advance(mesh, false, None)?;
            return Ok(());
        }
        if self.fire.as_ref().is_some_and(|p| p.finished) {
            // NormalFire.AnimEnd(channel 1) -> selected human ServerThink idle
            // branch, EndState clears channel 1. Never clear the shot deadline.
            self.fire = None;
            self.idle_started = None;
        }
        if shot {
            self.fire = Some(Player::new(mesh, &a.fire_anim)?);
            self.idle_started = None;
        }
        if let Some(fire) = &mut self.fire {
            fire.advance(mesh, false, None)?;
        } else {
            let stationary = now.pawn.body.velocity.iter().all(|&v| v as f32 == 0.);
            let mut fidget = false;
            if stationary {
                let started = *self.idle_started.get_or_insert(now.statistics.ticks);
                if (now.statistics.ticks - started) as f64 * crate::STEP >= f64::from(a.idle_delay)
                {
                    base.select(mesh, "Idle", true)?;
                    self.playing_idle = true;
                    self.idle_started = Some(now.statistics.ticks);
                    fidget = true;
                }
            } else {
                self.idle_started = None;
            }
            if !fidget {
                let clip = locomotion(now.pawn.body.velocity);
                if clip != "ActionBreathe" || !self.playing_idle {
                    base.select(mesh, clip, base.finished)?;
                    self.playing_idle = false;
                }
            }
        }
        // Channel 0 keeps its own phase while channel 1 fires.
        base.advance(mesh, false, None)?;
        if base.finished {
            self.playing_idle = false;
        }
        Ok(())
    }
}
fn validate_player(mesh: &PreparedMesh, p: &Player) -> Result<(), String> {
    let clip = mesh
        .mesh
        .clips
        .get(p.clip)
        .ok_or("Invalid weapon animation clip")?;
    let last = 1. - 1. / clip.sequence.frames as f32;
    if !p.frame.is_finite()
        || !(0. ..=last).contains(&p.frame)
        || !p.started && (p.frame != 0. || p.finished)
        || p.started && p.finished != (p.frame >= last)
    {
        return Err("Invalid weapon animation frame/flags".into());
    }
    Ok(())
}
pub(crate) fn validate_game(game: &crate::Game, s: &crate::State) -> Result<(), String> {
    let profile = game.dc15.as_ref().and_then(|a| a.animation.as_ref());
    let state = s.visuals.as_ref().and_then(|v| v.weapon_animation.as_ref());
    match (profile, state) {
        (None, None) => Ok(()),
        (Some(a), Some(state)) => state.validate(
            a,
            &game.visuals.as_ref().ok_or("Missing weapon assets")?.weapon,
            &s.visuals.as_ref().ok_or("Missing weapon state")?.weapon,
            s.statistics.ticks,
        ),
        _ => Err("Weapon animation checkpoint module mismatch".into()),
    }
}

impl crate::Game {
    /// Keep the new weapon actor path alive at player-control early returns.
    /// Hidden/locked hands must not resume a stale firing pose after a cinematic.
    pub(crate) fn locked_weapon_animation_tick(&mut self) -> Result<(), String> {
        let Some(a) = self.dc15.as_ref().and_then(|a| a.animation.as_ref()) else {
            return Ok(());
        };
        let assets = self
            .visuals
            .as_ref()
            .ok_or("Missing locked weapon assets")?;
        let mut visual = self
            .state
            .visuals
            .take()
            .ok_or("Missing locked weapon state")?;
        let result = visual
            .weapon_animation
            .as_mut()
            .ok_or("Missing locked weapon animation")?
            .tick(
                a,
                &assets.weapon,
                &mut visual.weapon,
                &self.state,
                &self.state,
                true,
            );
        self.state.visuals = Some(visual);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_velocity_boundaries_include_vertical_motion() {
        assert_eq!(locomotion([0.; 3]), "ActionBreathe");
        assert_eq!(locomotion([0.199, 0., 0.]), "ActionBreathe");
        assert_eq!(locomotion([0.2, 0., 0.]), "Walk");
        assert_eq!(locomotion([225.99, 0., 0.]), "Walk");
        assert_eq!(locomotion([226., 0., 0.]), "Run");
        assert_eq!(locomotion([0., 0., -226.]), "Run");
        assert_eq!(locomotion([160., 160., 0.]), "Run");
    }
}
