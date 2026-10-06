//! Explicit PC integration policy, not reconstructed native walking/falling physics.
use crate::{
    movement::{MoveState, Movement, PlacementState},
    world_collision::{BodyContact, StaticBodyWorld},
};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct BodyState {
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub grounded: bool,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub struct PhysicsOptions {
    pub extent: [f64; 3],
    pub gravity: f64,
    pub terminal_speed: f64,
    pub skin: f64,
    pub support_distance: f64,
    pub minimum_up: f64,
    pub max_iterations: usize,
    pub step_height: f64,
}
#[derive(Debug, Serialize)]
pub struct BodyFrame {
    pub body: BodyState,
    pub contacts: Vec<BodyContact>,
    pub support: Option<BodyContact>,
    pub step: Option<StepUp>,
}
#[derive(Debug, Serialize)]
pub struct StepUp {
    pub rise: f64,
    pub support: BodyContact,
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
impl StaticBodyWorld {
    fn try_step(
        &self,
        start: [f64; 3],
        displacement: [f64; 3],
        baseline: &Movement,
        options: PhysicsOptions,
    ) -> Result<Option<(Movement, StepUp)>, String> {
        let Some(start_support) = self.floor_contact(
            start,
            options.extent,
            options.support_distance,
            options.minimum_up,
        )?
        else {
            return Ok(None);
        };
        let lift = options.step_height + options.skin;
        let raised = [start[0], start[1], start[2] + lift];
        let upward = self.sweep_motion(start, raised, options.extent)?;
        if !upward.complete {
            return Err(format!("step upward query incomplete: {:?}", upward.errors));
        }
        if !upward.contacts.is_empty() {
            return Ok(None);
        }
        let mut forward = self.slide_body(
            raised,
            displacement,
            options.extent,
            options.skin,
            options.max_iterations,
        )?;
        if matches!(
            forward.state,
            MoveState::InitialContact | MoveState::IterationLimit
        ) {
            return Ok(None);
        }
        let progress = |position: [f64; 3]| -> f64 {
            (0..2)
                .map(|i| (position[i] - start[i]) * displacement[i])
                .sum()
        };
        if progress(forward.position) <= progress(baseline.position) + 1e-8 {
            return Ok(None);
        }
        let distance = lift + options.support_distance;
        let Some(support) = self.floor_contact(
            forward.position,
            options.extent,
            distance,
            options.minimum_up,
        )?
        else {
            return Ok(None);
        };
        let landing_center = forward.position[2] - distance * support.hit.fraction;
        let starting_center = start[2] - options.support_distance * start_support.hit.fraction;
        let rise = landing_center - starting_center;
        let drop =
            (distance * support.hit.fraction - options.skin / support.hit.normal[2]).max(0.0);
        forward.position[2] -= drop;
        // Own progress tolerance suppresses serialized-plane noise near an unchanged floor.
        if rise <= 1e-4 || rise > options.step_height + 1e-8 {
            return Ok(None);
        }
        if self.body_placement(forward.position, options.extent)?.state
            == PlacementState::Penetrating
        {
            return Ok(None);
        }
        Ok(Some((forward, StepUp { rise, support })))
    }
    /// Integrates one bounded step and returns a candidate atomically; input is never changed.
    /// Caller supplies velocity. Acceleration, friction, jumping and native volume rules are absent.
    pub fn body_tick(
        &self,
        body: &BodyState,
        dt: f64,
        options: PhysicsOptions,
    ) -> Result<BodyFrame, String> {
        if !dt.is_finite()
            || !(0.0..=0.05).contains(&dt)
            || dt == 0.0
            || body
                .velocity
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1e6)
            || !options.gravity.is_finite()
            || !(0.0..=10000.0).contains(&options.gravity)
            || !options.terminal_speed.is_finite()
            || !(0.0..=1e6).contains(&options.terminal_speed)
            || options.terminal_speed == 0.0
            || !options.support_distance.is_finite()
            || options.support_distance <= options.skin
            || options.support_distance > 100.0
            || !options.minimum_up.is_finite()
            || !(0.01..=1.0).contains(&options.minimum_up)
            || !options.step_height.is_finite()
            || !(0.0..=100.0).contains(&options.step_height)
        {
            return Err("invalid body tick options".into());
        }
        if self.body_placement(body.position, options.extent)?.state == PlacementState::Penetrating
        {
            return Err("body tick starts penetrating geometry".into());
        }
        let displacement = [body.velocity[0] * dt, body.velocity[1] * dt, 0.0];
        let mut horizontal = self.slide_body(
            body.position,
            displacement,
            options.extent,
            options.skin,
            options.max_iterations,
        )?;
        if matches!(
            horizontal.state,
            MoveState::InitialContact | MoveState::IterationLimit
        ) {
            return Err("horizontal body tick could not finish safely".into());
        }
        let mut step = None;
        if options.step_height > 0.0
            && body.velocity[2] <= 0.0
            && horizontal
                .contacts
                .iter()
                .any(|c| c.hit.normal[2] < options.minimum_up)
            && self
                .floor_contact(
                    body.position,
                    options.extent,
                    options.support_distance,
                    options.minimum_up,
                )?
                .is_some()
        {
            if let Some((candidate, event)) =
                self.try_step(body.position, displacement, &horizontal, options)?
            {
                horizontal = candidate;
                step = Some(event);
            }
        }
        let mut velocity = body.velocity;
        // Horizontal velocity clips stay horizontal; do not create upward launch velocity at walls.
        for contact in &horizontal.contacts {
            let n = [contact.hit.normal[0], contact.hit.normal[1], 0.0];
            let length = dot(n, n);
            if length > 1e-12 {
                let inward = dot(velocity, n).min(0.0) / length;
                velocity = std::array::from_fn(|i| velocity[i] - inward * n[i]);
            }
        }
        let support = if velocity[2] <= 0.0 {
            self.floor_contact(
                horizontal.position,
                options.extent,
                options.support_distance,
                options.minimum_up,
            )?
        } else {
            None
        };
        velocity[2] = if support.is_some() {
            0.0
        } else {
            (velocity[2] - options.gravity * dt).max(-options.terminal_speed)
        };
        let vertical = self.slide_body(
            horizontal.position,
            [0.0, 0.0, velocity[2] * dt],
            options.extent,
            options.skin,
            options.max_iterations,
        )?;
        if matches!(
            vertical.state,
            MoveState::InitialContact | MoveState::IterationLimit
        ) {
            return Err("vertical body tick could not finish safely".into());
        }
        for contact in &vertical.contacts {
            let n = contact.hit.normal;
            let inward = dot(velocity, n).min(0.0);
            velocity = std::array::from_fn(|i| velocity[i] - inward * n[i]);
        }
        let mut position = vertical.position;
        let support = if velocity[2] <= 1e-9 {
            self.floor_contact(
                position,
                options.extent,
                options.support_distance,
                options.minimum_up,
            )?
        } else {
            None
        };
        if let Some(contact) = &support {
            // Snap downward only; the full-body floor sweep has checked this path.
            let drop = (options.support_distance * contact.hit.fraction
                - options.skin / contact.hit.normal[2])
                .max(0.0);
            position[2] -= drop;
            velocity[2] = 0.0;
        }
        if self.body_placement(position, options.extent)?.state == PlacementState::Penetrating {
            return Err("body tick candidate penetrates geometry".into());
        }
        let mut contacts = horizontal.contacts;
        contacts.extend(vertical.contacts);
        Ok(BodyFrame {
            body: BodyState {
                position,
                velocity,
                grounded: support.is_some(),
            },
            contacts,
            support,
            step,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        hull::{Hull, HullSet},
        world_collision::{BodyActor, BodyShape},
    };
    use std::sync::Arc;
    fn world(bounds: Vec<[[f64; 3]; 2]>) -> StaticBodyWorld {
        StaticBodyWorld {
            world: Ok(Arc::new(HullSet {
                hulls: bounds
                    .into_iter()
                    .enumerate()
                    .map(|(offset, bounds)| Hull {
                        offset,
                        planes: vec![],
                        bounds,
                    })
                    .collect(),
                missing_solid_leaves: 0,
                empty_root_solid: false,
            })),
            actors: vec![],
        }
    }
    fn options() -> PhysicsOptions {
        PhysicsOptions {
            extent: [1.0; 3],
            gravity: 100.0,
            terminal_speed: 100.0,
            skin: 0.1,
            support_distance: 0.5,
            minimum_up: 0.7,
            max_iterations: 8,
            step_height: 0.0,
        }
    }
    #[test]
    fn falling_lands_and_remains_stable_for_many_ticks() {
        let w = world(vec![[[-100.0, -100.0, -0.1], [100.0, 100.0, 0.0]]]);
        let mut body = BodyState {
            position: [0.0, 0.0, 20.0],
            velocity: [0.0; 3],
            grounded: true,
        };
        let first = w.body_tick(&body, 0.02, options()).unwrap();
        assert!(!first.body.grounded && first.body.velocity[2] < 0.0);
        for _ in 0..150 {
            body = w.body_tick(&body, 0.02, options()).unwrap().body;
        }
        assert!(body.grounded);
        assert!((body.position[2] - 1.1).abs() < 1e-9);
        assert_eq!(body.velocity[2], 0.0);
        body.velocity[0] = 10.0;
        let moved = w.body_tick(&body, 0.02, options()).unwrap().body;
        assert!(moved.position[0] > body.position[0] && moved.grounded);
        assert_eq!(moved.position[2], body.position[2]);
    }
    #[test]
    fn leaving_ledge_falls_immediately_and_ceiling_clips_upward_velocity() {
        let w = world(vec![[[-10.0, -10.0, -1.0], [0.0, 10.0, 0.0]]]);
        let b = BodyState {
            position: [0.0, 0.0, 1.0],
            velocity: [100.0, 0.0, 0.0],
            grounded: true,
        };
        let r = w.body_tick(&b, 0.05, options()).unwrap().body;
        assert!(!r.grounded && r.velocity[2] < 0.0 && r.position[2] < 1.0);
        let ceiling = world(vec![[[-10.0, -10.0, 5.0], [10.0, 10.0, 6.0]]]);
        let b = BodyState {
            position: [0.0, 0.0, 2.0],
            velocity: [0.0, 0.0, 100.0],
            grounded: false,
        };
        let r = ceiling.body_tick(&b, 0.05, options()).unwrap().body;
        assert!(r.position[2] < 4.0 && r.velocity[2].abs() < 1e-9 && !r.grounded);
    }
    #[test]
    fn invalid_and_missing_geometry_do_not_produce_a_candidate() {
        let mut w = world(vec![]);
        let b = BodyState {
            position: [0.0; 3],
            velocity: [0.0, 0.0, -1000.0],
            grounded: false,
        };
        let r = w.body_tick(&b, 0.05, options()).unwrap().body;
        assert_eq!(r.velocity[2], -100.0);
        for dt in [0.0, -0.01, 0.1, f64::NAN] {
            assert!(w.body_tick(&b, dt, options()).is_err());
        }
        w.actors.push(BodyActor {
            name: "missing".into(),
            shape: BodyShape::Unsupported("missing model".into()),
        });
        assert!(w.body_tick(&b, 0.02, options()).is_err());
        assert_eq!(b.position, [0.0; 3]);
        assert_eq!(b.velocity[2], -1000.0);
    }
    fn staircase(height: f64) -> StaticBodyWorld {
        world(vec![
            [[-100.0, -100.0, -1.0], [100.0, 100.0, 0.0]],
            [[0.0, -100.0, 0.0], [100.0, 100.0, height]],
        ])
    }
    fn walker() -> BodyState {
        BodyState {
            position: [-2.0, 0.0, 1.1],
            velocity: [100.0, 0.0, 0.0],
            grounded: true,
        }
    }
    #[test]
    fn reachable_steps_preserve_forward_velocity_and_can_repeat() {
        let w = world(vec![
            [[-100.0, -100.0, -1.0], [100.0, 100.0, 0.0]],
            [[0.0, -100.0, 0.0], [4.0, 100.0, 1.0]],
            [[4.0, -100.0, 0.0], [100.0, 100.0, 2.0]],
        ]);
        let mut o = options();
        o.step_height = 1.0;
        let first = w.body_tick(&walker(), 0.04, o).unwrap();
        assert!(first.step.is_some() && first.body.grounded);
        assert!((first.body.position[2] - 2.1).abs() < 1e-9);
        assert_eq!(first.body.velocity[0], 100.0);
        let second = w.body_tick(&first.body, 0.05, o).unwrap();
        assert!(second.step.is_some() && second.body.grounded);
        assert!((second.body.position[2] - 3.1).abs() < 1e-9);
        assert!(second.body.position[0] > first.body.position[0]);
        o.step_height = 0.0;
        let blocked = w.body_tick(&walker(), 0.05, o).unwrap();
        assert!(blocked.step.is_none() && blocked.body.position[0] < 0.0);
    }
    #[test]
    fn high_steps_low_ceiling_and_airborne_bodies_cannot_step() {
        let mut o = options();
        o.step_height = 1.0;
        let high = staircase(2.0).body_tick(&walker(), 0.05, o).unwrap();
        assert!(high.step.is_none() && high.body.position[0] < 0.0);
        let mut hovering = walker();
        hovering.position[2] = 1.4;
        let too_high = staircase(1.2).body_tick(&hovering, 0.05, o).unwrap();
        assert!(too_high.step.is_none());
        let low = world(vec![
            [[-100.0, -100.0, -1.0], [100.0, 100.0, 0.0]],
            [[0.0, -100.0, 0.0], [100.0, 100.0, 1.0]],
            [[-100.0, -100.0, 3.0], [100.0, 100.0, 4.0]],
        ])
        .body_tick(&walker(), 0.05, o)
        .unwrap();
        assert!(low.step.is_none() && low.body.position[0] < 0.0);
        let mut air = walker();
        air.position[2] = 3.0;
        let result = staircase(3.0).body_tick(&air, 0.05, o).unwrap();
        assert!(result.step.is_none());
        let gap = world(vec![
            [[-100.0, -100.0, -1.0], [-1.0, 100.0, 0.0]],
            [[0.0, -100.0, 0.0], [0.1, 100.0, 1.0]],
        ])
        .body_tick(&walker(), 0.05, o)
        .unwrap();
        assert!(gap.step.is_none() && gap.body.position[0] < 0.0);
    }
}
