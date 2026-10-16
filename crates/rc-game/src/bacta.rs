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
    pub mesh: Option<Mesh>,
    pub location: [f32; 3],
    pub rotation: [i32; 3],
    pub scale: [f32; 3],
    pub pivot: [f32; 3],
    pub notifies: Vec<Notify>,
}
pub struct Prepared {
    pub mesh: PreparedMesh,
    pub assets: Assets,
    pub texture_base: usize,
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
        let mesh = self
            .mesh
            .take()
            .ok_or("Missing Bacta mesh")?
            .prepare_reference()?;
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
        Ok(Prepared {
            mesh,
            assets: self,
            texture_base,
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
        Ok(())
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
