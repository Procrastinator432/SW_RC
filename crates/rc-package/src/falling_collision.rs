//! Diagnostic connection of reconstructed Falling arithmetic to the own static AABB solver.
use crate::{
    falling::{free_fall, FreeFallFrame, FreeFallOptions, FALLING_STEP},
    movement::{MoveState, PlacementState},
    physics::{BodyFrame, BodyState, PhysicsOptions},
    world_collision::{StaticBodyWorld, WorldSweep},
};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct FallingCollisionOptions {
    pub extent: [f64; 3],
    pub skin: f64,
    pub support_distance: f64,
    pub minimum_up: f64,
    pub max_iterations: usize,
}
impl From<PhysicsOptions> for FallingCollisionOptions {
    fn from(p: PhysicsOptions) -> Self {
        Self {
            extent: p.extent,
            skin: p.skin,
            support_distance: p.support_distance,
            minimum_up: p.minimum_up,
            max_iterations: p.max_iterations,
        }
    }
}
#[derive(Debug, Serialize)]
pub struct FallingCollisionFrame {
    pub air_control_probe: Option<WorldSweep>,
    /// Arithmetic for an unobstructed move; actual position/velocity are in motion.
    pub integration_plan: FreeFallFrame,
    pub motion: BodyFrame,
}
impl StaticBodyWorld {
    /// One substep only. Contact projection and support snapping are diagnostic policies.
    /// No native cylinder/trace flags, landing callbacks, time refund or automatic walking.
    pub fn falling_diagnostic_tick(
        &self,
        body: &BodyState,
        acceleration: [f64; 2],
        dt: f64,
        falling: FreeFallOptions,
        geometry: FallingCollisionOptions,
    ) -> Result<FallingCollisionFrame, String> {
        if !dt.is_finite()
            || dt <= 0.0
            || dt > FALLING_STEP
            || !geometry.skin.is_finite()
            || geometry.skin <= 0.0
            || geometry.skin > 100.0
            || !geometry.support_distance.is_finite()
            || geometry.support_distance <= geometry.skin
            || geometry.support_distance > 100.0
            || !geometry.minimum_up.is_finite()
            || !(0.01..=1.0).contains(&geometry.minimum_up)
            || !(1..=32).contains(&geometry.max_iterations)
            || falling.gravity[0] != 0.0
            || falling.gravity[1] != 0.0
            || falling.gravity[2] > 0.0
            || falling.zone_velocity != [0.0; 3]
        {
            return Err("unsupported falling collision diagnostic options".into());
        }
        // Preliminary arithmetic describes the required query, not a clear-query fallback.
        let mut plan = free_fall(body.velocity, acceleration, dt, Some(false), falling)?;
        if self.body_placement(body.position, geometry.extent)?.state == PlacementState::Penetrating
        {
            return Err("falling diagnostic starts penetrating geometry".into());
        }
        let air_control_probe = if let Some(displacement) = plan.lookahead_displacement {
            let end = std::array::from_fn(|i| body.position[i] + displacement[i]);
            let probe = self.sweep_motion(body.position, end, geometry.extent)?;
            if !probe.complete {
                return Err(format!("air-control query incomplete: {:?}", probe.errors));
            }
            if !probe.contacts.is_empty() {
                plan = free_fall(body.velocity, acceleration, dt, Some(true), falling)?;
            }
            Some(probe)
        } else {
            None
        };
        let movement = self.slide_body(
            body.position,
            plan.displacement,
            geometry.extent,
            geometry.skin,
            geometry.max_iterations,
        )?;
        if matches!(
            movement.state,
            MoveState::InitialContact | MoveState::IterationLimit
        ) {
            return Err("falling diagnostic movement could not finish safely".into());
        }
        let mut velocity = plan.substeps[0].velocity_before_terminal;
        for _ in 0..8 {
            for contact in &movement.contacts {
                let inward = (0..3)
                    .map(|i| velocity[i] * contact.hit.normal[i])
                    .sum::<f64>()
                    .min(0.0);
                for (v, n) in velocity.iter_mut().zip(contact.hit.normal) {
                    *v -= n * inward;
                }
            }
        }
        if movement
            .contacts
            .iter()
            .any(|c| (0..3).map(|i| velocity[i] * c.hit.normal[i]).sum::<f64>() < -1e-8)
        {
            return Err("falling diagnostic velocity projection did not converge".into());
        }
        let mut position = movement.position;
        let descending_floor_hit = plan.substeps[0].velocity_before_terminal[2] <= 0.0
            && movement
                .contacts
                .iter()
                .any(|c| c.hit.normal[2] >= geometry.minimum_up);
        let support = if velocity[2] <= 1e-9 || descending_floor_hit {
            self.floor_contact(
                position,
                geometry.extent,
                geometry.support_distance,
                geometry.minimum_up,
            )?
        } else {
            None
        };
        if let Some(contact) = &support {
            let drop = (geometry.support_distance * contact.hit.fraction
                - geometry.skin / contact.hit.normal[2])
                .max(0.0);
            position[2] -= drop;
            velocity[2] = 0.0;
        }
        // Preserve native free-path cap ordering: after the movement/contact policy.
        let speed = velocity[0].hypot(velocity[1]).hypot(velocity[2]);
        if speed > falling.terminal_speed {
            velocity = velocity.map(|v| v * falling.terminal_speed / speed);
        }
        if self.body_placement(position, geometry.extent)?.state == PlacementState::Penetrating {
            return Err("falling diagnostic candidate penetrates geometry".into());
        }
        Ok(FallingCollisionFrame {
            air_control_probe,
            integration_plan: plan,
            motion: BodyFrame {
                body: BodyState {
                    position,
                    velocity,
                    grounded: support.is_some(),
                },
                contacts: movement.contacts,
                support,
                step: None,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hull::{Hull, HullSet};
    use std::sync::Arc;
    fn world(bounds: Vec<[[f64; 3]; 2]>) -> StaticBodyWorld {
        StaticBodyWorld {
            world: Ok(Arc::new(HullSet {
                hulls: bounds
                    .into_iter()
                    .enumerate()
                    .map(|(offset, bounds)| Hull {
                        offset,
                        bounds,
                        planes: vec![],
                    })
                    .collect(),
                missing_solid_leaves: 0,
                empty_root_solid: false,
            })),
            actors: vec![],
        }
    }
    fn falling() -> FreeFallOptions {
        FreeFallOptions {
            acceleration_rate: 1024.0,
            air_control: 0.35,
            ground_speed: 450.0,
            terminal_speed: 12000.0,
            gravity: [0.0, 0.0, -1100.0],
            zone_velocity: [0.0; 3],
        }
    }
    fn geometry() -> FallingCollisionOptions {
        FallingCollisionOptions {
            extent: [1.0; 3],
            skin: 0.1,
            support_distance: 0.5,
            minimum_up: 0.7,
            max_iterations: 8,
        }
    }
    fn body() -> BodyState {
        BodyState {
            position: [0.0, 0.0, 20.0],
            velocity: [0.0; 3],
            grounded: false,
        }
    }
    fn near(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-8, "{a} != {b}");
    }
    #[test]
    fn descending_contact_lands_on_walkable_slope_despite_upward_projection() {
        let w = StaticBodyWorld {
            world: Ok(Arc::new(HullSet {
                hulls: vec![Hull {
                    offset: 0,
                    planes: vec![[-0.6, 0.0, 0.8, 0.0]],
                    bounds: [[-100.0; 3], [100.0; 3]],
                }],
                missing_solid_leaves: 0,
                empty_root_solid: false,
            })),
            actors: vec![],
        };
        let b = BodyState {
            position: [0.0, 0.0, 5.0],
            velocity: [100.0, 0.0, -20.0],
            grounded: false,
        };
        let r = w
            .falling_diagnostic_tick(
                &b,
                [0.0; 2],
                0.05,
                FreeFallOptions {
                    gravity: [0.0; 3],
                    ..falling()
                },
                geometry(),
            )
            .unwrap();
        assert!(r.motion.body.grounded);
        assert!(r.motion.support.is_some());
        near(r.motion.body.velocity[2], 0.0);
        assert_ne!(
            w.body_placement(r.motion.body.position, geometry().extent)
                .unwrap()
                .state,
            PlacementState::Penetrating
        );
    }
    #[test]
    fn clear_query_preserves_the_free_flight_result() {
        let w = world(vec![]);
        let b = body();
        let f = falling();
        let r = w
            .falling_diagnostic_tick(&b, [1024.0, 0.0], 0.05, f, geometry())
            .unwrap();
        let expected = free_fall(b.velocity, [1024.0, 0.0], 0.05, Some(false), f).unwrap();
        assert!(r.air_control_probe.unwrap().contacts.is_empty());
        for i in 0..3 {
            near(r.motion.body.velocity[i], expected.velocity[i]);
            near(
                r.motion.body.position[i],
                b.position[i] + expected.displacement[i],
            );
        }
    }
    #[test]
    fn wall_probe_disables_control_and_motion_clips_existing_velocity() {
        let w = world(vec![[[5.0, -100.0, 0.0], [6.0, 100.0, 100.0]]]);
        let b = BodyState {
            velocity: [100.0, 0.0, 0.0],
            ..body()
        };
        let r = w
            .falling_diagnostic_tick(&b, [1024.0, 0.0], 0.05, falling(), geometry())
            .unwrap();
        assert!(!r.air_control_probe.unwrap().contacts.is_empty());
        assert_eq!(r.integration_plan.effective_air_control, 0.0);
        assert!(r.motion.body.position[0] <= 3.9 + 1e-8);
        near(r.motion.body.velocity[0], 0.0);
        assert!(r.motion.body.velocity[2] < 0.0);
    }
    #[test]
    fn floors_land_and_ceiling_contacts_do_not_ground() {
        let floor = world(vec![[[-100.0, -100.0, -1.0], [100.0, 100.0, 0.0]]]);
        let mut b = body();
        for _ in 0..60 {
            let r = floor
                .falling_diagnostic_tick(&b, [0.0; 2], 1.0 / 60.0, falling(), geometry())
                .unwrap();
            b = r.motion.body;
        }
        assert!(b.grounded);
        near(b.position[2], 1.1);
        near(b.velocity[2], 0.0);
        let ceiling = world(vec![[[-100.0, -100.0, 25.0], [100.0, 100.0, 26.0]]]);
        let b = BodyState {
            velocity: [0.0, 0.0, 475.0],
            ..body()
        };
        let r = ceiling
            .falling_diagnostic_tick(&b, [0.0; 2], 0.05, falling(), geometry())
            .unwrap();
        assert!(!r.motion.body.grounded);
        assert!(r.motion.body.position[2] <= 23.9 + 1e-8);
        near(r.motion.body.velocity[2], 0.0);
    }
    #[test]
    fn incomplete_penetrating_and_unsupported_queries_have_no_candidate() {
        let b = body();
        let missing = StaticBodyWorld {
            world: Err("missing".into()),
            actors: vec![],
        };
        assert!(missing
            .falling_diagnostic_tick(&b, [0.0; 2], 0.01, falling(), geometry())
            .is_err());
        let solid = world(vec![[[-2.0, -2.0, 18.0], [2.0, 2.0, 22.0]]]);
        assert!(solid
            .falling_diagnostic_tick(&b, [0.0; 2], 0.01, falling(), geometry())
            .is_err());
        let empty = world(vec![]);
        assert!(empty
            .falling_diagnostic_tick(&b, [0.0; 2], 0.06, falling(), geometry())
            .is_err());
        assert!(empty
            .falling_diagnostic_tick(
                &b,
                [0.0; 2],
                0.01,
                FreeFallOptions {
                    zone_velocity: [1.0, 0.0, 0.0],
                    ..falling()
                },
                geometry()
            )
            .is_err());
        assert_eq!(b.position, [0.0, 0.0, 20.0]);
        assert_eq!(b.velocity, [0.0; 3]);
    }
}
