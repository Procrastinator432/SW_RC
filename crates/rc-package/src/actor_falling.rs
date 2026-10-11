//! Prepared AActor::physFalling arithmetic, engine.dll 10490c63..10490d71.
//! MoveActor, GetNetBuoyancy, volume selection and script callbacks are host boundaries.
//! With bBounce set, 104913cd skips velocity reconstruction and terminal-speed capping.

/// Values supplied by the current physics volume and GetNetBuoyancy.
#[derive(Clone, Copy, Debug)]
pub struct Environment {
    pub gravity: [f32; 3],
    pub zone_velocity: [f32; 3],
    pub buoyancy: f32,
    pub mass: f32,
    pub fluid_friction: f32,
}

/// GetNetBuoyancy(1048dd40), dry-volume branch, with the caller's zero-initialized
/// outputs. Only vertical gravity is compared; water uses a separate brush trace.
pub fn dry_buoyancy(
    gravity_z: f32,
    default_gravity_z: f32,
    buoyancy: f32,
    fluid_friction: f32,
) -> (f32, f32) {
    if gravity_z == default_gravity_z {
        (0., 0.)
    } else {
        (buoyancy * 0.5, fluid_friction * 0.5)
    }
}

/// Original SSE operation order. No projectile MaxSpeed or terminal-speed cap here.
pub fn step(
    velocity: [f32; 3],
    acceleration: [f32; 3],
    environment: Environment,
    dt: f32,
) -> ([f32; 3], [f32; 3]) {
    let gravity_scale = 1. - environment.buoyancy / environment.mass;
    let damping = 1. - environment.fluid_friction * dt;
    let velocity = std::array::from_fn(|i| {
        let force = acceleration[i] + environment.gravity[i] * gravity_scale;
        velocity[i] * damping + force * dt
    });
    let displacement = std::array::from_fn(|i| (environment.zone_velocity[i] + velocity[i]) * dt);
    (velocity, displacement)
}

/// Native per-call iteration budget; bouncing can refund the unused move time twice.
/// Remaining time after iteration eight is discarded by the original routine.
#[derive(Clone, Copy, Debug)]
pub struct Budget {
    remaining: f32,
    iterations: u8,
    bounces: u32,
}
impl Budget {
    pub fn new(seconds: f32, previous_iterations: u8) -> Self {
        Self {
            remaining: seconds,
            iterations: previous_iterations,
            bounces: 0,
        }
    }
    pub fn next_step(&mut self) -> Option<f32> {
        if self.remaining <= 0. || self.iterations >= 8 {
            return None;
        }
        self.iterations += 1;
        let dt = if self.remaining > 0.05 {
            (self.remaining * 0.5).min(0.05)
        } else {
            self.remaining
        };
        self.remaining -= dt;
        Some(dt)
    }
    /// Call after HitWall only when physics remains active. Fraction is MoveActor's
    /// fraction for this step, not a global fraction of the whole frame.
    #[allow(clippy::assign_op_pattern)] // Preserve 10490edb..10490ee0 operand order.
    pub fn bounce(&mut self, dt: f32, hit_fraction: f32) {
        if self.bounces < 2 {
            self.remaining = (1. - hit_fraction) * dt + self.remaining;
        }
        self.bounces += 1;
    }
    pub fn remaining(&self) -> f32 {
        self.remaining
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn air() -> Environment {
        Environment {
            gravity: [0., 0., -950.],
            zone_velocity: [0.; 3],
            buoyancy: 0.,
            mass: 100.,
            fluid_friction: 0.,
        }
    }
    #[test]
    fn default_dry_gravity_preserves_zero_outputs_even_with_configured_fluid_friction() {
        assert_eq!(dry_buoyancy(-950., -950., 100., 0.3), (0., 0.));
    }
    #[test]
    fn nondefault_dry_gravity_uses_half_actor_buoyancy_and_volume_friction() {
        assert_eq!(dry_buoyancy(-400., -950., 100., 2.), (50., 1.));
    }
    #[test]
    fn bounce_path_keeps_speed_above_projectile_and_terminal_caps() {
        let (v, d) = step([4000., 0., -5000.], [0.; 3], air(), 0.02);
        assert_eq!(v, [4000., 0., -5019.]);
        assert_eq!(d, [80., 0., -100.38]);
    }
    #[test]
    fn buoyancy_damping_and_zone_velocity_have_distinct_roles() {
        let mut e = air();
        e.buoyancy = 100.;
        e.fluid_friction = 2.;
        e.zone_velocity = [100., -50., 20.];
        let (v, d) = step([10., 20., 30.], [4., 8., 12.], e, 0.25);
        assert_eq!(v, [6., 12., 18.]);
        assert_eq!(d, [26.5, -9.5, 9.5]);
    }
    #[test]
    fn large_frame_has_eight_step_ceiling() {
        let mut b = Budget::new(1., 0);
        for _ in 0..8 {
            assert_eq!(b.next_step(), Some(0.05));
        }
        assert_eq!(b.next_step(), None);
        assert!(b.remaining() > 0.59);
        assert!(Budget::new(0.02, 8).next_step().is_none());
    }
    #[test]
    fn small_remainder_is_halved_before_final_step() {
        let mut b = Budget::new(0.08, 0);
        assert_eq!(b.next_step(), Some(0.04));
        assert_eq!(b.next_step(), Some(0.04));
        assert_eq!(b.next_step(), None);
    }
    #[test]
    fn only_first_two_bounces_refund_time() {
        let mut b = Budget::new(0.04, 0);
        let dt = b.next_step().unwrap();
        b.bounce(dt, 0.25);
        assert_eq!(b.remaining(), 0.03);
        let dt = b.next_step().unwrap();
        b.bounce(dt, 0.);
        let dt = b.next_step().unwrap();
        b.bounce(dt, 0.);
        assert_eq!(b.next_step(), None);
    }
}
