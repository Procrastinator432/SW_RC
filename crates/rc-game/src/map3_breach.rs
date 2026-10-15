//! Selected GEO_01C healing, walking graph and three-role door-breach host.
use crate::{
    map3_escape::{translated, Door, PreparedDoor},
    map3_opening::{self, Animation, Notify},
    visual::{Mesh, PreparedMesh},
    Game, Input, Point, STEP,
};
use rc_package::{
    hull::{Hull, HullSet},
    world_collision::{BodyActor, BodyShape},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Station {
    pub original: u32,
    pub actor: String,
    pub position: Point,
    pub rotation: [i32; 3],
    pub floor: f64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balcony: Option<Box<crate::map3_balcony::Assets>>,
    pub stations: Vec<Station>,
    pub bacta_mesh: Option<Mesh>,
    pub heal_duration: f64,
    pub heal_range: f64,
    pub navigation: crate::navigation::Assets,
    pub spots: [Point; 3],
    pub rotations: [[i32; 3]; 3],
    pub clips: Vec<String>,
    pub sounds: Vec<Vec<Notify>>,
    pub doors: Vec<Door>,
    pub move_time: f64,
    pub provenance: serde_json::Value,
}
pub struct Prepared {
    pub balcony: Option<Box<crate::map3_balcony::Prepared>>,
    pub assets: Assets,
    pub bacta: PreparedMesh,
    pub texture: usize,
    pub doors: Vec<PreparedDoor>,
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct State {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balcony: Option<Box<crate::map3_balcony::State>>,
    pub healing: Option<usize>,
    pub heal_fraction: f64,
    pub healed: [u32; 2],
    pub agents: [crate::navigation::Agent; 3],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settling: Option<[rc_package::physics::BodyState; 3]>,
    pub stage: u8,
    pub animations: Vec<Animation>,
    pub anchors: [Point; 3],
    pub door_fraction: f64,
    pub command_tick: Option<u64>,
    pub animation_tick: Option<u64>,
    pub opened_tick: Option<u64>,
    pub finished_tick: Option<u64>,
}
impl Game {
    pub(crate) fn prepare_map3_breach(&mut self) -> Result<(), String> {
        let Some(mut a) = self
            .map
            .as_mut()
            .and_then(|m| m.opening.as_mut())
            .and_then(|p| p.planning.as_mut())
            .and_then(|p| p.guard.as_mut())
            .and_then(|p| p.assets.breach.take())
        else {
            return Ok(());
        };
        if a.stations.iter().map(|s| s.original).ne([310, 261])
            || a.doors.iter().map(|d| d.original).ne([120, 121])
            || a.clips.len() != 3
            || a.sounds.len() != 3
            || !a.heal_duration.is_finite()
            || !(0.1..=60.).contains(&a.heal_duration)
            || !a.heal_range.is_finite()
            || !(1. ..=500.).contains(&a.heal_range)
            || !a.move_time.is_finite()
            || !(0.01..=10.).contains(&a.move_time)
        {
            return Err("Invalid C breach/healing bindings".into());
        }
        a.navigation.validate()?;
        if a.spots
            .iter()
            .flatten()
            .chain(a.stations.iter().flat_map(|s| s.position.iter()))
            .any(|v| !v.is_finite() || v.abs() > 1e7)
            || a.stations.iter().any(|s| !s.floor.is_finite())
            || a.sounds
                .iter()
                .flatten()
                .any(|n| n.sound >= self.map.as_ref().unwrap().assets.sounds.len())
        {
            return Err("Invalid C breach points/sounds".into());
        }
        let bacta = a
            .bacta_mesh
            .take()
            .ok_or("Missing original Bacta mesh")?
            .prepare_reference()?;
        bacta.clip("Holster")?;
        bacta.clip("Load")?;
        let texture = self.scene.textures.len();
        self.scene
            .textures
            .extend(bacta.mesh.materials.iter().map(|m| rc_render::Texture {
                width: m.width,
                height: m.height,
                pixels: m.pixels.clone(),
            }));
        for s in &a.stations {
            if !self.world.actors.iter().any(|x| x.name == s.actor) {
                self.world.actors.push(BodyActor {
                    name: s.actor.clone(),
                    shape: BodyShape::Hulls(Arc::new(HullSet {
                        hulls: vec![Hull {
                            offset: 0,
                            planes: vec![],
                            bounds: [
                                [s.position[0] - 40., s.position[1] - 40., s.floor],
                                [s.position[0] + 40., s.position[1] + 40., s.floor + 240.],
                            ],
                        }],
                        missing_solid_leaves: 0,
                        empty_root_solid: false,
                    })),
                });
            }
        }
        let mut doors = vec![];
        for d in &a.doors {
            if d.offset.iter().any(|v| !v.is_finite() || v.abs() > 1000.)
                || d.snapshot.len() > 2 * 1024 * 1024
            {
                return Err("Invalid breach door".into());
            }
            let collider = self
                .world
                .actors
                .iter()
                .enumerate()
                .find_map(|(i, x)| {
                    if x.name == d.actor {
                        if let BodyShape::Hulls(h) = &x.shape {
                            Some((i, h.clone()))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .ok_or("Missing breach door collider")?;
            let scene = rc_render::Scene::from_snapshot(&d.snapshot)?;
            let texture = self.scene.textures.len();
            self.scene.textures.extend(scene.textures.iter().cloned());
            doors.push(PreparedDoor {
                scene,
                texture,
                collider,
            });
        }
        let p = self.map.as_ref().unwrap();
        for (i, mesh) in [
            &p.squad,
            &p.fixer.as_ref().unwrap().0,
            &p.opening.as_ref().unwrap().actors[2].0,
        ]
        .iter()
        .enumerate()
        {
            mesh.clip(&a.clips[i])?;
        }
        let animations = a
            .spots
            .iter()
            .enumerate()
            .map(|(i, at)| Animation {
                random: None,
                clip: a.clips[i].clone(),
                playback: map3_opening::empty_playback(),
                finished: true,
                visible: false,
                blocked: 0,
                position: *at,
                notifies: vec![],
                root: false,
            })
            .collect();
        let state = State {
            balcony: None,
            settling: None,
            healing: None,
            heal_fraction: 0.,
            healed: [0; 2],
            agents: std::array::from_fn(|_| crate::navigation::Agent::default()),
            stage: 0,
            animations,
            anchors: a.spots,
            door_fraction: 0.,
            command_tick: None,
            animation_tick: None,
            opened_tick: None,
            finished_tick: None,
        };
        self.state
            .map
            .as_mut()
            .unwrap()
            .opening
            .as_mut()
            .unwrap()
            .planning
            .as_mut()
            .unwrap()
            .guard
            .as_mut()
            .unwrap()
            .breach = Some(Box::new(state));
        self.map
            .as_mut()
            .unwrap()
            .opening
            .as_mut()
            .unwrap()
            .planning
            .as_mut()
            .unwrap()
            .guard
            .as_mut()
            .unwrap()
            .breach = Some(Box::new(Prepared {
            balcony: None,
            assets: *a,
            bacta,
            texture,
            doors,
        }));
        Ok(())
    }
    pub(crate) fn breach_prepared(&self) -> Option<&Prepared> {
        self.map
            .as_ref()?
            .opening
            .as_ref()?
            .planning
            .as_ref()?
            .guard
            .as_ref()?
            .breach
            .as_deref()
    }
    pub(crate) fn breach_state(&self) -> Option<&State> {
        self.state
            .map
            .as_ref()?
            .opening
            .as_ref()?
            .planning
            .as_ref()?
            .guard
            .as_ref()?
            .breach
            .as_deref()
    }
    pub(crate) fn map3_healing(&self) -> bool {
        self.breach_state().is_some_and(|s| s.healing.is_some())
    }
    pub(crate) fn map3_breach_playing(&self) -> bool {
        self.breach_state().is_some_and(|s| s.stage == 2)
    }
    pub(crate) fn sync_map3_breach(&mut self) {
        let Some(p) = self.breach_prepared() else {
            return;
        };
        let fraction = self.breach_state().unwrap().door_fraction;
        let shapes: Vec<_> = p
            .doors
            .iter()
            .zip(&p.assets.doors)
            .map(|(d, a)| {
                (
                    d.collider.0,
                    Arc::new(translated(&d.collider.1, a.offset.map(|v| v * fraction))),
                )
            })
            .collect();
        for (i, h) in shapes {
            self.world.actors[i].shape = BodyShape::Hulls(h);
        }
    }
    pub(crate) fn map3_breach_tick(&mut self, input: Input) -> Result<(), String> {
        let Some(p) = self.map.as_ref() else {
            return Ok(());
        };
        let Some(b) = p
            .opening
            .as_ref()
            .and_then(|p| p.planning.as_ref())
            .and_then(|p| p.guard.as_ref())
            .and_then(|p| p.breach.as_ref())
        else {
            return Ok(());
        };
        if self.map3_locked() {
            return Ok(());
        }
        let player = self.state.pawn.body.position;
        let eye = self.eye();
        let mut station = None;
        if input.use_action && self.state.health > 0 && self.state.health < 100 {
            for (i, s) in b.assets.stations.iter().enumerate() {
                if crate::distance(player, s.position) < b.assets.heal_range {
                    let q = self.world.sweep_motion(eye, s.position, [0.; 3])?;
                    if !q.complete {
                        return Err("Incomplete Bacta visibility".into());
                    }
                    if q.contacts.first().is_none_or(|c| c.object == s.actor) {
                        station = Some(i);
                        break;
                    }
                }
            }
        }
        let usable = if input.use_action && crate::distance(player, b.assets.spots[0]) < 256. {
            let q = self.world.sweep_motion(eye, b.assets.spots[0], [0.; 3])?;
            if !q.complete {
                return Err("Incomplete breach command visibility".into());
            }
            q.contacts.is_empty()
        } else {
            false
        };
        let m = self.state.map.as_mut().unwrap();
        let o = m.opening.as_mut().unwrap();
        let s = o
            .planning
            .as_mut()
            .unwrap()
            .guard
            .as_mut()
            .unwrap()
            .breach
            .as_mut()
            .unwrap();
        s.healing = station;
        if let Some(i) = station {
            s.heal_fraction += STEP * 100. / b.assets.heal_duration;
            let gain = (s.heal_fraction + 1e-8).floor() as u32;
            let gain = gain.min(100 - self.state.health);
            self.state.health += gain;
            s.healed[i] += gain;
            s.heal_fraction -= gain as f64;
        } else {
            s.heal_fraction = 0.;
        }
        let tick = self.state.statistics.ticks + 1;
        if s.stage == 0 && o.objectives[3] && usable && station.is_none() {
            s.stage = 1;
            s.command_tick = Some(tick);
            for a in &mut s.agents {
                a.route.clear();
                a.next_plan = 0.;
            }
        }
        let mut members = [
            m.companion.as_mut().unwrap(),
            m.fixer.as_mut().unwrap(),
            &mut o.sev,
        ];
        if s.stage == 1
            && members.iter().enumerate().all(|(i, c)| {
                c.joined && !c.down && crate::distance(c.position, b.assets.spots[i]) < 8.
            })
        {
            s.stage = 2;
            s.animation_tick = Some(tick);
            o.events.insert("CloneDoorBreach01".into());
            o.events.insert("DisableDoorBreachDialog".into());
            o.event_ticks.insert("CloneDoorBreach01".into(), tick);
            for (i, c) in members.iter().enumerate() {
                s.anchors[i] = c.position;
                let mesh = if i == 0 {
                    &p.squad
                } else if i == 1 {
                    &p.fixer.as_ref().unwrap().0
                } else {
                    &p.opening.as_ref().unwrap().actors[2].0
                };
                map3_opening::begin(mesh, &mut s.animations[i], &b.assets.clips[i])?;
            }
        }
        if s.stage == 2 {
            for (i, c) in members.iter_mut().enumerate() {
                let mesh = if i == 0 {
                    &p.squad
                } else if i == 1 {
                    &p.fixer.as_ref().unwrap().0
                } else {
                    &p.opening.as_ref().unwrap().actors[2].0
                };
                let a = &mut s.animations[i];
                for id in map3_opening::tick_animation(mesh, a, tick)? {
                    if let Some(sound) = b.assets.sounds[i]
                        .iter()
                        .find(|n| n.original == id)
                        .map(|n| n.sound)
                    {
                        m.audio_sequence += 1;
                        map3_opening::queue_audio(m.audio_sequence, tick, sound, &mut m.events);
                    }
                    let event = if id == 309 {
                        Some("MDBDestroySwitch")
                    } else if id == 306 {
                        Some("MDBtossGrenade")
                    } else {
                        None
                    };
                    if id == 309 {
                        o.events.insert("LandingPadDoorBreached".into());
                        o.event_ticks
                            .entry("LandingPadDoorBreached".into())
                            .or_insert(tick);
                    }
                    if let Some(event) = event {
                        o.events.insert(event.into());
                        o.event_ticks.entry(event.into()).or_insert(tick);
                    }
                }
                let to = map3_opening::authored(mesh, a, s.anchors[i], b.assets.rotations[i])?;
                let from = c.position;
                let body = rc_package::physics::BodyState {
                    position: from,
                    velocity: [(to[0] - from[0]) / STEP, (to[1] - from[1]) / STEP, 0.],
                    grounded: true,
                };
                let frame = self.world.body_tick(
                    &body,
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
                c.position = frame.body.position;
                a.blocked += u32::from((to[0] - c.position[0]).hypot(to[1] - c.position[1]) > 0.5);
                a.position = c.position;
                c.yaw = b.assets.rotations[i][1] as f64 * std::f64::consts::TAU / 65536.;
                c.moving = false;
            }
            if s.animations.iter().all(|a| a.finished) {
                s.stage = 3;
                s.finished_tick = Some(tick);
                s.settling = Some(std::array::from_fn(|i| rc_package::physics::BodyState {
                    position: members[i].position,
                    velocity: [0.; 3],
                    grounded: false,
                }));
            }
        }
        if let Some(bodies) = &mut s.settling {
            for (i, body) in bodies.iter_mut().enumerate() {
                if !body.grounded {
                    let frame = self.world.body_tick(
                        body,
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
                    *body = frame.body;
                    members[i].position = body.position;
                }
            }
            if bodies.iter().all(|body| body.grounded) {
                s.settling = None;
            }
        }
        if o.events.contains("MDBDestroySwitch") {
            if o.events.insert("SunFacRoomDoorOpening".into()) {
                o.event_ticks.insert("SunFacRoomDoorOpening".into(), tick);
            }
            s.door_fraction = (s.door_fraction + STEP / b.assets.move_time).min(1.);
            if s.door_fraction >= 1. && s.opened_tick.is_none() {
                s.opened_tick = Some(tick);
                o.events.insert("SunFacRoomDoorOpened".into());
                o.event_ticks.insert("SunFacRoomDoorOpened".into(), tick);
            }
        }
        self.sync_map3_breach();
        Ok(())
    }
    pub(crate) fn map3_breach_follow(&mut self) -> Result<bool, String> {
        let Some(p) = self
            .map
            .as_ref()
            .and_then(|m| m.opening.as_ref())
            .and_then(|p| p.planning.as_ref())
            .and_then(|p| p.guard.as_ref())
            .and_then(|p| p.breach.as_ref())
        else {
            return Ok(false);
        };
        let sev_controlled = self.map3_balcony_sev_controlled();
        let target = self.state.pawn.body.position;
        let now = self.state.statistics.ticks as f64 * STEP;
        let m = self.state.map.as_mut().unwrap();
        let o = m.opening.as_mut().unwrap();
        let s = o
            .planning
            .as_mut()
            .unwrap()
            .guard
            .as_mut()
            .unwrap()
            .breach
            .as_mut()
            .unwrap();
        if s.stage == 2 || s.settling.is_some() {
            return Ok(true);
        }
        let members = [
            m.companion.as_mut().unwrap(),
            m.fixer.as_mut().unwrap(),
            &mut o.sev,
        ];
        let mut previous = None;
        for (i, c) in members.into_iter().enumerate() {
            if i == 2 && sev_controlled {
                continue;
            }
            let from = c.position;
            let goal = if s.stage == 1 {
                p.assets.spots[i]
            } else {
                target
            };
            crate::map_gameplay::follow_to(
                &self.world,
                &p.assets.navigation,
                c,
                &mut s.agents[i],
                goal,
                now,
                450.,
                if s.stage == 1 { 6. } else { 180. },
            )?;
            if previous.is_some_and(|at| crate::distance(at, c.position) < 80.) {
                c.position = from;
                c.moving = false;
            }
            previous = Some(c.position);
        }
        Ok(true)
    }
    pub(crate) fn map3_breach_status(&self) -> Option<String> {
        let s = self.breach_state()?;
        if s.healing.is_some() {
            return Some(format!("GEO_01C · Bacta · {} / 100", self.state.health));
        }
        let p = self.breach_prepared()?;
        if !self.map3_locked()
            && self.state.health < 100
            && p.assets.stations.iter().any(|station| {
                crate::distance(self.state.pawn.body.position, station.position)
                    < p.assets.heal_range
                    && self
                        .world
                        .sweep_motion(self.eye(), station.position, [0.; 3])
                        .is_ok_and(|q| {
                            q.complete
                                && q.contacts.first().is_none_or(|c| c.object == station.actor)
                        })
            })
        {
            return Some("GEO_01C · BENUTZEN halten: Bacta-Heilung".into());
        }
        if s.stage == 3 {
            if let Some(status) = self.map3_finale_status() {
                return Some(status);
            }
            if let Some(status) = self.map3_balcony_status() {
                return Some(status);
            }
        }
        if s.stage == 0
            && self.state.map.as_ref()?.opening.as_ref()?.objectives[3]
            && crate::distance(self.state.pawn.body.position, p.assets.spots[0]) < 256.
            && self
                .world
                .sweep_motion(self.eye(), p.assets.spots[0], [0.; 3])
                .is_ok_and(|q| q.complete && q.contacts.is_empty())
        {
            return Some("GEO_01C · BENUTZEN: Squad-Tuerbrechmanoever".into());
        }
        Some(
            match s.stage {
                1 => "GEO_01C · Squad bezieht Tuerbrechpositionen",
                2 => "GEO_01C · Tuerbrechmanoever",
                3 => "GEO_01C · Tuer offen · Sun Fac verfolgen",
                _ => return None,
            }
            .into(),
        )
    }
    pub(crate) fn map3_breach_target(&self) -> Option<Point> {
        let s = self.breach_state()?;
        let p = self.breach_prepared()?;
        if let Some(i) = s.healing {
            return Some(p.assets.stations[i].position);
        }
        if s.stage > 0 && s.stage < 3 {
            Some(p.assets.spots[0])
        } else {
            None
        }
    }
    pub(crate) fn map3_breach_geometry(
        &self,
    ) -> Result<
        (
            Vec<rc_package::geometry::Triangle>,
            Vec<Option<rc_render::FaceTexture>>,
        ),
        String,
    > {
        let Some(b) = self.breach_prepared() else {
            return Ok((vec![], vec![]));
        };
        let s = self.breach_state().unwrap();
        let p = self.map.as_ref().unwrap();
        let mut ts = vec![];
        let mut fs = vec![];
        for (i, station) in b.assets.stations.iter().enumerate() {
            let clip = b.bacta.clip(if s.healing == Some(i) {
                "Load"
            } else {
                "Holster"
            })?;
            let channel = b.bacta.channel(clip, 0.)?;
            let mut g = b.bacta.geometry_with_root(&channel, true)?;
            for t in &mut g.triangles {
                t.points = t.points.map(|v| {
                    rc_package::mesh::transform_point(
                        v,
                        [
                            station.position[0] as f32,
                            station.position[1] as f32,
                            station.floor as f32 - b.bacta.rest_bounds[0][2],
                        ],
                        station.rotation,
                        [1.; 3],
                        [0.; 3],
                    )
                });
            }
            ts.extend(g.triangles);
            fs.extend(g.appearance.into_iter().map(|f| {
                f.map(|mut f| {
                    f.texture += b.texture;
                    f
                })
            }));
        }
        for (d, a) in b.doors.iter().zip(&b.assets.doors) {
            ts.extend(d.scene.triangles.iter().cloned().map(|mut t| {
                t.points = t.points.map(|v| {
                    std::array::from_fn(|i| v[i] + (a.offset[i] * s.door_fraction) as f32)
                });
                t
            }));
            fs.extend(d.scene.appearance.iter().map(|f| {
                f.map(|mut f| {
                    f.texture += d.texture;
                    f
                })
            }));
        }
        if s.stage == 2 {
            for (i, a) in s.animations.iter().enumerate() {
                let (mesh, base) = if i == 0 {
                    (&p.squad, p.texture_base)
                } else if i == 1 {
                    (&p.fixer.as_ref().unwrap().0, p.fixer.as_ref().unwrap().1)
                } else {
                    (
                        &p.opening.as_ref().unwrap().actors[2].0,
                        p.opening.as_ref().unwrap().actors[2].1,
                    )
                };
                let channel =
                    mesh.channel(mesh.clip(&a.clip)?, map3_opening::anim_frame(a, mesh)?)?;
                let mut g = mesh.geometry_with_root(&channel, true)?;
                for t in &mut g.triangles {
                    t.points = t.points.map(|v| {
                        rc_package::mesh::transform_point(
                            v,
                            [
                                a.position[0] as f32,
                                a.position[1] as f32,
                                a.position[2] as f32 - 84. - mesh.rest_bounds[0][2],
                            ],
                            b.assets.rotations[i],
                            [1.; 3],
                            [0.; 3],
                        )
                    });
                }
                ts.extend(g.triangles);
                fs.extend(g.appearance.into_iter().map(|f| {
                    f.map(|mut f| {
                        f.texture += base;
                        f
                    })
                }));
            }
        }
        Ok((ts, fs))
    }
}
