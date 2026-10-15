//! Original unlocked elevator door and condition-gated exit 39.
use crate::{
    map3_escape::{translated, Door, PreparedDoor},
    Game, Phase, Point, STEP,
};
use rc_package::world_collision::BodyShape;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub doors: Vec<Door>,
    pub move_time: f64,
    pub door_trigger: Point,
    pub door_radius: f64,
    pub door_height: f64,
    pub approach: Point,
    pub approach_radius: f64,
    pub approach_height: f64,
    pub exit: Point,
    pub exit_radius: f64,
    pub exit_height: f64,
    pub provenance: serde_json::Value,
}
pub struct Prepared {
    pub assets: Assets,
    pub doors: Vec<PreparedDoor>,
}
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct State {
    pub door_fraction: f64,
    pub approach_tick: Option<u64>,
    pub completed_tick: Option<u64>,
}
fn touches(at: Point, center: Point, radius: f64, height: f64, extent: Point) -> bool {
    (at[0] - center[0]).hypot(at[1] - center[1]) <= radius + extent[0]
        && (at[2] - center[2]).abs() <= height + extent[2]
}
impl Game {
    pub(crate) fn elevator_prepared(&self) -> Option<&Prepared> {
        self.finale_prepared()?.elevator.as_deref()
    }
    pub(crate) fn elevator_state(&self) -> Option<&State> {
        self.finale_state()?.elevator.as_deref()
    }
    fn elevator_state_mut(&mut self) -> &mut State {
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
            .balcony
            .as_mut()
            .unwrap()
            .finale
            .as_mut()
            .unwrap()
            .elevator
            .as_mut()
            .unwrap()
    }
    pub(crate) fn prepare_map3_elevator(&mut self) -> Result<(), String> {
        let Some(f) = self
            .map
            .as_mut()
            .and_then(|m| m.opening.as_mut())
            .and_then(|m| m.planning.as_mut())
            .and_then(|m| m.guard.as_mut())
            .and_then(|m| m.breach.as_mut())
            .and_then(|m| m.balcony.as_mut())
            .and_then(|m| m.finale.as_mut())
        else {
            return Ok(());
        };
        let Some(a) = f.assets.elevator.take() else {
            return Ok(());
        };
        if a.doors.iter().map(|d| d.original).ne([465, 466])
            || !a.move_time.is_finite()
            || !(0.01..=30.).contains(&a.move_time)
            || [&a.door_trigger, &a.approach, &a.exit]
                .into_iter()
                .flatten()
                .any(|v| !v.is_finite() || v.abs() > 1e7)
            || [
                a.door_radius,
                a.door_height,
                a.approach_radius,
                a.approach_height,
                a.exit_radius,
                a.exit_height,
            ]
            .into_iter()
            .any(|v| !v.is_finite() || !(1. ..=2000.).contains(&v))
        {
            return Err("Invalid original elevator bindings".into());
        }
        let mut doors = vec![];
        for d in &a.doors {
            if d.offset.iter().any(|v| !v.is_finite() || v.abs() > 1000.)
                || d.snapshot.len() > 2 * 1024 * 1024
            {
                return Err("Invalid elevator door".into());
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
                .ok_or("Missing elevator door collider")?;
            let scene = rc_render::Scene::from_snapshot(&d.snapshot)?;
            let texture = self.scene.textures.len();
            self.scene.textures.extend(scene.textures.iter().cloned());
            doors.push(PreparedDoor {
                scene,
                texture,
                collider,
            });
        }
        f.elevator = Some(Box::new(Prepared { assets: *a, doors }));
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
            .balcony
            .as_mut()
            .unwrap()
            .finale
            .as_mut()
            .unwrap()
            .elevator = Some(Box::new(State {
            door_fraction: 0.,
            approach_tick: None,
            completed_tick: None,
        }));
        Ok(())
    }
    pub(crate) fn sync_map3_elevator(&mut self) {
        let Some(p) = self.elevator_prepared() else {
            return;
        };
        let fraction = self.elevator_state().unwrap().door_fraction;
        let shapes: Vec<_> = p
            .doors
            .iter()
            .zip(&p.assets.doors)
            .map(|(d, a)| {
                (
                    d.collider.0,
                    BodyShape::Hulls(Arc::new(translated(
                        &d.collider.1,
                        a.offset.map(|v| v * fraction),
                    ))),
                )
            })
            .collect();
        for (i, shape) in shapes {
            self.world.actors[i].shape = shape;
        }
    }
    pub(crate) fn map3_elevator_tick(&mut self) -> Result<(), String> {
        let Some(p) = self.elevator_prepared() else {
            return Ok(());
        };
        let m = self.state.map.as_ref().unwrap();
        let o = m.opening.as_ref().unwrap();
        if o.locked || self.state.phase != Phase::Playing {
            return Ok(());
        }
        let s = self.elevator_state().unwrap();
        let extent = self.profile.physics.extent;
        let bodies: Vec<_> = std::iter::once(self.state.pawn.body.position)
            .chain(
                m.companion
                    .iter()
                    .chain(m.fixer.iter())
                    .filter(|c| c.joined)
                    .map(|c| c.position),
            )
            .chain(o.sev.joined.then_some(o.sev.position))
            .collect();
        let open = bodies.iter().any(|at| {
            touches(
                *at,
                p.assets.door_trigger,
                p.assets.door_radius,
                p.assets.door_height,
                extent,
            )
        });
        let proposed = (s.door_fraction
            + if open {
                STEP / p.assets.move_time
            } else {
                -STEP / p.assets.move_time
            })
        .clamp(0., 1.);
        let mut occupied = false;
        if proposed < s.door_fraction {
            for at in &bodies {
                for (d, a) in p.doors.iter().zip(&p.assets.doors) {
                    occupied |= translated(&d.collider.1, a.offset.map(|v| v * proposed))
                        .sweep(*at, *at, extent)?
                        .is_some();
                }
            }
        }
        let fraction = if occupied { s.door_fraction } else { proposed };
        let enabled = o.events.contains("ConditionSunFacDead")
            && self.finale_state().unwrap().success == Some(true);
        let approach = enabled
            && s.approach_tick.is_none()
            && o.events.contains("ElevatorTrigger")
            && bodies.iter().any(|at| {
                touches(
                    *at,
                    p.assets.approach,
                    p.assets.approach_radius,
                    p.assets.approach_height,
                    extent,
                )
            });
        // A cylinder overlap through the still-closed door must never complete the mission.
        let finish = enabled
            && o.objectives[..6].iter().all(|v| *v)
            && fraction >= 1.
            && touches(
                self.state.pawn.body.position,
                p.assets.exit,
                p.assets.exit_radius,
                p.assets.exit_height,
                extent,
            )
            && self
                .world
                .sweep_motion(self.state.pawn.body.position, p.assets.exit, [0.; 3])?
                .complete;
        let tick = self.state.statistics.ticks;
        let s = self.elevator_state_mut();
        s.door_fraction = fraction;
        if approach {
            s.approach_tick = Some(tick);
        }
        if finish {
            s.completed_tick = Some(tick);
        }
        let o = self.state.map.as_mut().unwrap().opening.as_mut().unwrap();
        if approach {
            o.events.insert("ContinueIntoElevator".into());
            o.event_ticks.insert("ContinueIntoElevator".into(), tick);
        }
        if finish {
            o.objectives[6] = true;
            o.active = 6;
            o.events.insert("FactoryElevatorEntered".into());
            o.event_ticks.insert("FactoryElevatorEntered".into(), tick);
            self.state.phase = Phase::Won;
        }
        self.sync_map3_elevator();
        Ok(())
    }
    pub(crate) fn map3_elevator_geometry(
        &self,
    ) -> (
        Vec<rc_package::geometry::Triangle>,
        Vec<Option<rc_render::FaceTexture>>,
    ) {
        let Some(p) = self.elevator_prepared() else {
            return (vec![], vec![]);
        };
        let fraction = self.elevator_state().unwrap().door_fraction;
        let mut ts = vec![];
        let mut fs = vec![];
        for (d, a) in p.doors.iter().zip(&p.assets.doors) {
            ts.extend(d.scene.triangles.iter().cloned().map(|mut t| {
                t.points = t
                    .points
                    .map(|v| std::array::from_fn(|i| v[i] + (a.offset[i] * fraction) as f32));
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
