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
    /// Absent in old scenes. Grenade notifies remain owned by throw_runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foley: Option<Foley>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Foley {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<rc_package::audio_params::Params>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<rc_package::sound_multiple::Group>,
    pub sounds: Vec<crate::combat::Sound>,
    pub notifies: Vec<SoundNotify>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoundNotify {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<usize>,
    pub clip: String,
    pub original: i32,
    pub time: f32,
    pub sound: usize,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<Vec<AudioRecord>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<SelectionState>,
    pub idle_started: Option<u64>,
    pub playing_idle: bool,
    pub fire: Option<Player>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionState {
    pub seed: u32,
    pub groups: Vec<rc_package::sound_multiple::State>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioRecord {
    pub sequence: u64,
    pub tick: u64,
    pub sound: usize,
    pub playback: rc_package::audio_params::Playback,
}
impl State {
    pub fn new(groups: usize, audio: bool) -> Self {
        Self {
            audio: audio.then(Vec::new),
            selection: (groups > 0).then(|| SelectionState {
                seed: 0x52434631,
                groups: vec![Default::default(); groups],
            }),
            ..Default::default()
        }
    }
}
impl Assets {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(foley) = &self.foley {
            if foley.sounds.is_empty()
                || foley.sounds.len() > 16
                || foley.groups.len() > 8
                || foley.notifies.len() > 32
            {
                return Err("Invalid weapon Foley budget".into());
            }
            if !foley.params.is_empty() {
                if foley.params.len() != foley.sounds.len() || foley.groups.is_empty() {
                    return Err("Weapon parameter bank mismatch".into());
                }
                for p in &foley.params {
                    p.validate()?;
                }
            }
            for group in &foley.groups {
                group.validate(foley.sounds.len())?;
            }
            for sound in &foley.sounds {
                crate::combat::validate_wave(&sound.wave)?;
            }
            for (i, n) in foley.notifies.iter().enumerate() {
                if !["Run", "Idle"].contains(&n.clip.as_str())
                    || n.original <= 0
                    || !n.time.is_finite()
                    || !(0. ..1.).contains(&n.time)
                    || n.sound >= foley.sounds.len()
                    || n.group.is_some_and(|g| g >= foley.groups.len())
                    || foley.notifies[..i].iter().any(|p| p.original == n.original)
                {
                    return Err("Invalid weapon Foley notify".into());
                }
            }
        }
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
        if let Some(foley) = &self.foley {
            for name in ["Run", "Idle"] {
                let original = &mesh.mesh.clips[mesh.clip(name)?].sequence.notifies;
                let bound: Vec<_> = foley.notifies.iter().filter(|n| n.clip == name).collect();
                if original.len() != bound.len()
                    || original.iter().zip(bound).any(|(a, b)| {
                        a.object_index != b.original || a.time_bits != b.time.to_bits()
                    })
                {
                    return Err("Weapon Foley differs from original clip notifies".into());
                }
            }
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
        let groups = a.foley.as_ref().map_or(&[][..], |f| f.groups.as_slice());
        match &self.selection {
            None if groups.is_empty() => (),
            Some(s) if !groups.is_empty() && s.groups.len() == groups.len() => {
                for (state, group) in s.groups.iter().zip(groups) {
                    state.validate(group)?;
                }
            }
            _ => return Err("Weapon sound selection checkpoint mismatch".into()),
        }
        let foley = a.foley.as_ref();
        match (&self.audio, foley.is_some_and(|f| !f.params.is_empty())) {
            (None, false) => (),
            (Some(records), true) => {
                if records.len() > 256 || records.windows(2).any(|r| r[0].sequence >= r[1].sequence)
                {
                    return Err("Invalid weapon audio record order".into());
                }
                for r in records {
                    if r.sequence == 0 || r.tick > tick || r.sound >= foley.unwrap().sounds.len() {
                        return Err("Invalid weapon audio record".into());
                    }
                    r.playback.validate()?;
                    let p = &foley.unwrap().params[r.sound];
                    if r.playback.radius.to_bits() != p.radius.to_bits()
                        || r.playback.category != p.category
                        || r.playback.volume < p.min_volume.min(1.)
                        || r.playback.volume > p.max_volume.min(1.)
                        || r.playback.pitch < p.min_pitch.clamp(0.5, 2.)
                        || r.playback.pitch > p.max_pitch.clamp(0.5, 2.)
                    {
                        return Err("Saved playback differs from its original profile".into());
                    }
                }
            }
            _ => return Err("Weapon audio checkpoint mismatch".into()),
        }
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
    ) -> Result<Vec<usize>, String> {
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
            return Ok(vec![]);
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
        let interval = base.advance(mesh, false, None)?;
        let mut sounds = vec![];
        if let Some(foley) = &a.foley {
            let clip = &mesh.mesh.clips[base.clip].sequence.name.name;
            for n in &foley.notifies {
                if n.clip.eq_ignore_ascii_case(clip)
                    && crate::visual::crossed_notify(interval.0, interval.1, n.time)
                {
                    let selected = if let Some(index) = n.group {
                        let state = self
                            .selection
                            .as_mut()
                            .ok_or("Missing weapon sound selection")?;
                        let seed = &mut state.seed;
                        state
                            .groups
                            .get_mut(index)
                            .ok_or("Missing sound group state")?
                            .select(&foley.groups[index], 0, 0, || {
                                rc_package::sound_multiple::random(seed)
                            })
                    } else {
                        Some(n.sound)
                    };
                    if let Some(sound) = selected {
                        if foley.params.is_empty() {
                            sounds.push(sound);
                        } else {
                            let seed = &mut self
                                .selection
                                .as_mut()
                                .ok_or("Missing audio random stream")?
                                .seed;
                            if let Some(playback) = foley.params[sound].select(seed) {
                                self.audio
                                    .as_mut()
                                    .ok_or("Missing weapon audio state")?
                                    .push(AudioRecord {
                                        sequence: 0,
                                        tick: now.statistics.ticks,
                                        sound,
                                        playback,
                                    });
                                sounds.push(sound);
                            }
                        }
                    }
                }
            }
        }
        if base.finished {
            self.playing_idle = false;
        }
        Ok(sounds)
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
    if profile.is_some_and(|a| a.foley.is_some()) {
        let count = game.audio_bank().len();
        let events = s
            .mission
            .as_ref()
            .map(|m| m.events.as_slice())
            .or_else(|| s.map.as_ref().map(|m| m.events.as_slice()))
            .unwrap_or(&[]);
        if count > 128 || events.len() > 256 || events.iter().any(|e| e.sound >= count) {
            return Err("Invalid weapon/campaign audio checkpoint".into());
        }
    }
    if let Some(records) = s
        .visuals
        .as_ref()
        .and_then(|v| v.weapon_animation.as_ref())
        .and_then(|w| w.audio.as_ref())
    {
        let events = s
            .mission
            .as_ref()
            .map(|m| m.events.as_slice())
            .or_else(|| s.map.as_ref().map(|m| m.events.as_slice()))
            .unwrap_or(&[]);
        let base = game.audio_bank().len() - game.weapon_foley_sounds().len();
        if records.iter().any(|r| {
            r.sound >= game.weapon_foley_sounds().len()
                || !events.iter().any(|e| {
                    e.sequence == r.sequence
                        && e.tick == r.tick
                        && e.sound == base + r.sound
                        && e.owner == -1
                })
        }) {
            return Err("Weapon playback record has no matching event".into());
        }
        if profile
            .and_then(|a| a.foley.as_ref())
            .is_some_and(|f| !f.params.is_empty())
            && events
                .iter()
                .any(|e| e.sound >= base && !records.iter().any(|r| r.sequence == e.sequence))
        {
            return Err("Missing saved weapon playback parameters".into());
        }
    }
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
    /// Older campaign producers sometimes append several cues without a cap.
    /// New Foley scenes share the existing most-recent-256 event policy after
    /// every tick, including cinematic/defeat/win early returns. Legacy pairs
    /// retain their exact historical queue and checkpoint behavior.
    pub(crate) fn trim_weapon_audio(&mut self) {
        if self.weapon_foley_sounds().is_empty() {
            return;
        }
        let events = if let Some(m) = &mut self.state.mission {
            &mut m.events
        } else if let Some(m) = &mut self.state.map {
            &mut m.events
        } else {
            return;
        };
        let excess = events.len().saturating_sub(256);
        events.drain(..excess);
        let live: Vec<_> = events.iter().map(|e| e.sequence).collect();
        if let Some(records) = self
            .state
            .visuals
            .as_mut()
            .and_then(|v| v.weapon_animation.as_mut())
            .and_then(|w| w.audio.as_mut())
        {
            records.retain(|r| live.contains(&r.sequence));
        }
    }
    pub fn weapon_foley_sounds(&self) -> &[crate::combat::Sound] {
        self.dc15
            .as_ref()
            .and_then(|a| a.animation.as_ref())
            .and_then(|a| a.foley.as_ref())
            .map_or(&[], |f| f.sounds.as_slice())
    }
    /// Append-only bank extension: existing mission/voice/weapon roles stay stable.
    pub fn audio_bank(&self) -> Vec<&crate::combat::Sound> {
        let base = self
            .map
            .as_ref()
            .map(|m| m.assets.sounds.as_slice())
            .or_else(|| self.mission.as_ref().map(|m| m.sounds.as_slice()))
            .or_else(|| self.combat.as_ref().map(|c| c.sounds.as_slice()))
            .unwrap_or(&[]);
        base.iter().chain(self.weapon_foley_sounds()).collect()
    }
    pub fn emit_weapon_foley(&mut self, events: &[(usize, i32)]) -> Result<(), String> {
        if events.is_empty() {
            return Ok(());
        }
        let count = self.weapon_foley_sounds().len();
        let bank = self.audio_bank().len();
        if bank > 128
            || events
                .iter()
                .any(|(sound, owner)| *sound >= count || *owner != -1)
        {
            return Err("Invalid weapon Foley event/bank".into());
        }
        let pending = self
            .state
            .visuals
            .as_ref()
            .and_then(|v| v.weapon_animation.as_ref())
            .and_then(|w| w.audio.as_ref());
        if let Some(records) = pending {
            let pending: Vec<_> = records.iter().filter(|r| r.sequence == 0).collect();
            if pending.len() != events.len()
                || pending.iter().zip(events).any(|(r, (sound, _))| {
                    r.sound != *sound || r.tick != self.state.statistics.ticks
                })
            {
                return Err("Weapon playback dispatch mismatch".into());
            }
        }
        let mut emitted = Vec::new();
        let tick = self.state.statistics.ticks;
        let (sequence, queue) = if let Some(m) = &mut self.state.mission {
            (&mut m.audio_sequence, &mut m.events)
        } else if let Some(m) = &mut self.state.map {
            (&mut m.audio_sequence, &mut m.events)
        } else {
            return Err("Weapon Foley requires campaign audio state".into());
        };
        for &(sound, owner) in events {
            *sequence += 1;
            emitted.push(*sequence);
            let excess = queue.len().saturating_sub(255);
            queue.drain(..excess);
            queue.push(crate::combat::Event {
                sequence: *sequence,
                tick,
                sound: bank - count + sound,
                owner,
            });
        }
        if let Some(records) = self
            .state
            .visuals
            .as_mut()
            .and_then(|v| v.weapon_animation.as_mut())
            .and_then(|w| w.audio.as_mut())
        {
            for (r, sequence) in records.iter_mut().filter(|r| r.sequence == 0).zip(emitted) {
                r.sequence = sequence;
            }
        }
        Ok(())
    }
    pub fn clear_weapon_playback(&mut self) {
        if let Some(records) = self
            .state
            .visuals
            .as_mut()
            .and_then(|v| v.weapon_animation.as_mut())
            .and_then(|w| w.audio.as_mut())
        {
            records.clear();
        }
    }
    pub fn weapon_playback(&self, sequence: u64) -> Option<&rc_package::audio_params::Playback> {
        self.state
            .visuals
            .as_ref()?
            .weapon_animation
            .as_ref()?
            .audio
            .as_ref()?
            .iter()
            .find(|r| r.sequence == sequence)
            .map(|r| &r.playback)
    }
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
        result.map(|_| ())
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
