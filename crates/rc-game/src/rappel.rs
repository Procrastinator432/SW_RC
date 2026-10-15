//! Selected original EntranceRope600 channel and landing-camera adapter.
//! Native channel timing/root matrices; static-world sweeps remain the port collision host.
use crate::{
    visual::{Mesh, PreparedMesh},
    Game, Point, STEP,
};
use rc_package::{
    animation_call::PlayAnimParameters,
    channel_playback::{
        play_channel, ChannelPlaybackHost, ChannelPlaybackSnapshot, PlaybackSequence,
    },
    event_lookup::EventNameSnapshot,
    mesh_animation::AnimationChannel,
    skeletal_animation_tick::{TickEndHost, TickNotify, TickNotifyHost},
    skeletal_lod_tick::{update_lod_animation, LodTickHost, TickSequence},
    skeletal_sequence::SequenceLookupHost,
};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notify {
    pub original: u32,
    pub time_bits: u32,
    pub role: Option<usize>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub player: Option<Mesh>,
    pub rope: Option<Mesh>,
    pub start: Point,
    pub rotation: [i32; 3],
    pub rope_location: [f32; 3],
    pub rope_rotation: [i32; 3],
    pub eye_height: f64,
    pub limits: [f64; 4],
    pub notifies: Vec<Notify>,
}
pub struct Prepared {
    pub assets: Assets,
    pub player: PreparedMesh,
    pub rope: PreparedMesh,
    pub texture_base: usize,
    pub anchor: [u32; 16],
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct State {
    pub started: bool,
    pub finished: bool,
    pub rope_destroyed: bool,
    pub playback: ChannelPlaybackSnapshot,
    pub notify_events: Vec<(u64, u32)>,
    pub finished_tick: Option<u64>,
    pub blocked_steps: u32,
}
impl Default for State {
    fn default() -> Self {
        Self {
            started: false,
            finished: false,
            rope_destroyed: false,
            playback: ChannelPlaybackSnapshot {
                channels: vec![],
                byte_60: 1,
                byte_61: 0,
            },
            notify_events: vec![],
            finished_tick: None,
            blocked_steps: 0,
        }
    }
}
impl Assets {
    pub fn validate(&self, sounds: usize) -> Result<(), String> {
        if self
            .start
            .iter()
            .chain(self.limits.iter())
            .any(|v| !v.is_finite() || v.abs() > 1e7)
            || self.rope_location.iter().any(|v| !v.is_finite())
            || !self.eye_height.is_finite()
            || !(1. ..=120.).contains(&self.eye_height)
            || self.limits[0] > self.limits[1]
            || self.limits[2] > self.limits[3]
            || self.notifies.len() != 7
            || self.notifies.iter().any(|n| {
                !f32::from_bits(n.time_bits).is_finite()
                    || !(0. ..1.).contains(&f32::from_bits(n.time_bits))
                    || n.role.is_some_and(|r| r >= sounds)
            })
        {
            return Err("Invalid rappel bindings".into());
        }
        Ok(())
    }
    pub fn prepare(mut self, scene: &mut rc_render::Scene) -> Result<Prepared, String> {
        let player = self
            .player
            .take()
            .ok_or("Missing rappel player")?
            .prepare_reference()?;
        let clip = player.clip("EntranceRope600")?;
        let identity: [u32; 16] = rc_package::mesh_transform::rotation_matrix([0; 3], [0; 3])?
            .concat()
            .try_into()
            .unwrap();
        if player.root_matrix(clip, 0.)?.map(f32::from_bits) != identity.map(f32::from_bits) {
            return Err("Rappel entry root must be identity".into());
        }
        let seq = &player.mesh.clips[clip].sequence;
        if seq.frames != 141
            || f32::from_bits(seq.rate_bits) != 30.
            || seq.flags & 1 == 0
            || seq.notifies.len() != self.notifies.len()
            || seq
                .notifies
                .iter()
                .zip(&self.notifies)
                .any(|(a, b)| a.object_index != b.original as i32 || a.time_bits != b.time_bits)
        {
            return Err("Rappel original sequence differs".into());
        }
        let rope = self
            .rope
            .take()
            .ok_or("Missing rappel rope")?
            .prepare_reference()?;
        rope.clip("ActionBreathe")?;
        let texture_base = scene.textures.len();
        scene
            .textures
            .extend(rope.mesh.materials.iter().map(|m| rc_render::Texture {
                width: m.width,
                height: m.height,
                pixels: m.pixels.clone(),
            }));
        let anchor = rc_package::mesh_transform::rotation_matrix(
            self.rotation,
            self.start.map(|v| (v as f32).to_bits()),
        )?
        .concat()
        .try_into()
        .unwrap();
        Ok(Prepared {
            assets: self,
            player,
            rope,
            texture_base,
            anchor,
        })
    }
}
pub(crate) struct StartHost;
impl ChannelPlaybackHost for StartHost {
    fn random_int(&mut self) -> Result<i32, String> {
        Err("Unexpected randomized rappel start".into())
    }
    fn post_init(&mut self, _: &mut AnimationChannel) -> Result<(), String> {
        Ok(())
    }
    fn replicate(&mut self, _: usize, _: &AnimationChannel) -> Result<(), String> {
        Ok(())
    }
}
pub(crate) struct TickHost {
    pub(crate) random: Option<u32>,
    pub(crate) sequence: TickSequence,
    pub(crate) events: Vec<u32>,
    pub(crate) ended: bool,
}
impl SequenceLookupHost for TickHost {
    fn find_sequence(&mut self, _: u32, _: bool) -> Result<u32, String> {
        Ok(1)
    }
}
impl TickNotifyHost for TickHost {
    fn notify(&mut self, id: u32, _: &mut Vec<AnimationChannel>) -> Result<(), String> {
        self.events.push(id);
        Ok(())
    }
}
impl TickEndHost for TickHost {
    fn clear(&mut self, _: [u32; 8], _: &mut AnimationChannel) -> Result<(), String> {
        Err("Unexpected upper-channel clear".into())
    }
    fn anim_end(&mut self, _: i32, _: &mut AnimationChannel) -> Result<(), String> {
        self.ended = true;
        Ok(())
    }
}
impl LodTickHost for TickHost {
    fn set_locked(&mut self, _: bool) -> Result<(), String> {
        Ok(())
    }
    fn flags(&mut self) -> Result<u32, String> {
        Ok(0)
    }
    fn request_destroy(&mut self) -> Result<(), String> {
        Err("Unexpected animation destruction".into())
    }
    fn sequence(&mut self, id: u32) -> Result<TickSequence, String> {
        if id != 1 {
            return Err("Unknown rappel sequence".into());
        }
        Ok(self.sequence.clone())
    }
    fn random_int(&mut self) -> Result<i32, String> {
        let seed = self.random.as_mut().ok_or("Unexpected animation jitter")?;
        *seed = seed.wrapping_mul(214013).wrapping_add(2531011);
        Ok(((*seed >> 16) & 32767) as i32)
    }
    fn actor_has_mesh(&mut self) -> Result<bool, String> {
        Ok(true)
    }
    fn replicate(&mut self, _: usize, _: &mut Vec<AnimationChannel>) -> Result<(), String> {
        Ok(())
    }
}
impl Prepared {
    fn start(&self, s: &mut State) -> Result<(), String> {
        let c = self.player.clip("EntranceRope600")?;
        let seq = &self.player.mesh.clips[c].sequence;
        let mut channel = self.player.channel(c, 0.)?;
        channel.words[0] = 99; // original preceding ActionBreathe; force channel reinitialization.
        s.playback.channels = vec![channel];
        let name = EventNameSnapshot {
            handle: 1,
            resolved_index: 1,
        };
        play_channel(
            &mut s.playback,
            0,
            &PlayAnimParameters {
                sequence: name,
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
            &mut StartHost,
        )?;
        s.started = true;
        Ok(())
    }
}
impl Game {
    pub(crate) fn rappel_tick(&mut self) -> Result<bool, String> {
        let Some(opening) = &self.opening else {
            return Ok(false);
        };
        let Some(p) = &opening.rappel else {
            return Ok(false);
        };
        let state = self.state.opening.as_mut().unwrap();
        let s = state.rappel.as_mut().unwrap();
        if !s.started || s.finished {
            return Ok(false);
        }
        if s.playback.channels.is_empty() {
            // event sets started; channel begins exactly once.
            p.start(s)?;
        }
        let clip = p.player.clip("EntranceRope600")?;
        let seq = &p.player.mesh.clips[clip].sequence;
        let mut host = TickHost {
            random: None,
            sequence: TickSequence {
                frames: seq.frames,
                rate: f32::from_bits(seq.rate_bits),
                jitter_amplitude: f32::from_bits(seq.word_18),
                notifies: p
                    .assets
                    .notifies
                    .iter()
                    .map(|n| TickNotify {
                        time_bits: n.time_bits,
                        object: n.original,
                    })
                    .collect(),
            },
            events: vec![],
            ended: false,
        };
        update_lod_animation(
            &mut s.playback.channels,
            &mut s.playback.byte_61,
            STEP as f32,
            false,
            &mut host,
        )?;
        let frame =
            f32::from_bits(s.playback.channels[0].words[7]).clamp(0., 1. - 1. / seq.frames as f32);
        let matrix = rc_package::root_motion::compose_relative(
            p.player.root_matrix(clip, frame)?,
            p.anchor,
        )?;
        let to: Point = std::array::from_fn(|j| f32::from_bits(matrix[12 + j]) as f64);
        let from = self.state.pawn.body.position;
        let sweep = self
            .world
            .sweep_motion(from, to, self.profile.physics.extent)?;
        if !sweep.complete {
            return Err("Incomplete rappel body sweep".into());
        }
        let fraction = sweep
            .contacts
            .first()
            .map_or(1., |c| c.hit.fraction.max(0.));
        if fraction < 1. {
            s.blocked_steps += 1;
        }
        self.state.pawn.body.position =
            std::array::from_fn(|j| from[j] + (to[j] - from[j]) * fraction);
        self.state.pawn.body.velocity = [0.; 3];
        let tick = self.state.statistics.ticks + 1;
        for id in host.events {
            s.notify_events.push((tick, id));
            if let Some(role) = p
                .assets
                .notifies
                .iter()
                .find(|n| n.original == id)
                .and_then(|n| n.role)
            {
                let m = self.state.mission.as_mut().unwrap();
                m.audio_sequence += 1;
                if m.events.len() >= 256 {
                    m.events.remove(0);
                }
                m.events.push(crate::combat::Event {
                    sequence: m.audio_sequence,
                    tick,
                    sound: role,
                    owner: -1,
                });
            }
        }
        if host.ended {
            s.finished = true;
            s.finished_tick = Some(tick);
        }
        // Final authored pose is applied once; ordinary floor physics resumes next tick.
        Ok(true)
    }
    pub(crate) fn clamp_opening_view(&mut self) {
        if !self.state.opening.as_ref().is_some_and(|s| s.locked) {
            return;
        }
        let Some(p) = self.opening.as_ref().and_then(|o| o.rappel.as_ref()) else {
            return;
        };
        let yaw = (self.state.yaw + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI;
        self.state.yaw = yaw.clamp(p.assets.limits[2], p.assets.limits[3]);
        self.state.pitch = self
            .state
            .pitch
            .clamp(p.assets.limits[0].max(-1.4), p.assets.limits[1].min(1.4));
    }
    pub(crate) fn rappel_eye(&self) -> Option<Point> {
        let p = self.opening.as_ref()?.rappel.as_ref()?;
        let s = self.state.opening.as_ref()?;
        (s.locked || !s.rappel.as_ref()?.finished).then(|| {
            let mut at = self.state.pawn.body.position;
            at[2] += p.assets.eye_height;
            at
        })
    }
    pub(crate) fn rappel_geometry(
        &self,
    ) -> Result<
        (
            Vec<rc_package::geometry::Triangle>,
            Vec<Option<rc_render::FaceTexture>>,
        ),
        String,
    > {
        let Some(p) = self.opening.as_ref().and_then(|o| o.rappel.as_ref()) else {
            return Ok((vec![], vec![]));
        };
        let s = self
            .state
            .opening
            .as_ref()
            .unwrap()
            .rappel
            .as_ref()
            .unwrap();
        if s.rope_destroyed {
            return Ok((vec![], vec![]));
        }
        let c = p.rope.clip("ActionBreathe")?;
        let mut scene = p.rope.geometry(&p.rope.channel(c, 0.)?)?;
        for t in &mut scene.triangles {
            t.points = t.points.map(|v| {
                rc_package::mesh::transform_point(
                    v,
                    p.assets.rope_location,
                    p.assets.rope_rotation,
                    [1.; 3],
                    [0.; 3],
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
mod tests {
    use super::*;
    fn assets() -> Assets {
        Assets {
            player: None,
            rope: None,
            start: [0.; 3],
            rotation: [0; 3],
            rope_location: [0.; 3],
            rope_rotation: [0; 3],
            eye_height: 60.,
            limits: [-1., 1., -1., 1.],
            notifies: (0..7)
                .map(|original| Notify {
                    original,
                    time_bits: 0.5f32.to_bits(),
                    role: Some(79),
                })
                .collect(),
        }
    }
    #[test]
    fn rejects_nonfinite_positions_limits_and_out_of_bank_foley() {
        let mut a = assets();
        a.validate(80).unwrap();
        a.start[0] = f64::NAN;
        assert!(a.validate(80).is_err());
        let mut a = assets();
        a.limits[0] = 2.;
        assert!(a.validate(80).is_err());
        let mut a = assets();
        a.notifies[0].role = Some(80);
        assert!(a.validate(80).is_err());
        let mut a = assets();
        a.notifies[0].time_bits = f32::NAN.to_bits();
        assert!(a.validate(80).is_err());
    }
    #[test]
    fn animation_jitter_requires_explicit_seed_and_has_replayable_crt_draws() {
        let mut host = TickHost {
            random: None,
            sequence: TickSequence {
                frames: 10,
                rate: 10.,
                jitter_amplitude: 0.2,
                notifies: vec![],
            },
            events: vec![],
            ended: false,
        };
        assert!(host.random_int().is_err());
        host.random = Some(1);
        assert_eq!(host.random_int().unwrap(), 41);
        let checkpoint = host.random;
        assert_eq!(host.random_int().unwrap(), 18467);
        host.random = checkpoint;
        assert_eq!(host.random_int().unwrap(), 18467);
    }
    #[test]
    fn native_nonlooping_channel_ends_once_and_keeps_final_frame() {
        let mut channels = vec![AnimationChannel::default()];
        let c = &mut channels[0];
        c.words[0] = 1;
        c.words[17] = 1;
        c.words[6] = 1f32.to_bits();
        c.words[9] = 0.9f32.to_bits();
        let mut h = TickHost {
            random: None,
            sequence: TickSequence {
                frames: 10,
                rate: 10.,
                jitter_amplitude: 0.,
                notifies: vec![
                    TickNotify {
                        time_bits: 0.3f32.to_bits(),
                        object: 1,
                    },
                    TickNotify {
                        time_bits: 0.6f32.to_bits(),
                        object: 2,
                    },
                ],
            },
            events: vec![],
            ended: false,
        };
        let mut ended = 0;
        for _ in 0..90 {
            h.ended = false;
            update_lod_animation(&mut channels, &mut 0, STEP as f32, false, &mut h).unwrap();
            ended += usize::from(h.ended);
        }
        assert_eq!(h.events, [1, 2]);
        assert_eq!(ended, 1);
        assert_eq!(f32::from_bits(channels[0].words[7]), 0.9);
        assert_eq!(channels[0].words[6], 0);
    }
}
