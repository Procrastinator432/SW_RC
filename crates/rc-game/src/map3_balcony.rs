//! Selected AA-room encounter and Sun Fac shield/sniper-position staging.
use crate::{
    map3_opening::{self, Animation},
    map3_planning::Factory,
    Enemy, Game, Input, Point, STEP,
};
use rc_package::{hull::HullSet, world_collision::BodyShape};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finale: Option<Box<crate::map3_finale::Assets>>,
    pub factories: Vec<Factory>,
    pub sun_route: Vec<Point>,
    pub outside_rotation: [i32; 3],
    pub shield_snapshot: Vec<u8>,
    pub shield_bounds: [Point; 2],
    pub standby: Point,
    pub snipe: Point,
    pub crate_position: Point,
    pub crate_actor: String,
    pub provenance: serde_json::Value,
}
pub struct Prepared {
    pub finale: Option<Box<crate::map3_finale::Prepared>>,
    pub assets: Assets,
    pub shield: rc_render::Scene,
    pub texture: usize,
    pub collider: usize,
    pub hull: Arc<HullSet>,
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct State {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finale: Option<Box<crate::map3_finale::State>>,
    pub bindings: Vec<(usize, usize)>,
    pub spawned_tick: Option<u64>,
    pub stage: u8,
    pub wait: f64,
    pub waypoint: usize,
    pub animation: Animation,
    pub anchor: Point,
    pub rotation: [i32; 3],
    pub shield_tick: Option<u64>,
    pub shield_visible: bool,
    pub sev_agent: crate::navigation::Agent,
    pub assigned_tick: Option<u64>,
    pub positioned_tick: Option<u64>,
    pub blocked: u64,
}
impl Game {
    pub(crate) fn balcony_prepared(&self) -> Option<&Prepared> {
        self.breach_prepared()?.balcony.as_deref()
    }
    pub(crate) fn balcony_state(&self) -> Option<&State> {
        self.breach_state()?.balcony.as_deref()
    }
    pub(crate) fn prepare_map3_balcony(&mut self) -> Result<(), String> {
        let Some(b) = self
            .map
            .as_mut()
            .and_then(|m| m.opening.as_mut())
            .and_then(|p| p.planning.as_mut())
            .and_then(|p| p.guard.as_mut())
            .and_then(|p| p.breach.as_mut())
        else {
            return Ok(());
        };
        let Some(a) = b.assets.balcony.take() else {
            return Ok(());
        };
        if a.factories
            .iter()
            .map(|f| f.original)
            .ne([151, 152, 153, 474, 475, 476, 481, 486])
            || a.sun_route.is_empty()
            || a.sun_route.len() > 2048
            || a.shield_snapshot.len() > 2 * 1024 * 1024
            || a.sun_route
                .iter()
                .flatten()
                .chain(a.shield_bounds.iter().flatten())
                .chain(a.standby.iter())
                .chain(a.snipe.iter())
                .chain(a.crate_position.iter())
                .any(|v| !v.is_finite() || v.abs() > 1e7)
            || (0..3).any(|i| a.shield_bounds[0][i] >= a.shield_bounds[1][i])
            || a.factories.iter().any(|f| {
                f.health == 0
                    || f.health > 10000
                    || f.warrior
                    || f.position.iter().any(|v| !v.is_finite())
                    || f.extent
                        .iter()
                        .any(|v| !v.is_finite() || *v <= 0. || *v > 250.)
            })
        {
            return Err("Invalid balcony bindings".into());
        }
        let shield = rc_render::Scene::from_snapshot(&a.shield_snapshot)?;
        let texture = self.scene.textures.len();
        self.scene.textures.extend(shield.textures.iter().cloned());
        let (collider, hull) = self
            .world
            .actors
            .iter()
            .enumerate()
            .find_map(|(i, a)| {
                if a.name == "Prop3" {
                    if let BodyShape::Hulls(h) = &a.shape {
                        Some((i, h.clone()))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .ok_or("Missing original shield collision")?;
        self.world.actors[collider].shape = BodyShape::Clear;
        let mesh = &self.map.as_ref().unwrap().opening.as_ref().unwrap().actors[5].0;
        mesh.clip("EmoteSeeEnemy")?;
        mesh.clip("DeathTaunt")?;
        let state = State {
            finale: None,
            bindings: vec![],
            spawned_tick: None,
            stage: 0,
            wait: 0.,
            waypoint: 0,
            animation: Animation {
                random: None,
                clip: "EmoteSeeEnemy".into(),
                playback: map3_opening::empty_playback(),
                finished: true,
                visible: true,
                blocked: 0,
                position: a.sun_route[0],
                notifies: vec![],
                root: false,
            },
            anchor: a.sun_route[0],
            rotation: [0; 3],
            shield_tick: None,
            shield_visible: false,
            sev_agent: crate::navigation::Agent::default(),
            assigned_tick: None,
            positioned_tick: None,
            blocked: 0,
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
            .breach
            .as_mut()
            .unwrap()
            .balcony = Some(Box::new(state));
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
            .breach
            .as_mut()
            .unwrap()
            .balcony = Some(Box::new(Prepared {
            finale: None,
            assets: *a,
            shield,
            texture,
            collider,
            hull,
        }));
        Ok(())
    }
    pub(crate) fn sync_map3_balcony(&mut self) {
        let Some(p) = self.balcony_prepared() else {
            return;
        };
        let collider = p.collider;
        let shape = if self.balcony_state().unwrap().shield_tick.is_some() {
            BodyShape::Hulls(p.hull.clone())
        } else {
            BodyShape::Clear
        };
        self.world.actors[collider].shape = shape;
    }
    pub(crate) fn map3_balcony_spec(&self, i: usize) -> Option<crate::map3_guard::Spec> {
        let p = self.balcony_prepared()?;
        let slot = self
            .balcony_state()?
            .bindings
            .iter()
            .find(|(global, _)| *global == i)?
            .1;
        let f = &p.assets.factories[slot];
        Some(crate::map3_guard::Spec {
            extent: f.extent,
            warrior: false,
            sbd: false,
            active: true,
            fire_sound: Some(
                self.map
                    .as_ref()?
                    .opening
                    .as_ref()?
                    .planning
                    .as_ref()?
                    .guard
                    .as_ref()?
                    .assets
                    .fire_sound,
            ),
        })
    }
    pub(crate) fn map3_balcony_sev_controlled(&self) -> bool {
        self.balcony_state()
            .is_some_and(|s| s.shield_tick.is_some())
            && !self
                .state
                .map
                .as_ref()
                .unwrap()
                .opening
                .as_ref()
                .unwrap()
                .events
                .contains("DisableBalconySnipe")
    }
    pub(crate) fn map3_balcony_tick(&mut self, input: Input) -> Result<(), String> {
        let Some(p) = self.balcony_prepared() else {
            return Ok(());
        };
        if self.map3_locked() {
            return Ok(());
        }
        let eye = self.eye();
        let direction = crate::direction(self.state.yaw, self.state.pitch);
        let end = std::array::from_fn(|i| eye[i] + direction[i] * 2500.);
        let assigned = self.balcony_state().unwrap().assigned_tick.is_some();
        let marker_command = if input.use_action && !assigned {
            let hit = crate::ray_box(eye, end, p.assets.crate_position, [90., 90., 100.]);
            let trace = self.world.sweep_motion(eye, end, [0.; 3])?;
            if !trace.complete {
                return Err("Incomplete sniper-marker visibility".into());
            }
            hit.is_some_and(|f| {
                trace
                    .contacts
                    .first()
                    .is_none_or(|c| c.object == p.assets.crate_actor || c.hit.fraction + 0.01 >= f)
            })
        } else {
            false
        };
        let p = self.map.as_ref().unwrap().opening.as_ref().unwrap();
        let mesh = &p.actors[5].0;
        let b = p
            .planning
            .as_ref()
            .unwrap()
            .guard
            .as_ref()
            .unwrap()
            .breach
            .as_ref()
            .unwrap();
        let a = &b.balcony.as_ref().unwrap().assets;
        let m = self.state.map.as_mut().unwrap();
        let o = m.opening.as_mut().unwrap();
        let planning = o.planning.as_mut().unwrap();
        let escape = planning.escape.as_mut().unwrap();
        let state = planning
            .guard
            .as_mut()
            .unwrap()
            .breach
            .as_mut()
            .unwrap()
            .balcony
            .as_mut()
            .unwrap();
        let tick = self.state.statistics.ticks + 1;
        if state.spawned_tick.is_none() && o.events.contains("SpawnAARoom") {
            for (slot, f) in a.factories.iter().enumerate() {
                let i = self.state.enemies.len();
                self.state.enemies.push(Enemy {
                    position: f.position,
                    health: f.health,
                    cooldown: 1.4,
                    flash: 0.,
                });
                state.bindings.push((i, slot));
            }
            state.spawned_tick = Some(tick);
        }
        match state.stage {
            0 if o.events.contains("SunFacRoomDoorOpened") => {
                state.stage = 1;
                state.wait = 2.;
            }
            1 => {
                state.wait = (state.wait - STEP).max(0.);
                if state.wait < 1e-8 {
                    state.stage = 2;
                    state.anchor = escape.position;
                    state.rotation = [0, (escape.yaw * 65536. / std::f64::consts::TAU) as i32, 0];
                    map3_opening::begin(mesh, &mut state.animation, "EmoteSeeEnemy")?;
                }
            }
            3 => {
                state.wait = (state.wait - STEP).max(0.);
                if state.wait < 1e-8 {
                    state.stage = 4;
                }
            }
            4 => {
                escape.moving = false;
                if let Some(target) = a.sun_route.get(state.waypoint) {
                    let from = escape.position;
                    if crate::distance(from, *target) < 5. {
                        state.waypoint += 1;
                    } else {
                        let d = crate::distance(from, *target);
                        let wanted: Point = std::array::from_fn(|i| {
                            from[i] + (target[i] - from[i]) * (450. * STEP / d).min(1.)
                        });
                        if let Some(next) = sun_step(&self.world, from, wanted)? {
                            escape.position = next;
                            escape.yaw = (target[1] - from[1]).atan2(target[0] - from[0]);
                            escape.moving = crate::distance(from, next) > 0.01;
                        } else {
                            state.blocked += 1;
                        }
                    }
                } else {
                    state.stage = 5;
                    escape.yaw = a.outside_rotation[1] as f64 * std::f64::consts::TAU / 65536.;
                    state.shield_tick = Some(tick);
                    o.events.insert("ActivateShield".into());
                    o.event_ticks.insert("ActivateShield".into(), tick);
                    state.anchor = escape.position;
                    state.rotation = a.outside_rotation;
                    map3_opening::begin(mesh, &mut state.animation, "DeathTaunt")?;
                }
            }
            _ => {}
        }
        if state.stage == 2 || state.stage == 5 {
            escape.moving = false;
            map3_opening::tick_animation(mesh, &mut state.animation, tick)?;
            let to = map3_opening::authored(mesh, &state.animation, state.anchor, state.rotation)?;
            let from = escape.position;
            let frame = self.world.body_tick(
                &rc_package::physics::BodyState {
                    position: from,
                    velocity: [(to[0] - from[0]) / STEP, (to[1] - from[1]) / STEP, 0.],
                    grounded: true,
                },
                STEP,
                rc_package::physics::PhysicsOptions {
                    extent: [50., 50., 115.],
                    gravity: 950.,
                    terminal_speed: 4000.,
                    skin: 0.5,
                    support_distance: 35.,
                    minimum_up: 0.7,
                    max_iterations: 8,
                    step_height: 35.,
                },
            )?;
            escape.position = frame.body.position;
            state.animation.position = escape.position;
            if state.animation.finished {
                if state.stage == 2 {
                    state.stage = 3;
                    state.wait = 5.;
                } else {
                    state.stage = 6;
                }
            }
        }
        if let Some(at) = state.shield_tick {
            state.shield_visible = tick >= at + 120;
        }
        o.animations[6].position = escape.position;
        if marker_command && o.events.contains("EnableBalconySnipe") && o.sev.joined && !o.sev.down
        {
            state.assigned_tick = Some(tick);
            state.sev_agent.route.clear();
            state.sev_agent.next_plan = 0.;
        }
        if state.shield_tick.is_some()
            && o.events.contains("WalkToSnipeMarker")
            && !o.events.contains("DisableBalconySnipe")
        {
            let target = if state.assigned_tick.is_some() {
                a.snipe
            } else {
                a.standby
            };
            crate::map_gameplay::follow_to(
                &self.world,
                &b.assets.navigation,
                &mut o.sev,
                &mut state.sev_agent,
                target,
                tick as f64 * STEP,
                450.,
                6.,
            )?;
            if state.assigned_tick.is_some()
                && state.positioned_tick.is_none()
                && crate::distance(o.sev.position, a.snipe) < 8.
            {
                state.positioned_tick = Some(tick);
                o.sev.yaw = 0.;
                o.events.insert("InSnipePosition".into());
                o.event_ticks.insert("InSnipePosition".into(), tick);
                o.events.insert("SnipeStarted".into());
                o.event_ticks.insert("SnipeStarted".into(), tick);
            }
        }
        self.sync_map3_balcony();
        Ok(())
    }
    pub(crate) fn map3_balcony_sun_pose(&self) -> Result<Option<(usize, f32)>, String> {
        let Some(s) = self.balcony_state() else {
            return Ok(None);
        };
        if s.stage != 2 && s.stage != 5 {
            return Ok(None);
        }
        let mesh = &self.map.as_ref().unwrap().opening.as_ref().unwrap().actors[5].0;
        Ok(Some((
            mesh.clip(&s.animation.clip)?,
            map3_opening::anim_frame(&s.animation, mesh)?,
        )))
    }
    pub(crate) fn map3_balcony_status(&self) -> Option<String> {
        let s = self.balcony_state()?;
        if s.positioned_tick.is_some() {
            return Some("GEO_01C · Sev in Scharfschuetzenposition".into());
        }
        if s.assigned_tick.is_some() {
            return Some("GEO_01C · Sev bezieht Scharfschuetzenposition".into());
        }
        if s.shield_tick.is_some() {
            return Some("GEO_01C · Schild aktiv · Kiste anvisieren und BENUTZEN".into());
        }
        s.spawned_tick
            .map(|_| "GEO_01C · AA-Raum · Sun Fac verfolgen".into())
    }
    pub(crate) fn map3_balcony_target(&self) -> Option<Point> {
        let s = self.balcony_state()?;
        let p = self.balcony_prepared()?;
        if s.shield_tick.is_some() {
            Some(p.assets.crate_position)
        } else if s.spawned_tick.is_some() {
            Some(p.assets.sun_route[0])
        } else {
            None
        }
    }
    pub(crate) fn map3_balcony_geometry(
        &self,
    ) -> Result<
        (
            Vec<rc_package::geometry::Triangle>,
            Vec<Option<rc_render::FaceTexture>>,
        ),
        String,
    > {
        let Some(p) = self.balcony_prepared() else {
            return Ok((vec![], vec![]));
        };
        if !self.balcony_state().unwrap().shield_visible {
            return Ok((vec![], vec![]));
        }
        Ok((
            p.shield.triangles.clone(),
            p.shield
                .appearance
                .iter()
                .map(|f| {
                    f.map(|mut f| {
                        f.texture += p.texture;
                        f
                    })
                })
                .collect(),
        ))
    }
}

pub(crate) fn sun_step(
    world: &rc_package::world_collision::StaticBodyWorld,
    from: Point,
    to: Point,
) -> Result<Option<Point>, String> {
    let frame = world.body_tick(
        &rc_package::physics::BodyState {
            position: from,
            velocity: [(to[0] - from[0]) / STEP, (to[1] - from[1]) / STEP, 0.],
            grounded: true,
        },
        STEP,
        rc_package::physics::PhysicsOptions {
            extent: [50., 50., 115.],
            gravity: 950.,
            terminal_speed: 4000.,
            skin: 0.5,
            support_distance: 35.,
            minimum_up: 0.7,
            max_iterations: 8,
            step_height: 35.,
        },
    )?;
    Ok(frame.body.grounded.then_some(frame.body.position))
}
