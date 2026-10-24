//! Selected continuous world sources. Playback parameters are chosen once per
//! simulation activation; querying or rebuilding the Android host never draws.
use crate::{Game, State};
use rc_package::audio_params::{Params, Playback};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub owner: i32,
    pub sound: usize,
    pub params: Params,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Voice {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<crate::ambient_geometry::Voice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade: Option<rc_package::ambient_lifecycle::Fade>,
    pub owner: i32,
    pub sound: usize,
    pub started: u64,
    pub playback: Option<Playback>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listener_region: Option<crate::ambient_geometry::CachedRegion>,
    pub seed: u32,
    pub voices: Vec<Voice>,
}
impl Default for AudioState {
    fn default() -> Self {
        Self {
            listener_region: None,
            seed: 0x52434131,
            voices: vec![],
        }
    }
}
pub struct Source {
    pub activation: u64,
    pub owner: i32,
    pub sound: usize,
    pub location: [f32; 3],
    pub playback: Option<Playback>,
    pub flags: u32,
}
pub fn validate_profiles(a: &crate::opening::Assets, sounds: usize) -> Result<(), String> {
    if let Some(g) = &a.ambient_geometry {
        g.validate()?;
        if a.ambient_policy.is_none()
            || g.actors.len() != a.ambient_profiles.len()
            || a.ambient_profiles
                .iter()
                .any(|p| !g.actors.iter().any(|a| a.owner == p.owner))
        {
            return Err("Audio geometry/profile module mismatch".into());
        }
    }
    if let Some(p) = &a.ambient_policy {
        p.validate()?;
        if a.ambient_profiles.is_empty() {
            return Err("Ambient lifecycle without profiles".into());
        }
    }
    if a.ambient_profiles.len() > 16 {
        return Err("Too many spatial ambient profiles".into());
    }
    let mut owners = std::collections::BTreeSet::new();
    for p in &a.ambient_profiles {
        p.params.validate()?;
        let bound = if p.owner == 736 {
            a.bacta.as_ref().and_then(|b| b.ambient_loop) == Some(p.sound)
        } else {
            a.world_effects
                .iter()
                .any(|b| b.original as i32 == p.owner && b.ambient == Some(p.sound))
        };
        if p.owner < 1
            || !owners.insert(p.owner)
            || p.sound >= sounds
            || p.params.category != 2
            || !bound
        {
            return Err("Invalid spatial ambient profile binding".into());
        }
    }
    Ok(())
}
pub fn validate_state(game: &Game, s: &State) -> Result<(), String> {
    let profiles = game
        .opening
        .as_ref()
        .map_or(&[][..], |p| p.assets.ambient_profiles.as_slice());
    let audio = s.opening.as_ref().and_then(|o| o.ambient_audio.as_ref());
    if audio.is_some() == profiles.is_empty() {
        return Err("Ambient audio checkpoint module mismatch".into());
    }
    if let Some(audio) = audio {
        let geometry = game
            .opening
            .as_ref()
            .and_then(|o| o.assets.ambient_geometry.as_ref());
        if let Some(cache) = &audio.listener_region {
            cache.validate(geometry.ok_or("Unexpected saved audio listener region")?)?;
        }
        if geometry.is_some() && !audio.voices.is_empty() && audio.listener_region.is_none() {
            return Err("Missing saved audio listener region".into());
        }
        let mut owners = std::collections::BTreeSet::new();
        if audio.voices.len() > profiles.len() {
            return Err("Too many saved ambient voices".into());
        }
        for v in &audio.voices {
            let policy = game
                .opening
                .as_ref()
                .and_then(|o| o.assets.ambient_policy.as_ref());
            if policy.is_some() {
                if v.priority
                    .is_none_or(|p| !p.is_finite() || !(0.01..=1.).contains(&p))
                {
                    return Err("Invalid saved ambient admission priority".into());
                }
                if let Some(f) = &v.fade {
                    f.validate(v.started, s.statistics.ticks)?;
                }
            } else if v.priority.is_some() || v.fade.is_some() {
                return Err("Unexpected ambient lifecycle state".into());
            }
            let p = profiles
                .iter()
                .find(|p| p.owner == v.owner && p.sound == v.sound)
                .ok_or("Unknown saved ambient voice")?;
            if !owners.insert(v.owner) || v.started > s.statistics.ticks {
                return Err("Invalid ambient voice identity".into());
            }
            match (&v.geometry, geometry) {
                (Some(v), Some(g)) => v.validate(
                    g,
                    p.params.radius,
                    (s.statistics.ticks as f64 * crate::STEP) as f32,
                )?,
                (None, None) => (),
                _ => return Err("Saved audio geometry module mismatch".into()),
            }
            if let Some(pb) = &v.playback {
                pb.validate()?;
                if pb.radius.to_bits() != p.params.radius.to_bits()
                    || pb.category != p.params.category
                    || pb.volume < p.params.min_volume.min(1.)
                    || pb.volume > p.params.max_volume.min(1.)
                    || pb.pitch < p.params.min_pitch.clamp(0.5, 2.)
                    || pb.pitch > p.params.max_pitch.clamp(0.5, 2.)
                {
                    return Err("Saved ambient parameters differ from original profile".into());
                }
            } else if p.params.min_volume > 0.0001 {
                return Err("Audible ambient voice saved as muted".into());
            }
        }
        if s.phase == crate::Phase::Playing {
            for p in profiles {
                let mut required = if p.owner == 736 {
                    s.health > 0
                        && !s.squad.as_ref().is_some_and(|s| s.player_down)
                        && s.mission.as_ref().is_some_and(|m| m.using && !m.completed)
                        && s.opening
                            .as_ref()
                            .and_then(|o| o.bacta.as_ref())
                            .is_some_and(|b| b.phase == crate::bacta::Phase::Active)
                } else {
                    s.opening
                        .as_ref()
                        .and_then(|o| o.world_effects.as_ref())
                        .is_some_and(|w| w.live.iter().any(|b| b.original as i32 == p.owner))
                };
                if required && (p.owner != 736 || geometry.is_some()) {
                    if let Some(policy) = game
                        .opening
                        .as_ref()
                        .and_then(|o| o.assets.ambient_policy.as_ref())
                    {
                        let location = game.ambient_location_in(s, p.owner)?;
                        let mut listener = s.pawn.body.position;
                        listener[2] += if s.crouched {
                            56. * 0.65
                        } else {
                            game.profile.physics.extent[2] * 0.65
                        };
                        let listener = if geometry.is_some() {
                            audio
                                .listener_region
                                .as_ref()
                                .map_or(listener.map(|v| v as f32), |c| c.position())
                        } else {
                            listener.map(|v| v as f32)
                        };
                        let factor = if let Some(g) = geometry {
                            let actor = g
                                .actors
                                .iter()
                                .find(|a| a.owner == p.owner)
                                .ok_or("Missing audio geometry actor")?;
                            let (source, listener) = g.regions(location, listener)?;
                            g.query(
                                actor,
                                &source,
                                &listener,
                                p.params.non_positional || p.params.disable_occlusion,
                            )?
                            .factor()
                        } else {
                            1.
                        };
                        required = !s.opening.as_ref().is_some_and(|o| o.locked)
                            && policy
                                .distances(location, listener, p.params.radius, factor)?
                                .admit;
                    }
                }
                if required && !audio.voices.iter().any(|v| v.owner == p.owner) {
                    return Err("Active ambient voice missing from checkpoint".into());
                }
            }
        }
    }
    Ok(())
}
impl Game {
    pub(crate) fn tick_ambient_audio(&mut self) -> Result<(), String> {
        let desired = self.ambient_loops();
        if let Some(policy) = self
            .opening
            .as_ref()
            .and_then(|o| o.assets.ambient_policy.clone())
        {
            return self.tick_ambient_policy(&policy, desired);
        }
        let Some(opening) = &self.opening else {
            return Ok(());
        };
        if opening.assets.ambient_profiles.is_empty() {
            return Ok(());
        }
        let state = self
            .state
            .opening
            .as_mut()
            .and_then(|o| o.ambient_audio.as_mut())
            .ok_or("Missing ambient audio state")?;
        state
            .voices
            .retain(|v| desired.contains(&(v.sound, v.owner)));
        for p in &opening.assets.ambient_profiles {
            if desired.contains(&(p.sound, p.owner))
                && !state.voices.iter().any(|v| v.owner == p.owner)
            {
                state.voices.push(Voice {
                    geometry: None,
                    priority: None,
                    fade: None,
                    owner: p.owner,
                    sound: p.sound,
                    started: self.state.statistics.ticks,
                    playback: p.params.select(&mut state.seed),
                });
            }
        }
        Ok(())
    }
    fn tick_ambient_policy(
        &mut self,
        policy: &rc_package::ambient_lifecycle::Policy,
        desired: Vec<(usize, i32)>,
    ) -> Result<(), String> {
        let opening = self.opening.as_ref().ok_or("Missing ambient opening")?;
        let listener = self.audio_listener().0;
        let geometry = opening.assets.ambient_geometry.as_ref();
        let audio = self
            .state
            .opening
            .as_ref()
            .and_then(|o| o.ambient_audio.as_ref())
            .ok_or("Missing ambient lifecycle state")?;
        let listener_region = geometry
            .map(|g| {
                crate::ambient_geometry::CachedRegion::resolve(
                    &g.bsp,
                    listener,
                    g.level_info,
                    audio.listener_region.as_ref(),
                )
            })
            .transpose()?;
        let tick = self.state.statistics.ticks;
        let now = (tick as f64 * crate::STEP) as f32;
        let mut candidates = vec![];
        for p in &opening.assets.ambient_profiles {
            if !desired.contains(&(p.sound, p.owner)) {
                continue;
            }
            let location = self.ambient_location(p.owner)?;
            let previous = audio
                .voices
                .iter()
                .find(|v| v.owner == p.owner)
                .and_then(|v| v.geometry.as_ref());
            let (voice_geometry, hearing, factor) = if let Some(g) = geometry {
                let source = crate::ambient_geometry::CachedRegion::resolve(
                    &g.bsp,
                    location,
                    g.level_info,
                    previous.map(|v| &v.source),
                )?;
                let actor = g
                    .actors
                    .iter()
                    .find(|a| a.owner == p.owner)
                    .ok_or("Missing geometric audio actor")?;
                let hearing = g.query(
                    actor,
                    &source,
                    listener_region.as_ref().unwrap(),
                    p.params.non_positional || p.params.disable_occlusion,
                )?;
                (
                    Some(crate::ambient_geometry::Voice::update(
                        source,
                        hearing,
                        previous,
                        p.params.radius,
                        now,
                    )?),
                    Some(hearing),
                    hearing.factor(),
                )
            } else {
                (None, None, 1.)
            };
            let distances = policy.distances(location, listener, p.params.radius, factor)?;
            let priority = rc_package::world_audio::Priority {
                location,
                listener,
                volume: p.params.max_volume,
                radius: p.params.radius,
                radius_multiplier: policy.radius_multiplier,
                flags: 2,
                slot: 3,
            }
            .admission()?;
            candidates.push((p, distances, priority, voice_geometry, hearing));
        }
        candidates.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.owner.cmp(&b.0.owner)));
        let state = self
            .state
            .opening
            .as_mut()
            .and_then(|o| o.ambient_audio.as_mut())
            .ok_or("Missing ambient lifecycle state")?;
        state.listener_region = listener_region;
        state.voices.retain_mut(|v| {
            let Some((_, d, _, geometry, _)) = candidates
                .iter()
                .find(|(p, _, _, _, _)| p.owner == v.owner && p.sound == v.sound)
            else {
                return false;
            };
            v.geometry = geometry.clone();
            if let Some(f) = &mut v.fade {
                return !f.step();
            }
            if d.release {
                v.fade = Some(rc_package::ambient_lifecycle::Fade::new(tick));
            }
            true
        });
        for (p, d, priority, geometry, hearing) in candidates {
            if d.admit && !state.voices.iter().any(|v| v.owner == p.owner) {
                // A fade may have ended above. Every NEW activation runs Init;
                // it must not inherit the retired voice's radius clock.
                let geometry = match (geometry, hearing) {
                    (Some(g), Some(h)) => Some(crate::ambient_geometry::Voice::update(
                        g.source,
                        h,
                        None,
                        p.params.radius,
                        now,
                    )?),
                    (None, None) => None,
                    _ => return Err("Incomplete ambient geometry candidate".into()),
                };
                state.voices.push(Voice {
                    geometry,
                    owner: p.owner,
                    sound: p.sound,
                    started: tick,
                    priority: Some(priority),
                    fade: None,
                    playback: p.params.select(&mut state.seed),
                });
            }
        }
        Ok(())
    }
    fn ambient_location(&self, owner: i32) -> Result<[f32; 3], String> {
        self.ambient_location_in(&self.state, owner)
    }
    fn ambient_location_in(&self, state: &State, owner: i32) -> Result<[f32; 3], String> {
        if owner == 736 {
            let eye = self
                .bacta_effect_anchors_in(state)?
                .ok_or("Missing ambient Eye transform")?
                .0;
            return Ok([eye[3][0], eye[3][1], eye[3][2]]);
        }
        let b = self
            .opening
            .as_ref()
            .and_then(|o| {
                o.assets
                    .world_effects
                    .iter()
                    .find(|b| b.original as i32 == owner)
            })
            .ok_or("Missing ambient source")?;
        Ok(state
            .opening
            .as_ref()
            .and_then(|o| o.world_effects.as_ref())
            .and_then(|s| s.locations.iter().find(|(id, _, _)| *id == b.original))
            .map_or(b.location, |(_, p, _)| *p))
    }
    /// Matches the selected presentation camera, including intro and rappel.
    pub fn audio_listener(&self) -> ([f32; 3], f32) {
        let rappel = self.rappel_eye();
        let locked = self.state.opening.as_ref().is_some_and(|s| s.locked);
        let eye = if let Some(p) = rappel {
            p
        } else if locked {
            self.opening.as_ref().unwrap().assets.camera_origin
        } else {
            self.eye()
        };
        let yaw = if rappel.is_none() && locked {
            let target = self.opening.as_ref().unwrap().assets.camera_target;
            (target[1] - eye[1]).atan2(target[0] - eye[0])
        } else {
            self.state.yaw
        };
        (eye.map(|v| v as f32), yaw as f32)
    }
    pub fn spatial_ambient_sources(&self) -> Result<Vec<Source>, String> {
        let mut sources = vec![];
        for (sound, owner) in self.ambient_loops() {
            let profile = self.opening.as_ref().and_then(|o| {
                o.assets
                    .ambient_profiles
                    .iter()
                    .find(|p| p.owner == owner && p.sound == sound)
            });
            let Some(profile) = profile else {
                sources.push(Source {
                    activation: 0,
                    owner,
                    sound,
                    location: [0.; 3],
                    playback: None,
                    flags: 0,
                });
                continue;
            };
            let Some(voice) = self
                .state
                .opening
                .as_ref()
                .and_then(|o| o.ambient_audio.as_ref())
                .and_then(|s| s.voices.iter().find(|v| v.owner == owner))
            else {
                continue;
            };
            let Some(mut playback) = voice.playback.clone() else {
                continue;
            };
            if let Some(f) = &voice.fade {
                playback.volume *= f.value;
                if playback.volume <= 0.0001 {
                    continue;
                }
            }
            if let Some(g) = &voice.geometry {
                // Preserve the original unbounded smoothing in checkpoints, including
                // overshoot. The portable playback protocol only accepts 0..1e6.
                playback.radius = g.radii.restore()?.current.clamp(0., 1e6);
                // The legacy Android adapter interprets radius zero as no falloff.
                // A collapsed geometric radius must instead produce no stream.
                if profile.params.radius > 0.
                    && playback.radius == 0.
                    && !profile.params.non_positional
                    && !profile.params.disable_falloff
                {
                    continue;
                }
            }
            let location = self.ambient_location(owner)?;
            sources.push(Source {
                activation: voice.started,
                owner,
                sound,
                location,
                playback: Some(playback),
                flags: 1
                    | (u32::from(profile.params.non_positional) * 2)
                    | (u32::from(profile.params.disable_falloff) * 4),
            });
        }
        if self
            .opening
            .as_ref()
            .is_some_and(|o| o.assets.ambient_policy.is_some())
        {
            let voices = &self
                .state
                .opening
                .as_ref()
                .and_then(|o| o.ambient_audio.as_ref())
                .ok_or("Missing ambient lifecycle state")?
                .voices;
            let priority = |owner| {
                voices
                    .iter()
                    .find(|v| v.owner == owner)
                    .and_then(|v| v.priority)
                    .unwrap_or(0.)
            };
            sources.sort_by(|a, b| {
                priority(b.owner)
                    .total_cmp(&priority(a.owner))
                    .then(a.owner.cmp(&b.owner))
            });
        }
        Ok(sources)
    }
}
