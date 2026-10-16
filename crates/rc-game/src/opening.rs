//! Selected geo_01a landing briefing/injury lanes. Not a general script VM.
use crate::{Game, STEP};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Op {
    Wait {
        seconds: f64,
    },
    Sound {
        role: usize,
        caption_seconds: f64,
        seconds: f64,
        text: String,
    },
    Event {
        tag: String,
    },
    Injury {
        damage: f32,
    },
    Prompt {
        text: String,
        seconds: f64,
    },
    Blink {
        seconds: f64,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Action {
    pub original: u32,
    pub op: Op,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Program {
    pub original: u32,
    pub trigger: String,
    pub actions: Vec<Action>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bacta: Option<crate::bacta::Assets>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sound_policies: Vec<SoundPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enemy_fire_source: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sound_bindings: Vec<SoundBinding>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stop_at_transition: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intro: Option<crate::intro::Assets>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rappel: Option<crate::rappel::Assets>,
    pub gunship: Option<crate::visual::Mesh>,
    pub programs: Vec<Program>,
    pub sounds: Vec<crate::combat::Sound>,
    pub initial_health: f32,
    pub camera_target: [f64; 3],
    pub camera_origin: [f64; 3],
    pub explosion: [f64; 3],
    pub provenance: serde_json::Value,
}
/// Identity of a cue before later import stages extend the sound bank.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoundBinding {
    pub role: usize,
    pub source: String,
    pub pcm: String,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoundPolicy {
    pub role: usize,
    pub owner: i32,
    pub category: u8,
    pub no_override: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OwnedVoice {
    pub owner: i32,
    pub until: f64,
    pub protected: bool,
}
/// UAudioSubsystem::FindSource 103d0d10 compares IDs excluding bit0, then
/// rejects the incoming source if the EXISTING matching source has bit0 set.
fn admit_voice(active: &mut Vec<OwnedVoice>, policy: SoundPolicy, now: f64, duration: f64) -> bool {
    active.retain(|v| v.until > now + 1e-10);
    if policy.category != 1 {
        return true;
    } // Music/SFX use generated unique IDs.
    if active
        .iter()
        .any(|v| v.owner == policy.owner && v.protected)
    {
        return false;
    }
    active.retain(|v| v.owner != policy.owner);
    active.push(OwnedVoice {
        owner: policy.owner,
        until: now + duration,
        protected: policy.no_override,
    });
    true
}
#[derive(Clone, Debug, Default, Serialize, serde::Deserialize)]
pub struct Lane {
    pub started: bool,
    pub next: usize,
    pub remaining: f64,
    pub finished: bool,
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct State {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bacta: Option<crate::bacta::State>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owned_voices: Vec<OwnedVoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intro: Option<crate::intro::State>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rappel: Option<crate::rappel::State>,
    pub clock: f64,
    pub lanes: Vec<Lane>,
    pub done: bool,
    pub locked: bool,
    pub injured: bool,
    pub gunship_idle: Option<f64>,
    pub captions: Vec<(String, f64)>,
    pub subtitle: String,
    pub subtitle_until: f64,
    pub prompt: String,
    pub prompt_until: f64,
    pub blink_until: f64,
    pub events: Vec<(u64, u32, String)>,
    pub ready_tick: Option<u64>,
}
pub struct Prepared {
    pub bacta: Option<crate::bacta::Prepared>,
    pub intro: Option<crate::intro::Prepared>,
    pub rappel: Option<crate::rappel::Prepared>,
    pub assets: Assets,
    pub sound_base: usize,
}
impl Assets {
    pub fn sound_policy(&self, role: usize, owner: i32) -> Option<SoundPolicy> {
        self.sound_policies
            .iter()
            .find(|p| p.role == role && p.owner == owner)
            .copied()
    }
    pub fn sound_roles(&self) -> Vec<usize> {
        let mut roles = Vec::new();
        for p in &self.programs {
            for a in &p.actions {
                if let Op::Sound { role, .. } = a.op {
                    roles.push(role);
                }
            }
        }
        if let Some(i) = &self.intro {
            roles.extend(i.sounds);
        }
        if let Some(r) = &self.rappel {
            roles.extend(r.notifies.iter().filter_map(|n| n.role));
        }
        if let Some(b) = &self.bacta {
            roles.extend(b.notifies.iter().map(|n| n.role));
        }
        roles.sort_unstable();
        roles.dedup();
        roles
    }
    fn bind_sounds(&mut self, base: &[crate::combat::Sound]) -> Result<(), String> {
        if self.sound_bindings.len() > 128
            || self
                .sound_bindings
                .iter()
                .any(|b| b.role >= 128 || b.source.len() > 512 || b.pcm.len() != 16)
        {
            return Err("Invalid opening cue identity manifest".into());
        }
        if self.sound_bindings.is_empty() {
            return Ok(()); // Legacy scene packages have no identity manifest.
        }
        let bank: Vec<_> = base.iter().chain(self.sounds.iter()).collect();
        let mut mapping = std::collections::BTreeMap::new();
        for b in &self.sound_bindings {
            let matches = |s: &&crate::combat::Sound| {
                s.source == b.source && crate::replay::fingerprint(&s.wave) == b.pcm
            };
            let role = bank
                .get(b.role)
                .filter(|s| matches(s))
                .map(|_| b.role)
                .or_else(|| bank.iter().position(matches))
                .ok_or_else(|| format!("Opening cue identity missing: {}", b.source))?;
            if mapping.insert(b.role, role).is_some() {
                return Err("Duplicate opening cue identity".into());
            }
        }
        if self.sound_roles().iter().any(|r| !mapping.contains_key(r)) {
            return Err("Incomplete opening cue identities".into());
        }
        for p in &mut self.programs {
            for a in &mut p.actions {
                if let Op::Sound { role, .. } = &mut a.op {
                    *role = mapping[role];
                }
            }
        }
        for p in &mut self.sound_policies {
            p.role = *mapping.get(&p.role).ok_or("Missing owned sound identity")?;
        }
        if let Some(b) = &mut self.bacta {
            for n in &mut b.notifies {
                n.role = mapping[&n.role];
            }
        }
        if let Some(i) = &mut self.intro {
            for role in &mut i.sounds {
                *role = mapping[role];
            }
        }
        if let Some(r) = &mut self.rappel {
            for role in r.notifies.iter_mut().filter_map(|n| n.role.as_mut()) {
                *role = mapping[role];
            }
        }
        Ok(())
    }
    pub fn validate(&self, base: usize) -> Result<(), String> {
        if self.sound_policies.len() > 128
            || self
                .sound_policies
                .iter()
                .any(|p| p.role >= base + self.sounds.len() || p.owner < 1 || p.category > 2)
        {
            return Err("Invalid opening owned sound policy".into());
        }
        let unique: std::collections::BTreeSet<_> =
            self.sound_policies.iter().map(|p| p.role).collect();
        if unique.len() != self.sound_policies.len() {
            return Err("Duplicate opening sound policy".into());
        }
        if let Some(r) = &self.rappel {
            r.validate(base + self.sounds.len())?;
        }
        let mut scripts: Vec<_> = self.programs.iter().map(|p| p.original).collect();
        scripts.sort_unstable();
        if scripts != [12, 497, 523]
            || self.programs.len() != 3
            || self.sounds.len() > 32
            || !(176. ..=1000.).contains(&self.initial_health)
            || self
                .explosion
                .iter()
                .chain(self.camera_target.iter())
                .chain(self.camera_origin.iter())
                .any(|v| !v.is_finite() || v.abs() > 1e8)
        {
            return Err("Invalid opening assets".into());
        }
        for p in &self.programs {
            if ![12, 523, 497].contains(&p.original)
                || p.actions.is_empty()
                || p.actions.len() > 48
                || p.trigger.len() > 64
            {
                return Err("Invalid opening program".into());
            }
            for a in &p.actions {
                match &a.op {
                    Op::Wait { seconds }
                    | Op::Prompt { seconds, .. }
                    | Op::Blink { seconds }
                    | Op::Sound { seconds, .. }
                        if !seconds.is_finite() || !(0. ..=120.).contains(seconds) =>
                    {
                        return Err("Invalid opening time".into())
                    }
                    Op::Sound {
                        caption_seconds, ..
                    } if !caption_seconds.is_finite()
                        || !(0. ..=120.).contains(caption_seconds) =>
                    {
                        return Err("Invalid caption duration".into())
                    }
                    Op::Sound { role, text, .. }
                        if *role >= base + self.sounds.len() || text.len() > 512 =>
                    {
                        return Err("Invalid opening sound".into())
                    }
                    Op::Injury { damage }
                        if !damage.is_finite() || !(1. ..self.initial_health).contains(damage) =>
                    {
                        return Err("Invalid opening damage".into())
                    }
                    Op::Event { tag } if tag.is_empty() || tag.len() > 128 => {
                        return Err("Invalid opening event".into())
                    }
                    Op::Prompt { text, .. } if text.len() > 256 => {
                        return Err("Invalid opening prompt".into())
                    }
                    _ => {}
                }
            }
        }
        for sound in &self.sounds {
            crate::combat::validate_wave(&sound.wave)?;
        }
        Ok(())
    }
}
impl State {
    fn new(a: &Assets) -> Self {
        Self {
            bacta: None,
            owned_voices: vec![],
            intro: None,
            rappel: a.rappel.as_ref().map(|_| crate::rappel::State::default()),
            clock: 0.,
            lanes: (0..a.programs.len()).map(|_| Lane::default()).collect(),
            done: false,
            locked: true,
            injured: false,
            gunship_idle: None,
            captions: vec![],
            subtitle: String::new(),
            subtitle_until: 0.,
            prompt: String::new(),
            prompt_until: 0.,
            blink_until: 0.,
            events: vec![],
            ready_tick: None,
        }
    }
    fn signal(&mut self, a: &Assets, tag: &str) {
        for (l, p) in self.lanes.iter_mut().zip(&a.programs) {
            if p.trigger == tag {
                l.started = true;
            }
        }
    }
}
impl Game {
    pub fn attach_opening(&mut self, mut a: Assets) -> Result<(), String> {
        if self.opening.is_some() || self.tactics.is_none() {
            return Err("Opening requires selected mission/squad scene".into());
        }
        let base = self.mission.as_ref().unwrap().sounds.len();
        a.bind_sounds(&self.mission.as_ref().unwrap().sounds)?;
        a.validate(base)?;
        let enemy_fire = a
            .enemy_fire_source
            .as_ref()
            .map(|source| {
                self.mission
                    .as_ref()
                    .unwrap()
                    .sounds
                    .iter()
                    .chain(a.sounds.iter())
                    .position(|s| &s.source == source)
                    .ok_or("Missing opening enemy fire cue")
            })
            .transpose()?;
        let mesh = a
            .gunship
            .take()
            .ok_or("Missing opening gunship")?
            .prepare_reference()?;
        for name in [
            "WarriorIntroGunshipBreathe",
            "WarriorIntroGunship",
            "WarriorIntroGunshipLoop",
            "GunshipAmbushSpiderdroid",
            "GunshipAmbushSpiderdroidLoop",
        ] {
            mesh.clip(name)?;
        }
        let texture_base = self.scene.textures.len();
        self.scene
            .textures
            .extend(mesh.mesh.materials.iter().map(|m| rc_render::Texture {
                width: m.width,
                height: m.height,
                pixels: m.pixels.clone(),
            }));
        let p = self.artillery.as_mut().unwrap();
        p.actors[0] = mesh;
        p.texture_bases[0] = texture_base;
        self.mission
            .as_mut()
            .unwrap()
            .sounds
            .extend(a.sounds.iter().map(|s| crate::combat::Sound {
                source: s.source.clone(),
                wave: s.wave.clone(),
            }));
        if let Some(role) = enemy_fire {
            self.mission.as_mut().unwrap().enemy_fire_sound = Some(role);
        }
        self.state.health = a.initial_health.ceil() as u32;
        self.state.mission.as_mut().unwrap().health = a.initial_health;
        self.state.mission.as_mut().unwrap().objective_mode = 0;
        let mut s = State::new(&a);
        let bacta = if let Some(b) = a.bacta.take() {
            s.bacta = Some(crate::bacta::State::default());
            Some(b.prepare(&mut self.scene, base + a.sounds.len())?)
        } else {
            None
        };
        let rappel = if let Some(r) = a.rappel.take() {
            if self
                .world
                .body_placement(r.start, self.profile.physics.extent)?
                .state
                == rc_package::movement::PlacementState::Penetrating
            {
                return Err("Rappel start penetrates world".into());
            }
            self.state.pawn.body.position = r.start;
            self.state.pawn.body.velocity = [0.; 3];
            self.state.yaw = r.rotation[1] as f64 * std::f64::consts::TAU / 65536.;
            self.state.pitch = r.rotation[0] as f64 * std::f64::consts::TAU / 65536.;
            Some(r.prepare(&mut self.scene)?)
        } else {
            None
        };
        let intro = if let Some(assets) = a.intro.take() {
            let (prepared, state) = assets.prepare(&mut self.scene, base + a.sounds.len())?;
            s.intro = Some(state);
            Some(prepared)
        } else {
            None
        };
        s.signal(&a, "levelstart");
        self.state.opening = Some(s);
        self.opening = Some(Prepared {
            bacta,
            intro,
            rappel,
            assets: a,
            sound_base: base,
        });
        self.initial = self.state.clone();
        Ok(())
    }
    pub(crate) fn opening_tick(&mut self) -> Result<(), String> {
        let Some(p) = &self.opening else {
            return Ok(());
        };
        let a = &p.assets;
        let mut s = self.state.opening.take().unwrap();
        s.owned_voices.retain(|v| v.until > s.clock + 1e-10);
        let tick = self.state.statistics.ticks + 1;
        let result = (|| -> Result<(), String> {
            let mut budget = STEP;
            for _ in 0..128 {
                let mut progressed = false;
                for i in 0..s.lanes.len() {
                    while s.lanes[i].started
                        && !s.lanes[i].finished
                        && s.lanes[i].remaining <= 1e-10
                    {
                        progressed = true;
                        if s.lanes[i].next == a.programs[i].actions.len() {
                            s.lanes[i].finished = true;
                            break;
                        }
                        let action = &a.programs[i].actions[s.lanes[i].next];
                        s.lanes[i].next += 1;
                        let label = match &action.op {
                            Op::Wait { seconds } => {
                                s.lanes[i].remaining = *seconds;
                                "Wait".into()
                            }
                            Op::Sound {
                                role,
                                seconds,
                                caption_seconds,
                                text,
                            } => {
                                let policy =
                                    a.sound_policies.iter().find(|p| p.role == *role).copied();
                                s.lanes[i].remaining = *seconds;
                                let admitted = policy.is_none_or(|policy| {
                                    admit_voice(
                                        &mut s.owned_voices,
                                        policy,
                                        s.clock,
                                        *caption_seconds,
                                    )
                                });
                                if !admitted {
                                    s.events.push((
                                        tick,
                                        action.original,
                                        format!("Sound{role}BlockedByOwnedVoice"),
                                    ));
                                    continue;
                                }
                                let m = self.state.mission.as_mut().unwrap();
                                m.audio_sequence += 1;
                                if m.events.len() >= 256 {
                                    m.events.remove(0);
                                }
                                m.events.push(crate::combat::Event {
                                    sequence: m.audio_sequence,
                                    tick,
                                    sound: *role,
                                    owner: policy.map_or(-1, |p| p.owner),
                                });
                                s.lanes[i].remaining = *seconds;
                                s.subtitle = text.clone();
                                s.subtitle_until = s.clock + caption_seconds;
                                s.captions.retain(|(_, until)| *until > s.clock);
                                if s.captions.len() >= 8 {
                                    s.captions.remove(0);
                                }
                                s.captions.push((text.clone(), s.clock + caption_seconds));
                                format!("Sound{role}")
                            }
                            Op::Event { tag } => {
                                s.signal(a, tag);
                                match tag.as_str() {
                                    "PlayerRappel" => {
                                        if let Some(r) = &mut s.rappel {
                                            r.started = true;
                                        }
                                    }
                                    "DestroyCloneRope07" => {
                                        if let Some(r) = &mut s.rappel {
                                            r.rope_destroyed = true;
                                        }
                                    }
                                    "EndBriefing" => s.locked = false,
                                    "GunshipIdle" => s.gunship_idle = Some(s.clock),
                                    "MissionObjectives" => {
                                        self.state.mission.as_mut().unwrap().objective_mode = 1;
                                    }
                                    "Conversation06" => {
                                        s.done = true;
                                        s.ready_tick = Some(tick);
                                    }
                                    "GunshipDoorExplosion" => {
                                        self.state.bunker.as_mut().unwrap().explosions.push(
                                            crate::gunner_runtime::Explosion {
                                                position: a.explosion,
                                                age: 0.,
                                                bunker: false,
                                            },
                                        )
                                    }
                                    _ => {}
                                }
                                tag.clone()
                            }
                            Op::Injury { damage } => {
                                self.state.health =
                                    self.state.health.saturating_sub(damage.ceil() as u32);
                                self.state.mission.as_mut().unwrap().health =
                                    self.state.health as f32;
                                self.state.damage_flash = 0.2;
                                s.injured = true;
                                self.state.bunker.as_mut().unwrap().explosions.push(
                                    crate::gunner_runtime::Explosion {
                                        position: self.state.pawn.body.position,
                                        age: 0.,
                                        bunker: false,
                                    },
                                );
                                "ScriptedInjury".into()
                            }
                            Op::Prompt { text, seconds } => {
                                s.prompt = text.clone();
                                s.prompt_until = s.clock + seconds;
                                "Prompt".into()
                            }
                            Op::Blink { seconds } => {
                                s.blink_until = s.clock + seconds;
                                "BlinkHealth".into()
                            }
                        };
                        if s.events.len() >= 256 {
                            s.events.remove(0);
                        }
                        s.events.push((tick, action.original, label));
                    }
                }
                if progressed {
                    continue;
                }
                if budget <= 1e-10 {
                    break;
                }
                let step = s
                    .lanes
                    .iter()
                    .filter(|l| l.started && !l.finished && l.remaining > 0.)
                    .map(|l| l.remaining)
                    .fold(budget, f64::min);
                for l in &mut s.lanes {
                    if l.started && !l.finished {
                        l.remaining = (l.remaining - step).max(0.);
                    }
                }
                s.clock += step;
                budget -= step;
            }
            if budget > 1e-10 {
                return Err("Opening event budget exceeded".into());
            }
            Ok(())
        })();
        self.state.opening = Some(s);
        result
    }
    pub(crate) fn opening_geometry(
        &self,
    ) -> Result<
        (
            Vec<rc_package::geometry::Triangle>,
            Vec<Option<rc_render::FaceTexture>>,
        ),
        String,
    > {
        let Some(s) = &self.state.opening else {
            return Ok((vec![], vec![]));
        };
        let a = self.artillery.as_ref().unwrap();
        if self.state.artillery.as_ref().unwrap().attack_tick.is_some() {
            return Ok((vec![], vec![]));
        }
        let mesh = &a.actors[0];
        let (name, frame, delta) = if let Some(start) = s.gunship_idle {
            let clip = mesh.clip("WarriorIntroGunship")?;
            let seq = &mesh.mesh.clips[clip].sequence;
            let duration = (seq.frames - 1) as f64 / f32::from_bits(seq.rate_bits) as f64;
            let elapsed = s.clock - start;
            if elapsed < duration {
                let frame =
                    (elapsed * f32::from_bits(seq.rate_bits) as f64 / seq.frames as f64) as f32;
                (
                    "WarriorIntroGunship",
                    frame,
                    mesh.cinematic_root_delta(clip, frame)?,
                )
            } else {
                let c = mesh.clip("WarriorIntroGunshipLoop")?;
                let loop_seq = &mesh.mesh.clips[c].sequence;
                let f = ((elapsed - duration) * f32::from_bits(loop_seq.rate_bits) as f64
                    / loop_seq.frames as f64)
                    .rem_euclid(1.) as f32;
                let end = mesh.cinematic_root_delta(clip, 1. - 1. / seq.frames as f32)?;
                let now = mesh.cinematic_root_delta(c, f)?;
                (
                    "WarriorIntroGunshipLoop",
                    f,
                    std::array::from_fn(|i| end[i] + now[i]),
                )
            }
        } else {
            let c = mesh.clip("WarriorIntroGunshipBreathe")?;
            let seq = &mesh.mesh.clips[c].sequence;
            let f = (s.clock * f32::from_bits(seq.rate_bits) as f64 / seq.frames as f64)
                .rem_euclid(1.) as f32;
            (
                "WarriorIntroGunshipBreathe",
                f,
                mesh.cinematic_root_delta(c, f)?,
            )
        };
        let mut scene = mesh.geometry_with_root(&mesh.channel(mesh.clip(name)?, frame)?, false)?;
        let actor = &a.assets.actors[0];
        for t in &mut scene.triangles {
            t.points = t.points.map(|v| {
                rc_package::mesh::transform_point(
                    std::array::from_fn(|i| v[i] + delta[i]),
                    actor.location,
                    actor.rotation,
                    [1.; 3],
                    [0.; 3],
                )
            });
        }
        for f in scene.appearance.iter_mut().flatten() {
            f.texture += a.texture_bases[0];
        }
        Ok((scene.triangles, scene.appearance))
    }
    pub(crate) fn opening_status(&self) -> Option<String> {
        let s = self.state.opening.as_ref()?;
        (!s.done).then(|| {
            format!(
                "Landungsbriefing · {}",
                if s.subtitle.is_empty() {
                    "Auftrag empfangen"
                } else {
                    &s.subtitle
                }
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protected_existing_voice_rejects_same_owner_but_not_other_owners_or_sfx() {
        let mut voices = vec![];
        let p = SoundPolicy {
            role: 0,
            owner: 523,
            category: 1,
            no_override: true,
        };
        assert!(admit_voice(&mut voices, p, 0., 3.));
        assert!(!admit_voice(
            &mut voices,
            SoundPolicy {
                no_override: false,
                ..p
            },
            0.,
            2.6
        ));
        assert_eq!(voices[0].until, 3.);
        assert!(admit_voice(
            &mut voices,
            SoundPolicy { owner: 530, ..p },
            0.,
            1.
        ));
        assert!(admit_voice(
            &mut voices,
            SoundPolicy { category: 2, ..p },
            0.,
            1.
        ));
        assert!(admit_voice(&mut voices, p, 3., 1.));
    }
    #[test]
    fn unprotected_voice_is_replaced_not_queued_and_protection_belongs_to_existing_source() {
        let mut voices = vec![];
        let p = SoundPolicy {
            role: 0,
            owner: 523,
            category: 1,
            no_override: false,
        };
        assert!(admit_voice(&mut voices, p, 0., 3.));
        assert!(admit_voice(
            &mut voices,
            SoundPolicy {
                no_override: true,
                ..p
            },
            1.,
            1.
        ));
        assert_eq!(voices.len(), 1);
        assert_eq!(voices[0].until, 2.);
        assert!(voices[0].protected);
        let restored: Vec<OwnedVoice> =
            serde_json::from_slice(&serde_json::to_vec(&voices).unwrap()).unwrap();
        assert!(!admit_voice(&mut restored.clone(), p, 1.5, 3.));
        assert!(admit_voice(&mut voices, p, 2., 1.));
    }
    fn assets() -> Assets {
        Assets {
            bacta: None,
            sound_policies: vec![],
            enemy_fire_source: None,
            sound_bindings: vec![],
            stop_at_transition: false,
            intro: None,
            rappel: None,
            gunship: None,
            programs: [
                (12, "levelstart"),
                (523, "Rappel"),
                (497, "StartBactaTraining"),
            ]
            .into_iter()
            .map(|(original, trigger)| Program {
                original,
                trigger: trigger.into(),
                actions: vec![Action {
                    original: original + 1000,
                    op: Op::Wait { seconds: 1. },
                }],
            })
            .collect(),
            sounds: vec![],
            initial_health: 200.,
            camera_target: [0.; 3],
            camera_origin: [0.; 3],
            explosion: [0.; 3],
            provenance: serde_json::Value::Null,
        }
    }
    #[test]
    fn rejects_duplicate_scripts_nan_timers_and_missing_sound_roles() {
        let mut a = assets();
        a.validate(65).unwrap();
        a.programs[0].original = 497;
        assert!(a.validate(65).is_err());
        let mut a = assets();
        a.programs[0].actions[0].op = Op::Wait { seconds: f64::NAN };
        assert!(a.validate(65).is_err());
        let mut a = assets();
        a.programs[0].actions[0].op = Op::Sound {
            role: 65,
            caption_seconds: 1.,
            seconds: 0.,
            text: String::new(),
        };
        assert!(a.validate(65).is_err());
    }
    #[test]
    fn cue_identities_survive_bank_insertion_and_reject_pcm_changes() {
        let sound = |source: &str, wave: &[u8]| crate::combat::Sound {
            source: source.into(),
            wave: wave.to_vec(),
        };
        let base = vec![sound("base0", &[0]), sound("base1", &[1])];
        let mut a = assets();
        a.sounds.push(sound("dialogue", &[2, 3, 4]));
        a.programs[0].actions = (0..3)
            .map(|role| Action {
                original: role as u32,
                op: Op::Sound {
                    role,
                    caption_seconds: 1.,
                    seconds: 1.,
                    text: String::new(),
                },
            })
            .collect();
        a.sound_bindings = base
            .iter()
            .chain(a.sounds.iter())
            .enumerate()
            .map(|(role, s)| SoundBinding {
                role,
                source: s.source.clone(),
                pcm: crate::replay::fingerprint(&s.wave),
            })
            .collect();
        let mut extended = base;
        extended.push(sound("enemy", &[9]));
        a.bind_sounds(&extended).unwrap();
        assert_eq!(a.sound_roles(), vec![0, 1, 3]);
        // A matching source name with different PCM must fail closed.
        a.programs[0].actions[2].op = Op::Sound {
            role: 2,
            caption_seconds: 1.,
            seconds: 1.,
            text: String::new(),
        };
        a.sounds[0].wave.push(5);
        assert!(a.bind_sounds(&extended).is_err());
    }
    #[test]
    fn signals_selected_lanes_without_restarting_them() {
        let a = assets();
        let mut s = State::new(&a);
        s.signal(&a, "levelstart");
        assert!(s.lanes[0].started);
        assert!(!s.lanes[1].started);
        s.lanes[0].next = 1;
        s.signal(&a, "levelstart");
        assert_eq!(s.lanes[0].next, 1);
        s.signal(&a, "Rappel");
        assert!(s.lanes[1].started);
        assert!(!s.lanes[2].started);
    }
}
