//! Selected original compressed root translation; collision and goal lifecycle are host adapters.
use crate::{navigation, visual::PreparedMesh, Game, Point};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub seed: u32,
    pub provenance: serde_json::Value,
}
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct Motion {
    pub clip: usize,
    pub origin: Point,
    pub yaw: f64,
    pub frame: f32,
}
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct State {
    pub random: u32,
    pub draws: u64,
    pub started: u64,
    pub completed: u64,
    pub interrupted: u64,
    pub blocked: u64,
    pub motions: Vec<Option<Motion>>,
}
impl State {
    pub fn new(seed: u32, count: usize) -> Self {
        Self {
            random: seed,
            draws: 0,
            started: 0,
            completed: 0,
            interrupted: 0,
            blocked: 0,
            motions: vec![None; count],
        }
    }
    /// Deterministic encounter-owned MSVCRT-style stream; not the PC process RNG state.
    pub fn draw(&mut self) -> u16 {
        self.random = self.random.wrapping_mul(214013).wrapping_add(2531011);
        self.draws += 1;
        ((self.random >> 16) & 32767) as u16
    }
}
pub fn offset(mesh: &PreparedMesh, clip: usize, frame: f32, yaw: f64) -> Result<Point, String> {
    let root = mesh.root_position(clip, frame)?;
    let first = mesh.root_position(clip, 0.)?;
    let height = mesh.rest_bounds[1][2] - mesh.rest_bounds[0][2];
    if !height.is_finite() || height <= 0. {
        return Err("Invalid lunge mesh height".into());
    }
    let scale = 120. / f64::from(height);
    let local: Point = std::array::from_fn(|a| f64::from(root[a] - first[a]) * scale);
    let (sin, cos) = yaw.sin_cos();
    Ok([
        (local[0] * cos - local[1] * sin),
        (local[0] * sin + local[1] * cos),
        local[2],
    ])
}
pub fn point(mesh: &PreparedMesh, motion: &Motion, frame: f32) -> Result<Point, String> {
    let delta = offset(mesh, motion.clip, frame, motion.yaw)?;
    Ok(std::array::from_fn(|a| motion.origin[a] + delta[a]))
}
fn segment(
    world: &rc_package::world_collision::StaticBodyWorld,
    from: Point,
    to: Point,
    base: f64,
) -> Result<bool, String> {
    let sweep = world.sweep_motion(from, to, navigation::EXTENT)?;
    if !sweep.complete {
        return Err("Incomplete lunge sweep".into());
    }
    if !sweep.contacts.is_empty() {
        return Ok(false);
    }
    // Require floor beneath the airborne curve. Unsupported stairs/drops remain disabled.
    navigation::walkable(world, [from[0], from[1], base], [to[0], to[1], base])
}
pub fn feasible(
    world: &rc_package::world_collision::StaticBodyWorld,
    mesh: &PreparedMesh,
    motion: &Motion,
) -> Result<bool, String> {
    let seq = &mesh.mesh.clips[motion.clip].sequence;
    // Integer key durations make root translation piecewise linear between
    // original frame boundaries. Sweep those segments, including long dwell time.
    let steps = usize::try_from(seq.frames - 1).map_err(|_| "Invalid lunge frames")?;
    if steps == 0 || steps > 300 {
        return Err("Lunge exceeds motion budget".into());
    }
    let last = 1. - 1. / seq.frames as f32;
    let mut previous = motion.origin;
    for i in 1..=steps {
        let next = point(
            mesh,
            motion,
            if i == steps {
                last
            } else {
                i as f32 / seq.frames as f32
            },
        )?;
        if !segment(world, previous, next, motion.origin[2])? {
            return Ok(false);
        }
        previous = next;
    }
    Ok(true)
}
pub fn settle(
    world: &rc_package::world_collision::StaticBodyWorld,
    position: Point,
) -> Result<Point, String> {
    Ok(navigation::ground(world, position, 256.)?.unwrap_or(position))
}
impl Game {
    pub(crate) fn lunge_motion_tick(&mut self) -> Result<(), String> {
        let Some(state) = self.state.combat.as_ref().and_then(|c| c.lunge.as_ref()) else {
            return Ok(());
        };
        let mesh = &self.visuals.as_ref().ok_or("Missing lunge mesh")?.enemy;
        let visuals = self
            .state
            .visuals
            .as_ref()
            .ok_or("Missing lunge animation state")?;

        // Collect first: all scene errors still roll the entire fixed tick back in Game::advance.
        let motions = state.motions.clone();
        for (i, motion) in motions.into_iter().enumerate() {
            let Some(mut motion) = motion else {
                continue;
            };
            let player = &visuals.enemies[i];
            let expected = &mesh.mesh.clips[motion.clip].sequence.name.name;
            let active = self
                .state
                .combat
                .as_ref()
                .unwrap()
                .melee
                .as_ref()
                .unwrap()
                .attacks[i]
                .active
                .is_some_and(|a| a >= 2);
            let frame = player.frame;
            let playing = &mesh.mesh.clips[player.clip].sequence.name.name == expected;
            let next = point(mesh, &motion, frame)?;
            let okay = active
                && playing
                && segment(
                    &self.world,
                    self.state.enemies[i].position,
                    next,
                    motion.origin[2],
                )?;
            let state = self.state.combat.as_mut().unwrap().lunge.as_mut().unwrap();
            if !okay {
                self.state.enemies[i].position =
                    settle(&self.world, self.state.enemies[i].position)?;
                state.motions[i] = None;
                state.blocked += 1;
                self.state
                    .combat
                    .as_mut()
                    .unwrap()
                    .melee
                    .as_mut()
                    .unwrap()
                    .attacks[i]
                    .active = None;
            } else {
                self.state.enemies[i].position = next;
                motion.frame = frame;
                if player.finished {
                    self.state.enemies[i].position = settle(&self.world, next)?;
                    state.motions[i] = None;
                    state.completed += 1;
                } else {
                    state.motions[i] = Some(motion);
                }
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encounter_random_sequence_and_wrap_are_explicit() {
        let mut state = State::new(1, 1);
        assert_eq!(
            (state.draw(), state.draw(), state.draw()),
            (41, 18467, 6334)
        );
        assert_eq!(state.draws, 3);
        let mut state = State::new(u32::MAX, 1);
        assert!(state.draw() <= 32767);
    }
}
