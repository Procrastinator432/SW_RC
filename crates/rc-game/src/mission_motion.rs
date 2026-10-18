//! Original-size root-motion and ordinary JumpDest execution for mission Warriors.
use crate::{visual::PreparedMesh, warrior_ai::EXTENT, Point, STEP};
use rc_package::{jump_goal, jump_trajectory, world_collision::StaticBodyWorld};
use serde::Serialize;
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct Motion {
    pub clip: String,
    pub origin: Point,
    pub yaw: f64,
    pub frame: f32,
    pub node: Option<u32>,
    pub destination: Point,
    pub flight_origin: Point,
    pub velocity: [f32; 3],
    pub gravity: f32,
    pub flight_time: f32,
    pub elapsed: f64,
    pub stage: u8,
    pub goal: jump_goal::State,
    pub defensive: bool,
}
pub fn ground(
    world: &StaticBodyWorld,
    at: Point,
    extent: Point,
    depth: f64,
) -> Result<Option<Point>, String> {
    match world.floor_contact(at, extent, depth, 0.7) {
        Ok(c) => Ok(c.map(|c| [at[0], at[1], at[2] - depth * c.hit.fraction + 0.02])),
        Err(e) if e == "floor probe starts penetrating geometry" => Ok(None),
        Err(e) => Err(e),
    }
}
pub fn root(mesh: &PreparedMesh, clip: &str, frame: f32, yaw: f64) -> Result<Point, String> {
    let p = mesh
        .cinematic_root_delta(mesh.clip(clip)?, frame)?
        .map(f64::from);
    let (s, c) = yaw.sin_cos();
    Ok([p[0] * c - p[1] * s, p[0] * s + p[1] * c, p[2]])
}
fn clear(world: &StaticBodyWorld, from: Point, to: Point, extent: Point) -> Result<bool, String> {
    let s = world.sweep_motion(from, to, extent)?;
    if !s.complete {
        return Err("Incomplete mission motion sweep".into());
    }
    Ok(s.contacts.is_empty())
}
fn empty(clip: &str, origin: Point, yaw: f64) -> Motion {
    Motion {
        clip: clip.into(),
        origin,
        yaw,
        frame: 0.,
        node: None,
        destination: origin,
        flight_origin: origin,
        velocity: [0.; 3],
        gravity: 0.,
        flight_time: 0.,
        elapsed: 0.,
        stage: 0,
        goal: jump_goal::State::default(),
        defensive: false,
    }
}
pub fn lunge(
    world: &StaticBodyWorld,
    mesh: &PreparedMesh,
    from: Point,
    yaw: f64,
    clip: &str,
) -> Result<Option<Motion>, String> {
    let seq = &mesh.mesh.clips[mesh.clip(clip)?].sequence;
    let mut previous = from;
    for i in 1..seq.frames {
        let offset = root(mesh, clip, i as f32 / seq.frames as f32, yaw)?;
        let next = std::array::from_fn(|j| from[j] + offset[j]);
        if !clear(world, previous, next, EXTENT)?
            || !crate::navigation::walkable_for(
                world,
                [previous[0], previous[1], from[2]],
                [next[0], next[1], from[2]],
                EXTENT,
            )?
        {
            return Ok(None);
        }
        previous = next;
    }
    let mut m = empty(clip, from, yaw);
    m.destination = previous;
    Ok(Some(m))
}
pub fn jump(
    world: &StaticBodyWorld,
    volumes: &rc_package::volumes::VolumeWorld,
    mesh: &PreparedMesh,
    assets: &crate::jump::Assets,
    from: Point,
    node: &crate::jump::Node,
    defensive: bool,
) -> Result<Option<Motion>, String> {
    let at = node.position.map(f64::from);
    let Some(destination) = ground(world, [at[0], at[1], at[2] + 160.], EXTENT, 672.)? else {
        return Ok(None);
    };
    if crate::distance(from, destination) < 100. {
        return Ok(None);
    }
    let yaw = (destination[1] - from[1]).atan2(destination[0] - from[0]);
    let seq = &mesh.mesh.clips[mesh.clip("TakeOff")?].sequence;
    let offset = root(mesh, "TakeOff", 1. - 1. / seq.frames as f32, yaw)?;
    let flight_origin = std::array::from_fn(|i| from[i] + offset[i]);
    let volume = volumes.select(from.map(|v| v as f32))?;
    if !volume.complete
        || volume.settings.gravity[2] >= 0.
        || volumes.select(destination.map(|v| v as f32))?.object != volume.object
    {
        return Ok(None);
    }
    let gravity = volume.settings.gravity[2];
    let t = jump_trajectory::portable(jump_trajectory::Input {
        origin: flight_origin.map(|v| v as f32),
        target: destination.map(|v| v as f32),
        horizontal_speed: assets.air_speed,
        gravity_z: gravity,
        allow_flat_arcs: false,
    });
    if !t.time.is_finite()
        || t.time <= 0.
        || t.time > 12.
        || t.velocity.iter().any(|v| !v.is_finite())
    {
        return Ok(None);
    }
    let mut m = empty("TakeOff", from, yaw);
    m.node = Some(node.original);
    m.destination = destination;
    m.flight_origin = flight_origin;
    m.velocity = t.velocity;
    m.gravity = gravity;
    m.flight_time = t.time;
    m.defensive = defensive;
    let mut previous = from;
    for i in 1..seq.frames {
        let delta = root(mesh, "TakeOff", i as f32 / seq.frames as f32, yaw)?;
        let next = std::array::from_fn(|j| from[j] + delta[j]);
        if !clear(world, previous, next, EXTENT)? {
            return Ok(None);
        }
        previous = next;
    }
    let steps = (f64::from(t.time) / (STEP * 4.)).ceil() as usize;
    let interval = f64::from(t.time) / steps as f64;
    let margin = f64::from(gravity.abs()) * interval * interval / 8.;
    let extent = [50., 50., 115. + margin];
    for i in 1..=steps {
        let next = m.point(i as f64 * interval);
        let hit = world.sweep_motion(previous, next, extent)?;
        if !hit.complete {
            return Err("Incomplete mission jump arc sweep".into());
        }
        if hit.contacts.iter().any(|c| {
            i != steps
                || c.hit.normal[2] < 0.7
                || c.hit.start_overlapping
                || t.velocity[2] + gravity * t.time > 0.
        }) {
            return Ok(None);
        }
        let v = volumes.select(next.map(|v| v as f32))?;
        if !v.complete || v.object != volume.object {
            return Ok(None);
        }
        previous = next;
    }
    Ok(Some(m))
}
impl Motion {
    pub fn point(&self, t: f64) -> Point {
        let v = self.velocity.map(f64::from);
        [
            self.flight_origin[0] + v[0] * t,
            self.flight_origin[1] + v[1] * t,
            self.flight_origin[2] + v[2] * t + 0.5 * f64::from(self.gravity) * t * t,
        ]
    }
    /// Returns (finished, blocked); gameplay damage is emitted separately at clip notifies.
    pub fn advance(
        &mut self,
        world: &StaticBodyWorld,
        mesh: &PreparedMesh,
        position: &mut Point,
    ) -> Result<(bool, bool), String> {
        if self.stage == 1 {
            let seq = &mesh.mesh.clips[mesh.clip("FallBreathe")?].sequence;
            self.frame = (self.frame
                + f32::from_bits(seq.rate_bits) * STEP as f32 / seq.frames as f32)
                .rem_euclid(1.);
            self.elapsed += STEP;
            let next = self.point(self.elapsed.min(f64::from(self.flight_time)));
            let sweep = world.sweep_motion(*position, next, EXTENT)?;
            if !sweep.complete {
                return Err("Incomplete mission flight sweep".into());
            }
            if let Some(hit) = sweep.contacts.first() {
                let center = std::array::from_fn(|i| {
                    position[i] + (next[i] - position[i]) * hit.hit.fraction
                });
                if hit.hit.normal[2] >= 0.7
                    && self.velocity[2] + self.gravity * self.elapsed as f32 <= 0.
                    && self.goal.pre_update(
                        1,
                        center.map(|v| v as f32),
                        self.destination.map(|v| v as f32),
                        50. * 50.,
                    ) == jump_goal::Decision::Land
                {
                    *position = [center[0], center[1], center[2] + 0.02];
                    self.stage = 2;
                    self.clip = "Land".into();
                    self.frame = 0.;
                    return Ok((false, false));
                }
                return Ok((false, true));
            }
            *position = next;
            if self.elapsed >= f64::from(self.flight_time) {
                let Some(at) = ground(world, [next[0], next[1], next[2] + 0.1], EXTENT, 2.)? else {
                    return Ok((false, true));
                };
                if self.goal.pre_update(
                    1,
                    at.map(|v| v as f32),
                    self.destination.map(|v| v as f32),
                    50. * 50.,
                ) != jump_goal::Decision::Land
                {
                    return Ok((false, true));
                }
                *position = at;
                self.stage = 2;
                self.clip = "Land".into();
                self.frame = 0.;
            }
            return Ok((false, false));
        }
        let seq = &mesh.mesh.clips[mesh.clip(&self.clip)?].sequence;
        let last = 1. - 1. / seq.frames as f32;
        self.frame = (self.frame + f32::from_bits(seq.rate_bits) * STEP as f32 / seq.frames as f32)
            .min(last);
        if self.stage == 0 {
            let delta = root(mesh, &self.clip, self.frame, self.yaw)?;
            let next = std::array::from_fn(|i| self.origin[i] + delta[i]);
            if !clear(world, *position, next, EXTENT)? {
                return Ok((false, true));
            }
            *position = next;
        }
        if self.frame >= last {
            if self.node.is_some() && self.stage == 0 {
                self.stage = 1;
                self.clip = "FallBreathe".into();
                self.frame = 0.;
                self.goal.pre_update(
                    2,
                    position.map(|v| v as f32),
                    self.destination.map(|v| v as f32),
                    0.,
                );
                return Ok((false, false));
            }
            return Ok((true, false));
        }
        Ok((false, false))
    }
}

#[cfg(test)]
mod ground_tests {
    use super::*;
    use rc_package::hull::{Hull, HullSet};
    use std::sync::Arc;
    #[test]
    fn placement_search_skips_overlap_but_propagates_incomplete_geometry() {
        let world = StaticBodyWorld {
            world: Ok(Arc::new(HullSet {
                hulls: vec![Hull {
                    offset: 0,
                    bounds: [[-10., -10., 0.], [10., 10., 2.]],
                    planes: vec![],
                }],
                missing_solid_leaves: 0,
                empty_root_solid: false,
            })),
            actors: vec![],
        };
        assert_eq!(ground(&world, [0., 0., 1.], [1.; 3], 10.).unwrap(), None);
        let supported = ground(&world, [0., 0., 5.], [1.; 3], 10.).unwrap().unwrap();
        assert!((supported[2] - 3.02).abs() < 1e-8);
        let missing = StaticBodyWorld {
            world: Err("missing".into()),
            actors: vec![],
        };
        assert!(ground(&missing, [0., 0., 5.], [1.; 3], 10.).is_err());
    }
}
