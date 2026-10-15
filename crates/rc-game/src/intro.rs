//! Selected original intro actors; native animation timing with bounded port
//! static collision and walking navigation. No full native GoalQueue parity.
use crate::{
    intro_flow::{self, Effect},
    visual::{Mesh, PreparedMesh},
    Game, Point, STEP,
};
use rc_package::{
    animation_call::PlayAnimParameters,
    channel_playback::{play_channel, ChannelPlaybackSnapshot, PlaybackSequence},
    event_lookup::EventNameSnapshot,
    placement,
    skeletal_animation_tick::TickNotify,
    skeletal_lod_tick::{update_lod_animation, TickSequence},
};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Actor {
    pub original: u32,
    pub mesh: Option<Mesh>,
    pub location: [f32; 3],
    pub rotation: [i32; 3],
    pub extent: Point,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub flow: intro_flow::Flow,
    pub actors: Vec<Actor>,
    pub teleport: [f32; 3],
    pub rotation: [i32; 3],
    pub destination: Point,
    pub sounds: [usize; 6],
}
pub struct Prepared {
    pub assets: Assets,
    meshes: Vec<PreparedMesh>,
    textures: Vec<usize>,
    anchors: Vec<[u32; 16]>,
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct ActorState {
    pub position: Point,
    pub matrix: [u32; 16],
    pub frame: f32,
    pub playing: bool,
    pub destroyed: bool,
    pub playback: ChannelPlaybackSnapshot,
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct State {
    pub flow: intro_flow::State,
    pub actors: Vec<ActorState>,
    pub signal_cursor: usize,
    pub effect_cursor: usize,
    pub notifies: Vec<(u64, u32)>,
    pub teleport_accepted: Option<bool>,
    pub route: Vec<Point>,
    pub next: usize,
    pub goal_started: bool,
    pub goal_result: Option<String>,
}
fn matrix(rotation: [i32; 3], at: [f32; 3]) -> Result<[u32; 16], String> {
    Ok(
        rc_package::mesh_transform::rotation_matrix(rotation, at.map(f32::to_bits))?
            .concat()
            .try_into()
            .unwrap(),
    )
}
impl Assets {
    pub fn prepare(
        mut self,
        scene: &mut rc_render::Scene,
        sounds: usize,
    ) -> Result<(Prepared, State), String> {
        self.flow.validate()?;
        if self.actors.len() != 2
            || self.actors[0].original != 452
            || self.actors[1].original != 530
            || self.sounds.iter().any(|r| *r >= sounds)
            || self.teleport.iter().any(|v| !v.is_finite())
            || self.destination.iter().any(|v| !v.is_finite())
        {
            return Err("Invalid intro bindings".into());
        }
        let mut meshes = vec![];
        let mut textures = vec![];
        let mut anchors = vec![];
        let mut actors = vec![];
        for a in &mut self.actors {
            if a.location.iter().any(|v| !v.is_finite())
                || a.extent
                    .iter()
                    .any(|v| !v.is_finite() || *v <= 0. || *v > 1000.)
            {
                return Err("Invalid intro body".into());
            }
            let mesh = a
                .mesh
                .take()
                .ok_or("Missing intro mesh")?
                .prepare_reference()?;
            mesh.clip("WarriorIntroBreathe")?;
            let clip = mesh.clip("WarriorIntro")?;
            let m = matrix(a.rotation, a.location)?;
            anchors.push(rc_package::root_motion::compose_relative(
                rc_package::skeletal_matrix_inverse::inverse_matrix(mesh.root_matrix(clip, 0.)?),
                m,
            )?);
            textures.push(scene.textures.len());
            scene
                .textures
                .extend(mesh.mesh.materials.iter().map(|m| rc_render::Texture {
                    width: m.width,
                    height: m.height,
                    pixels: m.pixels.clone(),
                }));
            meshes.push(mesh);
            actors.push(ActorState {
                position: a.location.map(f64::from),
                matrix: m,
                frame: 0.,
                playing: false,
                destroyed: false,
                playback: ChannelPlaybackSnapshot {
                    channels: vec![],
                    byte_60: 1,
                    byte_61: 0,
                },
            });
        }
        let mut flow = intro_flow::State::default();
        flow.begin(&self.flow)?;
        let state = State {
            flow,
            actors,
            signal_cursor: 0,
            effect_cursor: 0,
            notifies: vec![],
            teleport_accepted: None,
            route: vec![],
            next: 0,
            goal_started: false,
            goal_result: None,
        };
        Ok((
            Prepared {
                assets: self,
                meshes,
                textures,
                anchors,
            },
            state,
        ))
    }
}
/// Existing mathematical static-world adapter, not native query/backoff parity.
struct World<'a>(&'a rc_package::world_collision::StaticBodyWorld);
impl placement::World for World<'_> {
    fn encroaching(&mut self, p: [f32; 3], e: [f32; 3]) -> Result<bool, String> {
        let q = self
            .0
            .sweep(p.map(f64::from), p.map(f64::from), e.map(f64::from))?;
        if !q.complete {
            return Err("Incomplete intro placement".into());
        }
        Ok(!q.contacts.is_empty())
    }
    fn trace(
        &mut self,
        s: [f32; 3],
        e: [f32; 3],
        extent: [f32; 3],
    ) -> Result<placement::Hit, String> {
        let q = self
            .0
            .sweep(s.map(f64::from), e.map(f64::from), extent.map(f64::from))?;
        if !q.complete {
            return Err("Incomplete intro placement trace".into());
        }
        let h = q.contacts.first();
        let fraction = h.map_or(1., |c| c.hit.fraction as f32);
        Ok(placement::Hit {
            actor: h.is_some(),
            fraction,
            location: std::array::from_fn(|i| s[i] + (e[i] - s[i]) * fraction),
            normal: h.map_or([0.; 3], |c| c.hit.normal.map(|v| v as f32)),
        })
    }
}
fn sound(state: &mut crate::State, role: usize, owner: i32) {
    let m = state.mission.as_mut().unwrap();
    m.audio_sequence += 1;
    if m.events.len() >= 256 {
        m.events.remove(0);
    }
    m.events.push(crate::combat::Event {
        sequence: m.audio_sequence,
        tick: state.statistics.ticks + 1,
        sound: role,
        owner,
    });
}
impl Game {
    pub(crate) fn intro_tick(&mut self) -> Result<(), String> {
        let Some(p) = self.opening.as_ref().and_then(|p| p.intro.as_ref()) else {
            return Ok(());
        };
        let mut s = self.state.opening.as_mut().unwrap().intro.take().unwrap();
        let result = (|| -> Result<(), String> {
            s.flow.tick(&p.assets.flow, STEP)?;
            let events = &self.state.opening.as_ref().unwrap().events;
            for (_, _, tag) in &events[s.signal_cursor..] {
                s.flow.signal(&p.assets.flow, tag)?;
            }
            s.signal_cursor = events.len();
            {
                while s.effect_cursor < s.flow.events.len() {
                    let effect = s.flow.events[s.effect_cursor].effect.clone();
                    s.effect_cursor += 1;
                    match effect {
                        Effect::Animation {
                            actor,
                            clip,
                            repeat,
                        } => {
                            if !repeat {
                                let i = usize::from(actor == 530);
                                let mesh = &p.meshes[i];
                                let c = mesh.clip(&clip)?;
                                let seq = &mesh.mesh.clips[c].sequence;
                                let a = &mut s.actors[i];
                                let mut channel = mesh.channel(c, 0.)?;
                                channel.words[0] = 999;
                                a.playback.channels = vec![channel];
                                play_channel(
                                    &mut a.playback,
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
                                a.playing = true;
                            }
                        }
                        Effect::Sound { owner } => sound(
                            &mut self.state,
                            p.assets.sounds[usize::from(owner == 452)],
                            owner as i32,
                        ),
                        Effect::Destroy { actor } => {
                            s.actors[usize::from(actor == 530)].destroyed = true
                        }
                        Effect::Teleport { .. } => {
                            let mut point = p.assets.teleport;
                            let accepted = p.assets.flow.teleport_no_check
                                || placement::find_spot(
                                    &mut point,
                                    p.assets.actors[1].extent.map(|v| v as f32),
                                    &mut World(&self.world),
                                )?;
                            s.teleport_accepted = Some(accepted);
                            if accepted {
                                s.actors[1].position = point.map(f64::from);
                            }
                        }
                        Effect::RotateTo { .. } => {
                            s.actors[1].matrix =
                                matrix(p.assets.rotation, s.actors[1].position.map(|v| v as f32))?
                        }
                        Effect::MoveGoal { .. } => s.goal_started = true,
                        Effect::Collision { .. } => {} // NPCs have no blocking/player-combat role in this bounded host.
                    }
                }
                // Process end-generated teleport/rotation effects before returning.

                for i in 0..2 {
                    let a = &mut s.actors[i];
                    if !a.playing || a.destroyed {
                        continue;
                    }
                    let mesh = &p.meshes[i];
                    let c = mesh.clip("WarriorIntro")?;
                    let seq = &mesh.mesh.clips[c].sequence;
                    let mut host = crate::rappel::TickHost {
                        random: None,
                        sequence: TickSequence {
                            frames: seq.frames,
                            rate: f32::from_bits(seq.rate_bits),
                            jitter_amplitude: f32::from_bits(seq.word_18),
                            notifies: seq
                                .notifies
                                .iter()
                                .map(|n| TickNotify {
                                    time_bits: n.time_bits,
                                    object: n.object_index as u32,
                                })
                                .collect(),
                        },
                        events: vec![],
                        ended: false,
                    };
                    update_lod_animation(
                        &mut a.playback.channels,
                        &mut a.playback.byte_61,
                        STEP as f32,
                        false,
                        &mut host,
                    )?;
                    a.frame = f32::from_bits(a.playback.channels[0].words[7])
                        .clamp(0., 1. - 1. / seq.frames as f32);
                    let m = rc_package::root_motion::compose_relative(
                        mesh.root_matrix(c, a.frame)?,
                        p.anchors[i],
                    )?;
                    let target: Point = std::array::from_fn(|j| f32::from_bits(m[12 + j]) as f64);
                    let q =
                        self.world
                            .sweep_motion(a.position, target, p.assets.actors[i].extent)?;
                    if !q.complete {
                        return Err("Incomplete intro root sweep".into());
                    }
                    let fraction = q.contacts.first().map_or(1., |c| c.hit.fraction.max(0.));
                    a.position = std::array::from_fn(|j| {
                        a.position[j] + (target[j] - a.position[j]) * fraction
                    });
                    a.matrix = m;
                    for j in 0..3 {
                        a.matrix[12 + j] = (a.position[j] as f32).to_bits();
                    }
                    for n in host.events {
                        s.notifies.push((self.state.statistics.ticks + 1, n));
                        if (190..=193).contains(&n) {
                            sound(
                                &mut self.state,
                                p.assets.sounds[2 + (n - 190) as usize],
                                530,
                            );
                        }
                    }
                    if host.ended {
                        a.playing = false;
                        if i == 1 {
                            s.flow.animation_ended(&p.assets.flow, 530)?;
                        }
                    }
                }
            }
            if s.goal_started && s.goal_result.is_none() {
                // WaitForLanding only waits for Falling (2), not RootMotion (12).
                let extent = p.assets.actors[1].extent;
                if s.route.is_empty() {
                    let from = crate::mission_motion::ground(
                        &self.world,
                        s.actors[1].position,
                        extent,
                        512.,
                    )?;
                    let target = [
                        p.assets.destination[0],
                        p.assets.destination[1],
                        p.assets.destination[2] + 200.,
                    ];
                    let to = crate::mission_motion::ground(&self.world, target, extent, 512.)?;
                    if let (Some(from), Some(to)) = (from, to) {
                        s.route = self.warrior_ai.as_ref().unwrap().navigation.plan_for(
                            &self.world,
                            from,
                            to,
                            extent,
                        )?;
                    }
                    if s.route.is_empty() {
                        // Explicit failed bounded path result, never successful arrival.
                        s.goal_result = Some("NoPath: bounded walking graph".into());
                    }
                }
                if s.goal_result.is_none() {
                    let target = s.route[s.next];
                    let from = s.actors[1].position;
                    let distance = crate::distance(from, target);
                    if distance < 2. {
                        s.next += 1;
                        if s.next == s.route.len() {
                            s.goal_result = Some("Arrived".into());
                        }
                    } else {
                        let fraction = (450. * STEP / distance).min(1.);
                        let next =
                            std::array::from_fn(|i| from[i] + (target[i] - from[i]) * fraction);
                        let q = self.world.sweep_motion(from, next, extent)?;
                        if !q.complete {
                            return Err("Incomplete intro walking sweep".into());
                        }
                        if q.contacts.is_empty() {
                            s.actors[1].position = next;
                            s.actors[1].matrix = matrix(p.assets.rotation, next.map(|v| v as f32))?;
                        } else {
                            s.goal_result = Some("Blocked: bounded static sweep".into());
                        }
                    }
                }
                if s.goal_result.is_some() {
                    s.flow.move_completed(530);
                }
            }
            Ok(())
        })();
        self.state.opening.as_mut().unwrap().intro = Some(s);
        result
    }
    pub(crate) fn intro_geometry(
        &self,
    ) -> Result<
        (
            Vec<rc_package::geometry::Triangle>,
            Vec<Option<rc_render::FaceTexture>>,
        ),
        String,
    > {
        let Some(p) = self.opening.as_ref().and_then(|p| p.intro.as_ref()) else {
            return Ok((vec![], vec![]));
        };
        let s = self.state.opening.as_ref().unwrap().intro.as_ref().unwrap();
        let mut triangles = vec![];
        let mut appearance = vec![];
        for (i, a) in s.actors.iter().enumerate() {
            if a.destroyed {
                continue;
            }
            let mesh = &p.meshes[i];
            let clip = mesh.clip(if a.playback.channels.is_empty() {
                "WarriorIntroBreathe"
            } else {
                "WarriorIntro"
            })?;
            let seq = &mesh.mesh.clips[clip].sequence;
            let frame = if a.playback.channels.is_empty() {
                (s.flow.clock * f32::from_bits(seq.rate_bits) as f64 / seq.frames as f64)
                    .rem_euclid(1.) as f32
            } else {
                a.frame
            };
            let mut geometry = mesh.geometry(&mesh.channel(clip, frame)?)?;
            let m = a.matrix.map(f32::from_bits);
            for t in &mut geometry.triangles {
                t.points = t.points.map(|v| {
                    std::array::from_fn(|j| {
                        ((v[0] * m[j] + v[1] * m[4 + j]) + v[2] * m[8 + j]) + m[12 + j]
                    })
                });
            }
            for f in geometry.appearance.iter_mut().flatten() {
                f.texture += p.textures[i];
            }
            triangles.extend(geometry.triangles);
            appearance.extend(geometry.appearance);
        }
        Ok((triangles, appearance))
    }
}
