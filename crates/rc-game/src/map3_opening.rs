//! Selected original GEO_01C entry lanes; native channels/root motion, bounded script host.
use crate::{
    map_runtime::Companion,
    visual::{Mesh, PreparedMesh},
    Game, Point, STEP,
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
pub struct Assets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planning: Option<Box<crate::map3_planning::Assets>>,
    pub player: Option<Mesh>,
    pub start: Point,
    pub rotation: [i32; 3],
    pub player_sounds: Vec<Notify>,
    pub actors: Vec<Actor>,
    pub scripts: Vec<Program>,
    pub limits: [f64; 4],
    pub eye_height: f64,
    pub provenance: serde_json::Value,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Actor {
    pub original: u32,
    pub role: String,
    pub mesh: Option<Mesh>,
    pub position: Point,
    pub rotation: [i32; 3],
    pub idle: String,
    pub extent: Option<Point>,
    pub height: f64,
    pub armed: bool,
    pub remove_after_intro: bool,
    pub sounds: Vec<Notify>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notify {
    pub original: u32,
    pub sound: usize,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Program {
    pub original: u32,
    pub actions: Vec<Action>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub original: u32,
    pub op: String,
    pub text: String,
    pub role: usize,
    pub seconds: f64,
}
pub struct Prepared {
    pub planning: Option<Box<crate::map3_planning::Prepared>>,
    pub assets: Assets,
    pub player: PreparedMesh,
    pub actors: Vec<(PreparedMesh, usize)>,
}
#[derive(Clone, Debug, Default, Serialize, serde::Deserialize)]
pub struct Lane {
    pub next: usize,
    pub remaining: f64,
    pub waiting: Option<usize>,
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct Animation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub random: Option<u32>,
    pub clip: String,
    pub playback: ChannelPlaybackSnapshot,
    pub finished: bool,
    pub visible: bool,
    pub blocked: u32,
    pub position: Point,
    pub notifies: Vec<(u64, u32)>,
    pub root: bool,
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct State {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub planning: Option<Box<crate::map3_planning::State>>,
    pub clock: f64,
    pub locked: bool,
    pub joined: bool,
    pub objectives: [bool; 7],
    pub active: usize,
    pub events: std::collections::BTreeSet<String>,
    pub event_ticks: std::collections::BTreeMap<String, u64>,
    pub lanes: Vec<Lane>,
    pub animations: Vec<Animation>,
    pub sev: Companion,
    pub sev_body: rc_package::physics::BodyState,
    pub subtitle: String,
    pub subtitle_until: f64,
    pub finished_tick: Option<u64>,
}
impl Assets {
    pub fn prepare(
        mut self,
        scene: &mut rc_render::Scene,
        sounds: usize,
    ) -> Result<Prepared, String> {
        if self.start.iter().any(|v| !v.is_finite() || v.abs() > 1e7)
            || self.player_sounds.iter().any(|n| n.sound >= sounds)
            || self.actors.iter().map(|a| a.role.as_str()).ne([
                "scorch", "fixer", "sev", "warrior0", "warrior1", "sunfac", "rope62", "rope40",
            ])
            || self.actors.len() != 8
            || self.scripts.len() > 16
            || !self.limits.iter().all(|v| v.is_finite())
            || self.limits[0] > self.limits[1]
            || self.limits[2] > self.limits[3]
            || !(1. ..=120.).contains(&self.eye_height)
        {
            return Err("Invalid third-map opening bindings".into());
        }
        let player = self
            .player
            .take()
            .ok_or("Missing opening player")?
            .prepare_reference()?;
        player.clip("EntranceRope1760")?;
        let mut actors = vec![];
        for a in &mut self.actors {
            if !a.height.is_finite()
                || a.height < 0.
                || a.height > 200.
                || a.position.iter().any(|v| !v.is_finite() || v.abs() > 1e7)
                || a.sounds.iter().any(|n| n.sound >= sounds)
                || a.extent
                    .is_some_and(|e| e.iter().any(|v| !v.is_finite() || *v <= 0.))
            {
                return Err("Invalid opening actor".into());
            }
            let mesh = a
                .mesh
                .take()
                .ok_or("Missing opening actor mesh")?
                .prepare_reference()?;
            mesh.clip(&a.idle)?;
            let base = scene.textures.len();
            scene
                .textures
                .extend(mesh.mesh.materials.iter().map(|m| rc_render::Texture {
                    width: m.width,
                    height: m.height,
                    pixels: m.pixels.clone(),
                }));
            actors.push((mesh, base));
        }
        for p in &self.scripts {
            if p.actions.len() > 64 {
                return Err("Opening lane exceeds action budget".into());
            }
            for a in &p.actions {
                if !a.seconds.is_finite()
                    || a.seconds < 0.
                    || a.seconds > 120.
                    || ![
                        "wait",
                        "timer",
                        "emit",
                        "sound",
                        "play",
                        "join",
                        "unlock",
                        "objective",
                        "active",
                    ]
                    .contains(&a.op.as_str())
                {
                    return Err("Invalid opening opcode/timer".into());
                }
                if a.op == "play" {
                    if a.role == 0 {
                        player.clip(&a.text)?;
                    } else {
                        actors
                            .get(a.role - 1)
                            .ok_or("Invalid opening animation role")?
                            .0
                            .clip(&a.text)?;
                    }
                }
                if a.op == "sound" && a.role >= sounds {
                    return Err("Invalid opening sound role".into());
                }
                if ["objective", "active"].contains(&a.op.as_str()) && a.role >= 7 {
                    return Err("Invalid opening objective".into());
                }
            }
        }
        Ok(Prepared {
            planning: None,
            assets: self,
            player,
            actors,
        })
    }
}
impl Prepared {
    pub fn state(&self, start: Point) -> State {
        let mut animations = vec![Animation {
            random: None,
            clip: String::new(),
            playback: empty_playback(),
            finished: true,
            visible: false,
            blocked: 0,
            position: start,
            notifies: vec![],
            root: false,
        }];
        animations.extend(self.assets.actors.iter().map(|a| Animation {
            random: None,
            clip: a.idle.clone(),
            playback: empty_playback(),
            finished: true,
            visible: true,
            blocked: 0,
            position: a.position,
            notifies: vec![],
            root: false,
        }));
        let a = &self.assets.actors[2];
        State {
            planning: None,
            clock: 0.,
            locked: true,
            joined: false,
            objectives: [false; 7],
            active: 1,
            events: std::collections::BTreeSet::from(["levelstart".into()]),
            event_ticks: std::collections::BTreeMap::from([("levelstart".into(), 0)]),
            lanes: vec![Lane::default(); self.assets.scripts.len()],
            animations,
            sev: Companion {
                joined: false,
                health: 100,
                shields: 100.,
                down: false,
                position: a.position,
                yaw: a.rotation[1] as f64 * std::f64::consts::TAU / 65536.,
                moving: false,
            },
            sev_body: rc_package::physics::BodyState {
                position: a.position,
                velocity: [0.; 3],
                grounded: false,
            },
            subtitle: String::new(),
            subtitle_until: 0.,
            finished_tick: None,
        }
    }
}
pub(crate) fn empty_playback() -> ChannelPlaybackSnapshot {
    ChannelPlaybackSnapshot {
        channels: vec![],
        byte_60: 1,
        byte_61: 0,
    }
}
pub(crate) fn begin(mesh: &PreparedMesh, s: &mut Animation, name: &str) -> Result<(), String> {
    let clip = mesh.clip(name)?;
    let seq = &mesh.mesh.clips[clip].sequence;
    let mut channel = mesh.channel(clip, 0.)?;
    channel.words[0] = 99;
    s.playback = empty_playback();
    s.playback.channels.push(channel);
    play_channel(
        &mut s.playback,
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
    s.clip = name.into();
    s.finished = false;
    s.root = seq.flags & 1 != 0;
    s.visible = true;
    Ok(())
}
pub(crate) fn tick_animation(
    mesh: &PreparedMesh,
    s: &mut Animation,
    tick: u64,
) -> Result<Vec<u32>, String> {
    if s.finished {
        return Ok(vec![]);
    }
    let clip = mesh.clip(&s.clip)?;
    let seq = &mesh.mesh.clips[clip].sequence;
    let mut host = crate::rappel::TickHost {
        random: if f32::from_bits(seq.word_18) > 0. {
            Some(s.random.unwrap_or(1))
        } else {
            s.random
        },
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
        &mut s.playback.channels,
        &mut s.playback.byte_61,
        STEP as f32,
        false,
        &mut host,
    )?;
    s.random = host.random;
    s.finished = host.ended;
    for id in &host.events {
        s.notifies.push((tick, *id));
    }
    Ok(host.events)
}
pub(crate) fn anim_frame(s: &Animation, mesh: &PreparedMesh) -> Result<f32, String> {
    let seq = &mesh.mesh.clips[mesh.clip(&s.clip)?].sequence;
    Ok(s.playback
        .channels
        .first()
        .map_or(0., |c| f32::from_bits(c.words[7]))
        .clamp(0., 1. - 1. / seq.frames as f32))
}
pub(crate) fn authored(
    mesh: &PreparedMesh,
    s: &Animation,
    at: Point,
    rotation: [i32; 3],
) -> Result<Point, String> {
    let clip = mesh.clip(&s.clip)?;
    let root = mesh.root_matrix(clip, anim_frame(s, mesh)?)?;
    let anchor =
        rc_package::mesh_transform::rotation_matrix(rotation, at.map(|v| (v as f32).to_bits()))?
            .concat()
            .try_into()
            .unwrap();
    let result = rc_package::root_motion::compose_relative(root, anchor)?;
    Ok(std::array::from_fn(|i| {
        f32::from_bits(result[12 + i]) as f64
    }))
}
impl Game {
    pub(crate) fn map3_locked(&self) -> bool {
        self.state
            .map
            .as_ref()
            .and_then(|m| m.opening.as_ref())
            .is_some_and(|s| s.locked)
    }
    pub(crate) fn clamp_map3_view(&mut self) {
        if !self.map3_locked() {
            return;
        }
        let a = &self.map.as_ref().unwrap().opening.as_ref().unwrap().assets;
        let yaw = (self.state.yaw + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI;
        self.state.yaw = yaw.clamp(a.limits[2], a.limits[3]);
        self.state.pitch = self.state.pitch.clamp(a.limits[0], a.limits[1]);
    }
    pub(crate) fn map3_opening_tick(&mut self) -> Result<bool, String> {
        let Some(p) = self.map.as_ref().and_then(|m| m.opening.as_ref()) else {
            return Ok(false);
        };
        let m = self.state.map.as_mut().unwrap();
        let s = m.opening.as_mut().unwrap();
        let tick = self.state.statistics.ticks + 1;
        s.clock += STEP;
        for (i, program) in p.assets.scripts.iter().enumerate() {
            let lane = &mut s.lanes[i];
            if lane.remaining > 0. {
                lane.remaining = (lane.remaining - STEP).max(0.);
                if lane.remaining > 1e-8 {
                    continue;
                }
                lane.remaining = 0.;
            }
            if let Some(role) = lane.waiting {
                if !s.animations[role].finished {
                    continue;
                }
                lane.waiting = None;
            }
            for _ in 0..64 {
                let Some(a) = program.actions.get(lane.next) else {
                    break;
                };
                match a.op.as_str() {
                    "wait" => {
                        if !s.events.contains(&a.text) {
                            break;
                        }
                    }
                    "emit" => {
                        s.events.insert(a.text.clone());
                        s.event_ticks.entry(a.text.clone()).or_insert(tick);
                    }
                    "timer" => lane.remaining = a.seconds,
                    "sound" => {
                        queue_audio(m.audio_sequence + 1, tick, a.role, &mut m.events);
                        m.audio_sequence += 1;
                        lane.remaining = a.seconds;
                        s.subtitle = a.text.clone();
                        s.subtitle_until = s.clock + a.seconds;
                    }
                    "play" => {
                        let mesh = if a.role == 0 {
                            &p.player
                        } else {
                            &p.actors[a.role - 1].0
                        };
                        begin(mesh, &mut s.animations[a.role], &a.text)?;
                        if a.seconds > 0. {
                            lane.waiting = Some(a.role);
                        }
                    }
                    "join" => {
                        s.joined = true;
                        s.sev.joined = true;
                    }
                    "unlock" => {
                        s.events.insert("EndBriefingRequested".into());
                    }
                    "objective" => s.objectives[a.role] = true,
                    "active" => s.active = a.role,
                    _ => unreachable!(),
                }
                lane.next += 1;
                if lane.remaining > 0. || lane.waiting.is_some() {
                    break;
                }
            }
        }
        for role in 0..s.animations.len() {
            let mesh = if role == 0 {
                &p.player
            } else {
                &p.actors[role - 1].0
            };
            let a = &mut s.animations[role];
            if a.finished {
                continue;
            }
            let was_finished = a.finished;
            let events = tick_animation(mesh, a, tick)?;
            for id in events {
                if let Some(sound) = (if role == 0 {
                    &p.assets.player_sounds
                } else {
                    &p.assets.actors[role - 1].sounds
                })
                .iter()
                .find(|n| n.original == id)
                .map(|n| n.sound)
                {
                    queue_audio(m.audio_sequence + 1, tick, sound, &mut m.events);
                    m.audio_sequence += 1;
                }
            }
            if a.root || role == 0 {
                let (at, rotation, extent) = if role == 0 {
                    (
                        p.assets.start,
                        p.assets.rotation,
                        Some(self.profile.physics.extent),
                    )
                } else {
                    let b = &p.assets.actors[role - 1];
                    (b.position, b.rotation, b.extent)
                };
                let to = authored(mesh, a, at, rotation)?;
                let from = if role == 0 {
                    self.state.pawn.body.position
                } else {
                    a.position
                };
                let fraction = if let Some(extent) = extent {
                    let q = self.world.sweep_motion(from, to, extent)?;
                    if !q.complete {
                        return Err("Incomplete third-map authored body sweep".into());
                    }
                    q.contacts.first().map_or(1., |c| c.hit.fraction.max(0.))
                } else {
                    1.
                };
                a.blocked += u32::from(fraction < 1.);
                a.position = std::array::from_fn(|i| from[i] + (to[i] - from[i]) * fraction);
                if role == 0 {
                    self.state.pawn.body.position = a.position;
                    self.state.pawn.body.velocity = [0.; 3];
                }
                if role == 3 {
                    s.sev_body.position = a.position;
                    s.sev_body.velocity = [0.; 3];
                    s.sev_body.grounded = false;
                }
            }
            if !was_finished
                && a.finished
                && role > 0
                && p.assets.actors[role - 1].remove_after_intro
            {
                a.visible = false;
            }
        }
        if let Some(c) = &mut m.companion {
            if s.locked {
                c.position = s.animations[1].position;
            } else {
                s.animations[1].position = c.position;
            }
        }
        if let Some(c) = &mut m.fixer {
            if s.locked {
                c.position = s.animations[2].position;
            } else {
                s.animations[2].position = c.position;
            }
        }
        if (!s.animations[3].finished && s.animations[3].root) || !s.joined {
            s.sev.position = s.animations[3].position;
        } else {
            s.animations[3].position = s.sev.position;
        }
        if s.events.contains("Dialog62") && s.animations[3].finished && !s.sev_body.grounded {
            let frame = self.world.body_tick(
                &s.sev_body,
                STEP,
                rc_package::physics::PhysicsOptions {
                    extent: crate::squad::EXTENT,
                    gravity: 950.,
                    terminal_speed: 4000.,
                    skin: 0.5,
                    support_distance: 35.,
                    minimum_up: 0.7,
                    max_iterations: 8,
                    step_height: 35.,
                },
            )?;
            s.sev_body = frame.body;
            s.sev.position = frame.body.position;
            s.animations[3].position = frame.body.position;
        } else if s.joined && s.sev_body.grounded {
            s.sev_body.position = s.sev.position;
        }
        let locked = s.locked;
        if s.events.contains("EndBriefingRequested") && s.animations[0].finished {
            s.locked = false;
            s.finished_tick.get_or_insert(tick);
        }
        Ok(locked)
    }
    pub(crate) fn map3_status(&self) -> Option<String> {
        if let Some(s) = self.map3_breach_status() {
            return Some(s);
        }
        if let Some(status) = self.map3_guard_status() {
            return Some(status);
        }
        let s = self.state.map.as_ref()?.opening.as_ref()?;
        Some(if s.locked {
            "GEO_01C · Abseilen und Sev-Begegnung".into()
        } else if !s.objectives[1] {
            "GEO_01C · Rendezvous mit Zero-Seven".into()
        } else if s.planning.is_some() && !s.objectives[2] {
            "GEO_01C · Sev beigetreten · Strategieraumglas zerschiessen".into()
        } else if let Some(e) = s.planning.as_ref().and_then(|p| p.escape.as_ref()) {
            if e.trigger_tick.is_some() {
                "GEO_01C · Wachraum erreicht".into()
            } else if e.stage >= 3 {
                "GEO_01C · Sun Fac hinter dem Schild · Wachraum erreichen".into()
            } else {
                "GEO_01C · Sun Fac flieht durch die Wachtuer".into()
            }
        } else if s.planning.is_some() {
            "GEO_01C · Strategieraumglas zerstoert · Sun Fac verfolgen".into()
        } else {
            "GEO_01C · Sev beigetreten · Sun Fac im Strategieraum".into()
        })
    }
    pub(crate) fn map3_geometry(
        &self,
    ) -> Result<
        (
            Vec<rc_package::geometry::Triangle>,
            Vec<Option<rc_render::FaceTexture>>,
        ),
        String,
    > {
        let Some(p) = self.map.as_ref().and_then(|m| m.opening.as_ref()) else {
            return Ok((vec![], vec![]));
        };
        let m = self.state.map.as_ref().unwrap();
        let s = m.opening.as_ref().unwrap();
        let mut ts = vec![];
        let mut fs = vec![];
        for (i, (mesh, base)) in p.actors.iter().enumerate() {
            let actor = &p.assets.actors[i];
            let state = &s.animations[i + 1];
            if !state.visible || (i < 2 && !s.locked) || (i == 2 && self.map3_breach_playing()) {
                continue;
            }
            if i == 0 && m.companion.as_ref().is_some_and(|c| !c.joined || c.down) {
                continue;
            }
            if i == 1 && m.fixer.as_ref().is_some_and(|c| !c.joined || c.down) {
                continue;
            }
            let (clip, frame) = if let Some(pose) = if i == 5 {
                self.map3_finale_sun_pose()?
                    .or(self.map3_balcony_sun_pose()?)
            } else if i == 2 {
                self.map3_finale_sev_pose()?
            } else {
                None
            } {
                pose
            } else if state.finished {
                let escaping = i == 5
                    && s.planning
                        .as_ref()
                        .and_then(|p| p.escape.as_ref())
                        .is_some_and(|e| e.moving);
                let index = mesh.clip(if escaping { "RunForward" } else { &actor.idle })?;
                let seq = &mesh.mesh.clips[index].sequence;
                (
                    index,
                    (s.clock * f32::from_bits(seq.rate_bits) as f64 / seq.frames as f64)
                        .rem_euclid(1.) as f32,
                )
            } else {
                (mesh.clip(&state.clip)?, anim_frame(state, mesh)?)
            };
            let channel = mesh.channel(clip, frame)?;
            let lock = mesh.mesh.clips[clip].sequence.flags & 1 != 0;
            let mut g = mesh.geometry_with_root(&channel, lock)?;
            let mut at = state.position;
            if actor.height > 0. {
                at[2] -= actor.height + mesh.rest_bounds[0][2] as f64;
            }
            let rotation = if i == 2 && s.joined {
                [0, (s.sev.yaw * 65536. / std::f64::consts::TAU) as i32, 0]
            } else if i == 5 {
                s.planning
                    .as_ref()
                    .and_then(|p| p.escape.as_ref())
                    .map_or(actor.rotation, |e| {
                        [0, (e.yaw * 65536. / std::f64::consts::TAU) as i32, 0]
                    })
            } else {
                actor.rotation
            };
            for t in &mut g.triangles {
                t.points = t.points.map(|v| {
                    rc_package::mesh::transform_point(
                        v,
                        at.map(|v| v as f32),
                        rotation,
                        [1.; 3],
                        [0.; 3],
                    )
                });
            }
            for f in g.appearance.iter_mut().flatten() {
                f.texture += *base;
            }
            if actor.armed {
                let map = self.map.as_ref().unwrap();
                let (weapon, weapon_texture) = if i == 2 {
                    self.map3_finale_weapon()
                        .unwrap_or((&map.weapon, map.weapon_texture))
                } else {
                    (&map.weapon, map.weapon_texture)
                };
                let transform = mesh.attachment_transform(&channel, "weaponAttach_R", lock)?;
                for t in &weapon.triangles {
                    let mut t = t.clone();
                    t.points = t.points.map(|v| {
                        let v = std::array::from_fn::<_, 3, _>(|j| {
                            v[0] * transform[0][j]
                                + v[1] * transform[1][j]
                                + v[2] * transform[2][j]
                                + transform[3][j]
                        });
                        rc_package::mesh::transform_point(
                            v,
                            at.map(|v| v as f32),
                            rotation,
                            [1.; 3],
                            [0.; 3],
                        )
                    });
                    g.triangles.push(t);
                }
                g.appearance.extend(weapon.appearance.iter().map(|f| {
                    (*f).map(|mut f| {
                        f.texture += weapon_texture;
                        f
                    })
                }));
            }
            ts.extend(g.triangles);
            fs.extend(g.appearance);
        }
        Ok((ts, fs))
    }
}
pub(crate) fn queue_audio(
    sequence: u64,
    tick: u64,
    sound: usize,
    events: &mut Vec<crate::combat::Event>,
) {
    if events.len() >= 256 {
        events.remove(0);
    }
    events.push(crate::combat::Event {
        sequence,
        tick,
        sound,
        owner: -1,
    });
}
