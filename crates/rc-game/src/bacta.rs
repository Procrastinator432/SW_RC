//! Selected actor736 lifecycle. Original Load/FinishAnim/Holster and sound notifies;
//! the legacy bottom-aligned station transform/collision remains a port adapter.
use crate::{
    visual::{Mesh, PreparedMesh},
    Game, STEP,
};
use rc_package::{
    animation_call::PlayAnimParameters,
    channel_playback::{play_channel, ChannelPlaybackSnapshot, PlaybackSequence},
    event_lookup::EventNameSnapshot,
    skeletal_animation_tick::TickNotify,
    skeletal_lod_tick::{update_lod_animation, TickSequence},
};
use serde::{Deserialize, Serialize};
type EffectAnchors = ([[f32; 4]; 4], [[f32; 4]; 4], Vec<[f32; 3]>);

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notify {
    pub clip: String,
    pub original: u32,
    pub time_bits: u32,
    pub role: usize,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collision: Option<rc_package::cylinder::Cylinder>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<crate::particles::Template>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient_loop: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controller: Option<Controller>,
    pub mesh: Option<Mesh>,
    pub location: [f32; 3],
    pub rotation: [i32; 3],
    pub scale: [f32; 3],
    pub pivot: [f32; 3],
    pub notifies: Vec<Notify>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Controller {
    pub actor_location: [f32; 3],
    pub desired_xy: [f32; 2],
    pub marker_z: f32,
    pub player_range: f32,
    pub lock_location: bool,
}
fn turn_units(current: i32, desired: i32) -> i32 {
    let delta = desired.wrapping_sub(current) & 65535;
    let delta = if delta > 32768 { delta - 65536 } else { delta };
    current.wrapping_add(delta.clamp(-750, 750))
}
pub struct Prepared {
    pub mesh: PreparedMesh,
    pub assets: Assets,
    pub texture_base: usize,
    pub effect_texture_base: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Phase {
    #[default]
    Standby,
    Loading,
    Active,
    Holstering,
    Depleted,
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct State {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub particles: Option<crate::particles::State>,
    pub phase: Phase,
    pub playback: Option<ChannelPlaybackSnapshot>,
    pub notify_events: Vec<(u64, u32)>,
    pub activated_tick: Option<u64>,
}
impl Assets {
    pub fn prepare(
        mut self,
        scene: &mut rc_render::Scene,
        sounds: usize,
    ) -> Result<Prepared, String> {
        if let Some(c) = self.collision {
            c.validate()?;
        }
        if self
            .location
            .iter()
            .chain(self.scale.iter())
            .chain(self.pivot.iter())
            .any(|v| !v.is_finite() || v.abs() > 1e7)
            || self.scale.iter().any(|v| *v <= 0.)
            || self.notifies.len() != 6
        {
            return Err("Invalid Bacta station bindings".into());
        }
        if self.controller.as_ref().is_some_and(|c| {
            c.actor_location
                .iter()
                .chain(c.desired_xy.iter())
                .chain([&c.marker_z, &c.player_range])
                .any(|v| !v.is_finite() || v.abs() > 1e7)
                || !(1. ..=1000.).contains(&c.player_range)
        }) {
            return Err("Invalid Bacta player controller".into());
        }
        let mesh = self
            .mesh
            .take()
            .ok_or("Missing Bacta mesh")?
            .prepare_reference()?;
        if self.ambient_loop.is_some_and(|role| role >= sounds)
            || self.ambient_loop.is_some()
                && !mesh
                    .mesh
                    .bones
                    .iter()
                    .any(|b| b.name.name.eq_ignore_ascii_case("eye"))
        {
            return Err("Invalid Bacta Eye ambient binding".into());
        }
        for name in ["Load", "Holster"] {
            let clip = mesh.clip(name)?;
            let seq = &mesh.mesh.clips[clip].sequence;
            let bindings: Vec<_> = self.notifies.iter().filter(|n| n.clip == name).collect();
            if seq.frames != 30
                || f32::from_bits(seq.rate_bits) != 30.
                || bindings.len() != 3
                || seq.notifies.len() != bindings.len()
                || seq.notifies.iter().zip(bindings).any(|(a, b)| {
                    a.object_index != b.original as i32
                        || a.time_bits != b.time_bits
                        || b.role >= sounds
                })
            {
                return Err("Bacta original animation/notifies differ".into());
            }
        }
        let texture_base = scene.textures.len();
        scene
            .textures
            .extend(mesh.mesh.materials.iter().map(|m| rc_render::Texture {
                width: m.width,
                height: m.height,
                pixels: m.pixels.clone(),
            }));
        if !self.effects.is_empty() && self.effects.len() != 14 {
            return Err("Invalid Bacta emitter count".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        let effect_texture_base = scene.textures.len();
        for template in &self.effects {
            template.validate()?;
            if !ids.insert(template.original) {
                return Err("Duplicate Bacta emitter template".into());
            }
            scene.textures.push(rc_render::Texture {
                width: template.material.width,
                height: template.material.height,
                pixels: template.material.pixels.clone(),
            });
        }
        Ok(Prepared {
            mesh,
            assets: self,
            texture_base,
            effect_texture_base,
        })
    }
}
impl Prepared {
    fn start(&self, s: &mut State, name: &str) -> Result<(), String> {
        let clip = self.mesh.clip(name)?;
        let seq = &self.mesh.mesh.clips[clip].sequence;
        let mut channel = self.mesh.channel(clip, 0.)?;
        channel.words[0] = 99;
        let mut playback = ChannelPlaybackSnapshot {
            channels: vec![channel],
            byte_60: 1,
            byte_61: 0,
        };
        play_channel(
            &mut playback,
            0,
            &PlayAnimParameters {
                sequence: EventNameSnapshot {
                    handle: 1,
                    resolved_index: 1,
                },
                looping: false,
                bone: EventNameSnapshot {
                    handle: 0,
                    resolved_index: 0,
                },
                channel: 0,
                float_10: 1.,
                rate: 1.,
                start_frame: -1.,
            },
            Some(PlaybackSequence {
                token: 1,
                frames: seq.frames,
                rate: f32::from_bits(seq.rate_bits),
                minimum_blend: f32::from_bits(seq.minimum_blend_bits),
                randomize_start: seq.randomize_start != 0,
            }),
            &mut crate::rappel::StartHost,
        )?;
        s.playback = Some(playback);
        Ok(())
    }
    fn tick(
        &self,
        s: &mut State,
        using: bool,
        completed: bool,
        tick: u64,
    ) -> Result<Vec<usize>, String> {
        let desired = match s.phase {
            Phase::Standby | Phase::Holstering if using && !completed => {
                Some((Phase::Loading, "Load"))
            }
            Phase::Loading | Phase::Active if !using || completed => {
                Some((Phase::Holstering, "Holster"))
            }
            _ => None,
        };
        if let Some((phase, name)) = desired {
            self.start(s, name)?;
            s.phase = phase;
            s.activated_tick = None;
        }
        let name = match s.phase {
            Phase::Loading => "Load",
            Phase::Holstering => "Holster",
            _ => return Ok(vec![]),
        };
        let seq = &self.mesh.mesh.clips[self.mesh.clip(name)?].sequence;
        let mut host = crate::rappel::TickHost {
            random: None,
            sequence: TickSequence {
                frames: seq.frames,
                rate: f32::from_bits(seq.rate_bits),
                jitter_amplitude: f32::from_bits(seq.word_18),
                notifies: self
                    .assets
                    .notifies
                    .iter()
                    .filter(|n| n.clip == name)
                    .map(|n| TickNotify {
                        time_bits: n.time_bits,
                        object: n.original,
                    })
                    .collect(),
            },
            events: vec![],
            ended: false,
        };
        let playback = s.playback.as_mut().ok_or("Missing Bacta playback")?;
        update_lod_animation(
            &mut playback.channels,
            &mut playback.byte_61,
            STEP as f32,
            false,
            &mut host,
        )?;
        let mut sounds = vec![];
        for original in host.events {
            if s.notify_events.len() >= 64 {
                s.notify_events.remove(0);
            }
            s.notify_events.push((tick, original));
            sounds.push(
                self.assets
                    .notifies
                    .iter()
                    .find(|n| n.original == original && n.clip == name)
                    .ok_or("Unknown Bacta notify")?
                    .role,
            );
        }
        if host.ended {
            s.phase = match s.phase {
                Phase::Loading => {
                    s.activated_tick = Some(tick);
                    Phase::Active
                }
                _ if completed => Phase::Depleted,
                _ => Phase::Standby,
            };
        }
        Ok(sounds)
    }
}
impl Game {
    /// Desired ambient voices, not consumable cues: a restored host can
    /// reconstruct the current loop without replaying a sound-event history.
    pub fn ambient_loops(&self) -> Vec<(usize, i32)> {
        let mut loops = self.bacta_ambient_loop();
        loops.extend(self.world_ambient_loops());
        loops
    }
    fn bacta_ambient_loop(&self) -> Vec<(usize, i32)> {
        if self.state.phase != crate::Phase::Playing
            || self.state.health == 0
            || self.state.squad.as_ref().is_some_and(|s| s.player_down)
            || !self
                .state
                .mission
                .as_ref()
                .is_some_and(|m| m.using && !m.completed)
            || !self
                .state
                .opening
                .as_ref()
                .and_then(|o| o.bacta.as_ref())
                .is_some_and(|b| b.phase == Phase::Active)
        {
            return vec![];
        }
        self.opening
            .as_ref()
            .and_then(|o| o.bacta.as_ref())
            .and_then(|b| b.assets.ambient_loop)
            .map_or_else(Vec::new, |role| vec![(role, 736)])
    }
    pub(crate) fn bacta_using(&self, held: bool) -> bool {
        held && self
            .state
            .mission
            .as_ref()
            .is_some_and(|m| m.using && !m.completed)
            && self.state.health > 0
            && self
                .opening
                .as_ref()
                .and_then(|o| o.bacta.as_ref())
                .is_some_and(|b| b.assets.controller.is_some())
    }
    /// CTPlayer.UsingItem.PlayerMove: target-relative placement, proportional
    /// acceleration and bounded rotator correction. Collision/physics is the
    /// existing port host; network replication and ViewShake remain outside it.
    pub(crate) fn bacta_control(&mut self, held: bool) -> Result<Option<[f64; 2]>, String> {
        if !self.mission_use_candidate(held) {
            return Ok(None);
        }
        let Some(c) = self
            .opening
            .as_ref()
            .and_then(|o| o.bacta.as_ref())
            .and_then(|b| b.assets.controller.as_ref())
        else {
            return Ok(None);
        };
        let position = self.state.pawn.body.position.map(|v| v as f32);
        let desired = if c.lock_location {
            [c.desired_xy[0], c.desired_xy[1], position[2]]
        } else {
            position
        };
        let to: [f32; 3] = std::array::from_fn(|i| desired[i] - position[i]);
        let dist = ((to[0] * to[0] + to[1] * to[1]) + to[2] * to[2]).sqrt();
        let acceleration = if dist > 450. * STEP as f32 {
            std::array::from_fn(|i| {
                f64::from(15. * to[i] - self.state.pawn.body.velocity[i] as f32)
            })
        } else {
            if dist > 1. {
                let end = desired.map(f64::from);
                let sweep = self.world.sweep_motion(
                    self.state.pawn.body.position,
                    end,
                    self.profile.physics.extent,
                )?;
                if !sweep.complete {
                    return Err("Incomplete Bacta placement sweep".into());
                }
                if sweep.contacts.first().is_none_or(|c| c.hit.fraction >= 1.) {
                    self.state.pawn.body.position = end;
                } // Native SetLocation may fail; never snap through a solid.
            }
            self.state.pawn.body.velocity = [0.; 3];
            [0.; 2]
        };
        let eye = self.eye();
        let target: [f64; 3] = std::array::from_fn(|i| {
            f64::from(c.actor_location[i]) - eye[i]
                + if i == 2 { f64::from(c.marker_z) } else { 0. }
        });
        let unit = 65536. / std::f64::consts::TAU;
        let yaw = (target[1].atan2(target[0]) * unit) as i32;
        let pitch = (target[2].atan2(target[0].hypot(target[1])) * unit) as i32;
        let turn = |current: f64, desired: i32| {
            let current = (current * unit) as i32;
            f64::from(turn_units(current, desired)) / unit
        };
        self.state.yaw = turn(self.state.yaw, yaw).rem_euclid(std::f64::consts::TAU);
        self.state.pitch = turn(self.state.pitch, pitch).clamp(-1.4, 1.4);
        Ok(Some(acceleration))
    }
    pub(crate) fn bacta_tick(&mut self) -> Result<(), String> {
        let Some(p) = self.opening.as_ref().and_then(|o| o.bacta.as_ref()) else {
            return Ok(());
        };
        let m = self.state.mission.as_mut().ok_or("Missing Bacta mission")?;
        let s = self
            .state
            .opening
            .as_mut()
            .and_then(|o| o.bacta.as_mut())
            .ok_or("Missing Bacta state")?;
        for sound in p.tick(
            s,
            m.using && self.state.health > 0,
            m.completed,
            self.state.statistics.ticks,
        )? {
            m.audio_sequence += 1;
            if m.events.len() >= 256 {
                m.events.remove(0);
            }
            m.events.push(crate::combat::Event {
                sequence: m.audio_sequence,
                tick: self.state.statistics.ticks,
                sound,
                owner: 736,
            });
        }
        let active = m.using && self.state.health > 0 && s.phase == Phase::Active;
        if let Some((eye, body, joints)) = self.bacta_effect_anchors()? {
            let p = self.opening.as_ref().unwrap().bacta.as_ref().unwrap();
            let particles = self
                .state
                .opening
                .as_mut()
                .unwrap()
                .bacta
                .as_mut()
                .unwrap()
                .particles
                .as_mut()
                .ok_or("Missing Bacta particle state")?;
            particles.tick(&p.assets.effects, active, &eye, &body, &joints);
        }
        Ok(())
    }
    pub(crate) fn bacta_effect_anchors(&self) -> Result<Option<EffectAnchors>, String> {
        self.bacta_effect_anchors_in(&self.state)
    }
    pub(crate) fn bacta_effect_anchors_in(
        &self,
        state: &crate::State,
    ) -> Result<Option<EffectAnchors>, String> {
        let Some(p) = self
            .opening
            .as_ref()
            .and_then(|o| o.bacta.as_ref())
            .filter(|p| !p.assets.effects.is_empty())
        else {
            return Ok(None);
        };
        let s = state
            .opening
            .as_ref()
            .and_then(|o| o.bacta.as_ref())
            .ok_or("Bacta effect state")?;
        if !matches!(s.phase, Phase::Loading | Phase::Active)
            && s.particles
                .as_ref()
                .is_none_or(|s| s.eye.is_none() && s.bodies.is_empty())
        {
            return Ok(None);
        }
        let name = if matches!(s.phase, Phase::Loading | Phase::Active) {
            "Load"
        } else {
            "Holster"
        };
        let frame = s
            .playback
            .as_ref()
            .and_then(|p| p.channels.first())
            .map_or(0., |c| f32::from_bits(c.words[7]).clamp(0., 29. / 30.));
        let channel = p.mesh.channel(p.mesh.clip(name)?, frame)?;
        let local = p.mesh.attachment_transform(&channel, "eye", true)?;
        let point = |v| {
            rc_package::mesh::transform_point(
                v,
                p.assets.location,
                p.assets.rotation,
                p.assets.scale,
                p.assets.pivot,
            )
        };
        let origin = point([0.; 3]);
        let actor: [[f32; 4]; 4] = std::array::from_fn(|r| {
            if r == 3 {
                [origin[0], origin[1], origin[2], 1.]
            } else {
                let mut v = [0.; 3];
                v[r] = 1.;
                let v = point(v);
                [v[0] - origin[0], v[1] - origin[1], v[2] - origin[2], 0.]
            }
        });
        let eye = std::array::from_fn(|r| {
            std::array::from_fn(|c| (0..4).map(|i| local[r][i] * actor[i][c]).sum())
        });
        let position = state.pawn.body.position.map(|v| v as f32);
        let (sin, cos) = (state.yaw as f32).sin_cos();
        let body = [
            [cos, sin, 0., 0.],
            [-sin, cos, 0., 0.],
            [0., 0., 1., 0.],
            [position[0], position[1], position[2], 1.],
        ];
        let mesh = &self
            .rendezvous
            .as_ref()
            .ok_or("Bacta body skeleton unavailable")?
            .mesh;
        let clip = mesh.clip("ActionBreathe")?;
        let seq = &mesh.mesh.clips[clip].sequence;
        let frame =
            (state.statistics.ticks as f32 * crate::STEP as f32 * f32::from_bits(seq.rate_bits)
                / seq.frames as f32)
                .fract();
        let mut joints = mesh.joint_positions(&mesh.channel(clip, frame)?, true)?;
        for p in &mut joints {
            p[2] -= 84. + mesh.rest_bounds[0][2];
        }
        Ok(Some((eye, body, joints)))
    }
    pub(crate) fn bacta_effects(
        &self,
        basis: [[f32; 3]; 4],
    ) -> Result<Vec<rc_render::effects::TriangleEffect>, String> {
        let Some((eye, body, _)) = self.bacta_effect_anchors()? else {
            return Ok(vec![]);
        };
        let p = self.opening.as_ref().unwrap().bacta.as_ref().unwrap();
        let state = self
            .state
            .opening
            .as_ref()
            .unwrap()
            .bacta
            .as_ref()
            .unwrap()
            .particles
            .as_ref()
            .ok_or("Bacta particle state")?;
        Ok(state.geometry(&p.assets.effects, p.effect_texture_base, &eye, &body, basis))
    }
    pub(crate) fn bacta_geometry(
        &self,
    ) -> Result<
        (
            Vec<rc_package::geometry::Triangle>,
            Vec<Option<rc_render::FaceTexture>>,
        ),
        String,
    > {
        let Some(p) = self.opening.as_ref().and_then(|o| o.bacta.as_ref()) else {
            return Ok((vec![], vec![]));
        };
        let s = self
            .state
            .opening
            .as_ref()
            .and_then(|o| o.bacta.as_ref())
            .ok_or("Missing Bacta state")?;
        let name = if matches!(s.phase, Phase::Loading | Phase::Active) {
            "Load"
        } else {
            "Holster"
        };
        let frame = s
            .playback
            .as_ref()
            .and_then(|b| b.channels.first())
            .map_or(0., |c| f32::from_bits(c.words[7]).clamp(0., 29. / 30.));
        let mut channel = p.mesh.channel(p.mesh.clip(name)?, frame)?;
        if s.playback.is_none() {
            channel.words[14] = 0;
        }
        let mut scene = p.mesh.geometry_with_root(&channel, true)?;
        for t in &mut scene.triangles {
            t.points = t.points.map(|v| {
                rc_package::mesh::transform_point(
                    v,
                    p.assets.location,
                    p.assets.rotation,
                    p.assets.scale,
                    p.assets.pivot,
                )
            });
        }
        for f in scene.appearance.iter_mut().flatten() {
            f.texture += p.texture_base;
        }
        Ok((scene.triangles, scene.appearance))
    }
}

#[cfg(test)]
mod controller_tests {
    use super::turn_units;
    #[test]
    fn view_turn_takes_short_arc_across_zero_and_caps_rate() {
        assert_eq!(turn_units(65530, 5), 65541);
        assert_eq!(turn_units(5, 65530), -6);
        assert_eq!(turn_units(0, 16000), 750);
        assert_eq!(turn_units(0, 60000), -750);
        // Source keeps the exact half turn positive (strict >32768).
        assert_eq!(turn_units(0, 32768), 750);
    }
}
