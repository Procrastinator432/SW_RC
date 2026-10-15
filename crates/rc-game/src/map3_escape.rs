//! Bounded Sun Fac escape and original guard-room door/trigger bindings.
use crate::{Game, Point, STEP};
use rc_package::{
    hull::{Hull, HullSet},
    world_collision::BodyShape,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Door {
    pub original: u32,
    pub actor: String,
    pub offset: Point,
    pub snapshot: Vec<u8>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub route: Vec<Point>,
    pub start: Point,
    pub destination: Point,
    pub destination_rotation: [i32; 3],
    pub speed: f64,
    pub move_time: f64,
    pub doors: Vec<Door>,
    pub door_position: Point,
    pub trigger: Point,
    pub trigger_radius: f64,
    pub trigger_height: f64,
    pub provenance: serde_json::Value,
}
pub struct PreparedDoor {
    pub scene: rc_render::Scene,
    pub texture: usize,
    pub collider: (usize, Arc<HullSet>),
}
pub struct Prepared {
    pub assets: Assets,
    pub doors: Vec<PreparedDoor>,
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct State {
    pub stage: u8,
    pub waypoint: usize,
    pub yaw: f64,
    pub moving: bool,
    pub door_fraction: f64,
    pub locked: bool,
    pub wait: f64,
    pub blocked: u64,
    pub arrived_tick: Option<u64>,
    pub teleport_tick: Option<u64>,
    pub trigger_tick: Option<u64>,
    pub position: Point,
}
pub fn translated(h: &HullSet, offset: Point) -> HullSet {
    HullSet {
        hulls: h
            .hulls
            .iter()
            .map(|h| Hull {
                offset: h.offset,
                planes: h
                    .planes
                    .iter()
                    .map(|p| {
                        [
                            p[0],
                            p[1],
                            p[2],
                            p[3] + (0..3).map(|i| p[i] * offset[i]).sum::<f64>(),
                        ]
                    })
                    .collect(),
                bounds: h.bounds.map(|v| std::array::from_fn(|i| v[i] + offset[i])),
            })
            .collect(),
        missing_solid_leaves: h.missing_solid_leaves,
        empty_root_solid: h.empty_root_solid,
    }
}
impl Game {
    pub(crate) fn map3_escape_target(&self) -> Option<Point> {
        let p = self
            .map
            .as_ref()?
            .opening
            .as_ref()?
            .planning
            .as_ref()?
            .escape
            .as_ref()?;
        let o = self.state.map.as_ref()?.opening.as_ref()?;
        let e = o.planning.as_ref()?.escape.as_ref()?;
        Some(if !o.objectives[2] {
            [-19859.6796875, 11475.5244140625, -399.999755859375]
        } else if e.stage < 3 {
            p.assets.door_position
        } else if e.trigger_tick.is_none() {
            p.assets.trigger
        } else {
            p.assets.destination
        })
    }
    pub(crate) fn prepare_map3_escape(&mut self) -> Result<(), String> {
        let Some(a) = self
            .map
            .as_mut()
            .and_then(|m| m.opening.as_mut())
            .and_then(|p| p.planning.as_mut())
            .and_then(|p| p.assets.escape.take())
        else {
            return Ok(());
        };
        let points = a
            .route
            .iter()
            .chain([&a.start, &a.destination, &a.door_position, &a.trigger]);
        if a.route.is_empty()
            || a.route.len() > 2048
            || points.flatten().any(|v| !v.is_finite() || v.abs() > 1e7)
            || a.doors.iter().map(|d| d.original).ne([134, 135])
            || !a.speed.is_finite()
            || !(1. ..=1500.).contains(&a.speed)
            || !a.move_time.is_finite()
            || !(0.01..=30.).contains(&a.move_time)
            || !a.trigger_radius.is_finite()
            || !(1. ..=2000.).contains(&a.trigger_radius)
            || !a.trigger_height.is_finite()
            || !(1. ..=1000.).contains(&a.trigger_height)
        {
            return Err("Invalid Sun Fac escape bindings".into());
        }
        self.map.as_ref().unwrap().opening.as_ref().unwrap().actors[5]
            .0
            .clip("RunForward")?;
        let mut doors = vec![];
        for d in &a.doors {
            if d.offset.iter().any(|v| !v.is_finite() || v.abs() > 1000.)
                || d.snapshot.len() > 2 * 1024 * 1024
            {
                return Err("Invalid escape door binding".into());
            }
            let collider = self
                .world
                .actors
                .iter()
                .enumerate()
                .find_map(|(i, x)| {
                    if x.name == d.actor {
                        if let BodyShape::Hulls(h) = &x.shape {
                            return Some((i, h.clone()));
                        }
                    }
                    None
                })
                .ok_or("Missing escape door collider")?;
            let scene = rc_render::Scene::from_snapshot(&d.snapshot)?;
            let texture = self.scene.textures.len();
            self.scene.textures.extend(scene.textures.iter().cloned());
            doors.push(PreparedDoor {
                scene,
                texture,
                collider,
            });
        }
        let state = State {
            stage: 0,
            waypoint: 0,
            yaw: 17336. * std::f64::consts::TAU / 65536.,
            moving: false,
            door_fraction: 0.,
            locked: false,
            wait: 0.,
            blocked: 0,
            arrived_tick: None,
            teleport_tick: None,
            trigger_tick: None,
            position: a.start,
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
            .escape = Some(Box::new(state));
        self.map
            .as_mut()
            .unwrap()
            .opening
            .as_mut()
            .unwrap()
            .planning
            .as_mut()
            .unwrap()
            .escape = Some(Box::new(Prepared { assets: *a, doors }));
        Ok(())
    }
    pub(crate) fn sync_map3_escape(&mut self) {
        let Some(p) = self
            .map
            .as_ref()
            .and_then(|m| m.opening.as_ref())
            .and_then(|p| p.planning.as_ref())
            .and_then(|p| p.escape.as_ref())
        else {
            return;
        };
        let s = self
            .state
            .map
            .as_ref()
            .unwrap()
            .opening
            .as_ref()
            .unwrap()
            .planning
            .as_ref()
            .unwrap()
            .escape
            .as_ref()
            .unwrap();
        for (d, a) in p.doors.iter().zip(&p.assets.doors) {
            self.world.actors[d.collider.0].shape = BodyShape::Hulls(Arc::new(translated(
                &d.collider.1,
                a.offset.map(|v| v * s.door_fraction),
            )));
        }
    }
    pub(crate) fn map3_escape_tick(&mut self) -> Result<(), String> {
        let Some(p) = self
            .map
            .as_ref()
            .and_then(|m| m.opening.as_ref())
            .and_then(|p| p.planning.as_ref())
            .and_then(|p| p.escape.as_ref())
        else {
            return Ok(());
        };
        let m = self.state.map.as_mut().unwrap();
        let o = m.opening.as_mut().unwrap();
        if o.locked {
            return Ok(());
        }
        let s = o.planning.as_mut().unwrap().escape.as_mut().unwrap();
        let tick = self.state.statistics.ticks;
        if s.stage == 0 && o.events.contains("WindowDestroyed") {
            s.stage = 1;
            o.animations[6].position = s.position;
        }
        let unlocked = o.events.contains("GuardRoomDoorUnlocked");
        if unlocked {
            s.locked = false;
        }
        let clones_near = std::iter::once(self.state.pawn.body.position)
            .chain(m.companion.iter().chain(m.fixer.iter()).map(|c| c.position))
            .chain(std::iter::once(o.sev.position))
            .any(|at| crate::distance(at, p.assets.door_position) < 600.);
        let open = (s.stage == 1 && crate::distance(s.position, p.assets.door_position) < 600.)
            || (unlocked && clones_near);
        let proposed = (s.door_fraction
            + if open {
                STEP / p.assets.move_time
            } else {
                -STEP / p.assets.move_time
            })
        .clamp(0., 1.);
        let bodies = std::iter::once(self.state.pawn.body.position)
            .chain(m.companion.iter().chain(m.fixer.iter()).map(|c| c.position))
            .chain(std::iter::once(o.sev.position));
        let occupied = if proposed < s.door_fraction {
            let mut hit = false;
            for at in bodies {
                for (d, a) in p.doors.iter().zip(&p.assets.doors) {
                    if translated(&d.collider.1, a.offset.map(|v| v * proposed))
                        .sweep(at, at, crate::squad::EXTENT)?
                        .is_some()
                    {
                        hit = true;
                    }
                }
            }
            hit
        } else {
            false
        };
        if !occupied {
            s.door_fraction = proposed;
        }
        // Apply mover collision before the next physical Sun Fac step.
        for (d, a) in p.doors.iter().zip(&p.assets.doors) {
            self.world.actors[d.collider.0].shape = BodyShape::Hulls(Arc::new(translated(
                &d.collider.1,
                a.offset.map(|v| v * s.door_fraction),
            )));
        }
        s.moving = false;
        if s.stage == 1 {
            while s.waypoint < p.assets.route.len()
                && crate::distance(s.position, p.assets.route[s.waypoint]) < 4.
            {
                s.waypoint += 1;
            }
            if let Some(to) = p.assets.route.get(s.waypoint) {
                let d = (to[0] - s.position[0]).hypot(to[1] - s.position[1]);
                if d > 0.001 {
                    s.yaw = (to[1] - s.position[1]).atan2(to[0] - s.position[0]);
                    let speed = p.assets.speed.min(d / STEP);
                    let body = rc_package::physics::BodyState {
                        position: s.position,
                        velocity: [s.yaw.cos() * speed, s.yaw.sin() * speed, 0.],
                        grounded: true,
                    };
                    let q = self.world.body_tick(
                        &body,
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
                    if q.body.grounded {
                        s.moving = crate::distance(s.position, q.body.position) > 0.1;
                        s.position = q.body.position;
                    }
                    if !s.moving {
                        s.blocked += 1;
                    }
                }
            } else {
                s.stage = 2;
                s.locked = true;
                s.wait = 3.;
                s.arrived_tick = Some(tick);
                o.events.insert("SunFacThroughDoorReached".into());
                o.event_ticks
                    .insert("SunFacThroughDoorReached".into(), tick);
            }
            o.animations[6].position = s.position;
        } else if s.stage == 2 {
            s.wait = (s.wait - STEP).max(0.);
            if s.wait < 1e-8 {
                s.stage = 3;
                s.teleport_tick = Some(tick);
                s.position = p.assets.destination;
                s.yaw = p.assets.destination_rotation[1] as f64 * std::f64::consts::TAU / 65536.;
                o.animations[6].position = s.position;
                o.events.insert("ActivateSBDIntro".into());
                o.event_ticks.insert("ActivateSBDIntro".into(), tick);
                o.events.insert("SunFacInsideShieldReached".into());
                o.event_ticks
                    .insert("SunFacInsideShieldReached".into(), tick);
            }
        }
        if s.stage >= 3 && s.trigger_tick.is_none() {
            let bodies = std::iter::once(self.state.pawn.body.position)
                .chain(
                    m.companion
                        .iter()
                        .chain(m.fixer.iter())
                        .filter(|c| c.joined && !c.down)
                        .map(|c| c.position),
                )
                .chain(o.sev.joined.then_some(o.sev.position));
            if bodies.into_iter().any(|at| {
                touches(
                    at,
                    p.assets.trigger,
                    p.assets.trigger_radius,
                    p.assets.trigger_height,
                )
            }) {
                s.trigger_tick = Some(tick);
                o.events.insert("StartSBDIntro".into());
                o.event_ticks.insert("StartSBDIntro".into(), tick);
            }
        }
        Ok(())
    }
    pub(crate) fn map3_escape_geometry(
        &self,
    ) -> (
        Vec<rc_package::geometry::Triangle>,
        Vec<Option<rc_render::FaceTexture>>,
    ) {
        let Some(p) = self
            .map
            .as_ref()
            .and_then(|m| m.opening.as_ref())
            .and_then(|p| p.planning.as_ref())
            .and_then(|p| p.escape.as_ref())
        else {
            return (vec![], vec![]);
        };
        let s = self
            .state
            .map
            .as_ref()
            .unwrap()
            .opening
            .as_ref()
            .unwrap()
            .planning
            .as_ref()
            .unwrap()
            .escape
            .as_ref()
            .unwrap();
        let mut ts = vec![];
        let mut fs = vec![];
        for (d, a) in p.doors.iter().zip(&p.assets.doors) {
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
        (ts, fs)
    }
}
fn touches(at: Point, trigger: Point, radius: f64, height: f64) -> bool {
    (at[0] - trigger[0]).hypot(at[1] - trigger[1]) <= radius + 40.
        && (at[2] - trigger[2]).abs() <= height + 84.
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn translated_door_keeps_its_blocking_shape_at_the_open_position() {
        let h = HullSet {
            hulls: vec![Hull {
                offset: 7,
                planes: vec![
                    [1., 0., 0., 10.],
                    [-1., 0., 0., 10.],
                    [0., 1., 0., 5.],
                    [0., -1., 0., 5.],
                    [0., 0., 1., 100.],
                    [0., 0., -1., 100.],
                ],
                bounds: [[-10., -5., -100.], [10., 5., 100.]],
            }],
            missing_solid_leaves: 0,
            empty_root_solid: false,
        };
        let moved = translated(&h, [192., 0., 0.]);
        assert!(moved
            .sweep([0., -20., 0.], [0., 20., 0.], [0.; 3])
            .unwrap()
            .is_none());
        assert!(moved
            .sweep([192., -20., 0.], [192., 20., 0.], [0.; 3])
            .unwrap()
            .is_some());
        assert!(h
            .sweep([0., -20., 0.], [0., 20., 0.], [0.; 3])
            .unwrap()
            .is_some());
    }
    #[test]
    fn proximity_trigger_requires_vertical_and_horizontal_body_overlap() {
        assert!(touches([1040., 0., 106.], [0.; 3], 1000., 22.));
        assert!(!touches([1040.01, 0., 0.], [0.; 3], 1000., 22.));
        assert!(!touches([0., 0., 106.01], [0.; 3], 1000., 22.));
    }
}
