//! Isolated walking calcVelocity arithmetic, not a complete native Pawn/controller.
//! engine.dll 1048f900, walking caller 10492b60 (flags fluid=0, brake=1, buoyancy=0).
use crate::{
    controller::MovementRatios,
    physics::{BodyFrame, BodyState, PhysicsOptions},
    world_collision::StaticBodyWorld,
};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SpeedFactors {
    pub ratios: MovementRatios,
    pub crouch_ratio: f64,
    pub wounded_ratio: f64,
    pub walking: bool,
    pub crouched: bool,
    /// Caller supplies native health-affects-gameplay && HealthLevel < 2 condition.
    pub wounded: bool,
    /// Effective human weapon modifier, or one for the native nonhuman/no-weapon path.
    pub weapon_modifier: f64,
}
impl SpeedFactors {
    fn validate(self) -> Result<(), String> {
        if [
            self.ratios.walk,
            self.ratios.back,
            self.ratios.side,
            self.crouch_ratio,
            self.wounded_ratio,
        ]
        .iter()
        .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            || !self.weapon_modifier.is_finite()
            || !(0.0..=1000.0).contains(&self.weapon_modifier)
        {
            return Err("invalid SpeedFactor diagnostic ratios/modifier".into());
        }
        Ok(())
    }
    /// SpeedFactor 1048cd00 arithmetic on an already local direction; matrix/SSE parity absent.
    pub fn local_factor(self, direction: [f64; 3]) -> Result<f64, String> {
        self.validate()?;
        if direction.iter().any(|v| !v.is_finite() || v.abs() > 1e6) {
            return Err("invalid SpeedFactor direction".into());
        }
        let length = direction[0].hypot(direction[1]).hypot(direction[2]);
        if length == 0.0 {
            return Ok(0.0);
        }
        let mut local = direction.map(|v| v / length);
        if local[0] < 0.0 {
            local[0] *= self.ratios.back;
        }
        local[1] *= self.ratios.side;
        let state_factor = if self.crouched {
            self.crouch_ratio
        } else {
            (if self.walking { self.ratios.walk } else { 1.0 })
                * (if self.wounded {
                    self.wounded_ratio
                } else {
                    1.0
                })
        };
        Ok(local[0].hypot(local[1]).hypot(local[2]) * state_factor * self.weapon_modifier)
    }
    /// Diagnostic inverse yaw, ignoring pitch/roll and using f64 instead of native matrix floats.
    pub fn yaw_factor(self, direction: [f64; 2], yaw: i32) -> Result<f64, String> {
        let angle = f64::from((yaw >> 2) & 16383) * std::f64::consts::TAU / 16384.0;
        let (sin, cos) = angle.sin_cos();
        self.local_factor([
            direction[0] * cos + direction[1] * sin,
            -direction[0] * sin + direction[1] * cos,
            0.0,
        ])
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct WalkingDiagnosticOptions {
    pub base_acceleration: f64,
    pub base_speed: f64,
    /// Explicit runtime snapshot supplied by caller, never guessed from a class default.
    pub maximum_desired_speed: f64,
    pub ground_friction: f64,
    pub yaw: i32,
    pub factors: SpeedFactors,
}
#[derive(Debug, Serialize)]
pub struct WalkingDiagnosticFrame {
    /// Computed velocity before own body collision projection; internal dispatch snapshot.
    #[serde(skip)]
    pub planned_velocity: [f64; 3],
    pub motion: BodyFrame,
    pub walking_applied: bool,
    pub acceleration_factor: Option<f64>,
    pub velocity_factor: Option<f64>,
}
impl StaticBodyWorld {
    /// Diagnostic bridge: native walking arithmetic plus own static body solver.
    /// Unsupported airborne acceleration is an error; passive falling uses the existing solver.
    pub fn walking_diagnostic_tick(
        &self,
        state: &BodyState,
        acceleration: [f64; 2],
        dt: f64,
        options: WalkingDiagnosticOptions,
        physics: PhysicsOptions,
    ) -> Result<WalkingDiagnosticFrame, String> {
        options.factors.validate()?;
        if acceleration.iter().any(|v| !v.is_finite() || v.abs() > 1e6)
            || [
                options.base_acceleration,
                options.base_speed,
                options.maximum_desired_speed,
                options.ground_friction,
            ]
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=1e6).contains(v))
        {
            return Err("invalid walking diagnostic options/acceleration".into());
        }
        let supported = state.velocity[2] <= 0.0
            && self
                .floor_contact(
                    state.position,
                    physics.extent,
                    physics.support_distance,
                    physics.minimum_up,
                )?
                .is_some();
        let mut body = *state;
        let (acceleration_factor, velocity_factor) = if supported {
            let acceleration_factor = options.factors.yaw_factor(acceleration, options.yaw)?;
            let velocity = walking_velocity(
                [state.velocity[0], state.velocity[1]],
                acceleration,
                dt,
                WalkingVelocityOptions {
                    acceleration_limit: options.base_acceleration * acceleration_factor,
                    // Defer final cap until the actual new velocity direction is known.
                    speed_limit: 1e6,
                    ground_friction: options.ground_friction,
                    gravity_magnitude: physics.gravity,
                },
            )?;
            let velocity_factor = options.factors.yaw_factor(velocity, options.yaw)?;
            let cap = (options.base_speed * velocity_factor).min(options.maximum_desired_speed);
            let speed = velocity[0].hypot(velocity[1]);
            let retained = if speed > cap { cap / speed } else { 1.0 };
            body.velocity = [velocity[0] * retained, velocity[1] * retained, 0.0];
            (Some(acceleration_factor), Some(velocity_factor))
        } else {
            if acceleration != [0.0; 2] {
                return Err("walking diagnostic does not implement airborne acceleration".into());
            }
            (None, None)
        };
        Ok(WalkingDiagnosticFrame {
            planned_velocity: body.velocity,
            motion: self.body_tick(&body, dt, physics)?,
            walking_applied: supported,
            acceleration_factor,
            velocity_factor,
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct WalkingVelocityOptions {
    /// Caller supplies the effective acceleration limit, including SpeedFactor and runtime modifiers.
    pub acceleration_limit: f64,
    /// Caller supplies the effective final speed cap, including runtime restrictions.
    pub speed_limit: f64,
    pub ground_friction: f64,
    /// Magnitude of the volume gravity vector; walking removes velocity/acceleration Z.
    pub gravity_magnitude: f64,
}

/// Pure XY subset before ModifyVelocity, with an externally supplied final cap.
/// Uses f64 hypot, not the original SSE reciprocal-square-root approximation.
pub fn walking_velocity(
    velocity: [f64; 2],
    acceleration: [f64; 2],
    dt: f64,
    options: WalkingVelocityOptions,
) -> Result<[f64; 2], String> {
    if !dt.is_finite()
        || dt <= 0.0
        || dt > 0.05
        || velocity
            .iter()
            .chain(acceleration.iter())
            .any(|v| !v.is_finite() || v.abs() > 1e6)
        || [
            options.acceleration_limit,
            options.speed_limit,
            options.ground_friction,
            options.gravity_magnitude,
        ]
        .iter()
        .any(|v| !v.is_finite() || !(0.0..=1e6).contains(v))
    {
        return Err("invalid walking velocity diagnostic input/options".into());
    }
    let acceleration_length = acceleration[0].hypot(acceleration[1]);
    let speed = velocity[0].hypot(velocity[1]);
    let mut result = if acceleration_length > 0.0 {
        let direction = acceleration.map(|v| v / acceleration_length);
        let limited_acceleration = acceleration_length.min(options.acceleration_limit);
        std::array::from_fn(|i| {
            velocity[i] - (velocity[i] - speed * direction[i]) * dt * options.ground_friction
                + direction[i] * limited_acceleration * dt
        })
    } else if speed > 0.0 {
        // Native float at VA106701b4 is 0.125, not Pawn.DecelRate.
        let retained = ((speed - options.gravity_magnitude * dt * options.ground_friction * 0.125)
            / speed)
            .max(0.0);
        velocity.map(|v| v * retained)
    } else {
        velocity
    };
    let speed = result[0].hypot(result[1]);
    if speed > options.speed_limit {
        result = result.map(|v| v * options.speed_limit / speed);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn options() -> WalkingVelocityOptions {
        WalkingVelocityOptions {
            acceleration_limit: 1024.0,
            speed_limit: 450.0,
            ground_friction: 8.0,
            gravity_magnitude: 1100.0,
        }
    }
    fn near(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 1e-9, "{actual} != {expected}");
    }
    fn factors() -> SpeedFactors {
        SpeedFactors {
            ratios: MovementRatios {
                walk: 0.5,
                back: 0.8,
                side: 0.95,
            },
            crouch_ratio: 0.4,
            wounded_ratio: 0.7,
            walking: false,
            crouched: false,
            wounded: false,
            weapon_modifier: 1.0,
        }
    }
    #[test]
    fn speed_factor_uses_direction_state_precedence_and_weapon_modifier() {
        let f = factors();
        near(f.local_factor([1.0, 0.0, 0.0]).unwrap(), 1.0);
        near(f.local_factor([-1.0, 0.0, 0.0]).unwrap(), 0.8);
        near(f.local_factor([0.0, 9.0, 0.0]).unwrap(), 0.95);
        near(
            f.local_factor([1.0, 1.0, 0.0]).unwrap(),
            ((1.0 + 0.95_f64.powi(2)) / 2.0).sqrt(),
        );
        near(
            SpeedFactors {
                walking: true,
                wounded: true,
                weapon_modifier: 0.8,
                ..f
            }
            .local_factor([1.0, 0.0, 0.0])
            .unwrap(),
            0.5 * 0.7 * 0.8,
        );
        near(
            SpeedFactors {
                crouched: true,
                walking: true,
                wounded: true,
                weapon_modifier: 0.8,
                ..f
            }
            .local_factor([1.0, 0.0, 0.0])
            .unwrap(),
            0.4 * 0.8,
        );
        near(f.yaw_factor([0.0, -1.0], 16384).unwrap(), 0.8);
        assert_eq!(f.local_factor([0.0; 3]).unwrap(), 0.0);
        assert!(f.local_factor([f64::NAN, 0.0, 0.0]).is_err());
        assert!(SpeedFactors {
            weapon_modifier: -1.0,
            ..f
        }
        .local_factor([0.0; 3])
        .is_err());
    }
    #[test]
    fn walking_bridge_checks_real_support_caps_and_atomic_failure() {
        use crate::hull::{Hull, HullSet};
        use std::sync::Arc;
        let world = StaticBodyWorld {
            world: Ok(Arc::new(HullSet {
                hulls: vec![Hull {
                    offset: 0,
                    planes: vec![],
                    bounds: [[-1000.0, -1000.0, -1.0], [1000.0, 1000.0, 0.0]],
                }],
                missing_solid_leaves: 0,
                empty_root_solid: false,
            })),
            actors: vec![],
        };
        let physics = PhysicsOptions {
            extent: [1.0; 3],
            gravity: 1100.0,
            terminal_speed: 12000.0,
            skin: 0.1,
            support_distance: 0.5,
            minimum_up: 0.7,
            max_iterations: 8,
            step_height: 0.0,
        };
        let options = WalkingDiagnosticOptions {
            base_acceleration: 1024.0,
            base_speed: 450.0,
            maximum_desired_speed: 100.0,
            ground_friction: 8.0,
            yaw: 0,
            factors: factors(),
        };
        let state = BodyState {
            position: [0.0, 0.0, 1.1],
            velocity: [450.0, 0.0, 0.0],
            grounded: false,
        };
        let frame = world
            .walking_diagnostic_tick(&state, [0.0; 2], 1.0 / 60.0, options, physics)
            .unwrap();
        assert!(frame.walking_applied);
        near(frame.motion.body.velocity[0], 100.0);
        assert!(frame.motion.body.grounded);
        let airborne = BodyState {
            position: [0.0, 0.0, 20.0],
            grounded: true,
            ..state
        };
        assert!(world
            .walking_diagnostic_tick(&airborne, [1.0, 0.0], 0.01, options, physics)
            .is_err());
        let frame = world
            .walking_diagnostic_tick(&airborne, [0.0; 2], 0.01, options, physics)
            .unwrap();
        assert!(!frame.walking_applied);
        near(frame.motion.body.velocity[0], 450.0);
        near(frame.motion.body.velocity[2], -11.0);
        let missing = StaticBodyWorld {
            world: Err("missing world".into()),
            actors: vec![],
        };
        assert!(missing
            .walking_diagnostic_tick(&state, [0.0; 2], 0.01, options, physics)
            .is_err());
        near(state.velocity[0], 450.0);
    }
    #[test]
    fn gravity_friction_braking_preserves_direction_and_never_reverses() {
        let mut o = options();
        let v = walking_velocity([270.0, 360.0], [0.0; 2], 1.0 / 60.0, o).unwrap();
        near(v[0], 259.0);
        near(v[1], 345.3333333333333);
        o.gravity_magnitude = 110.0;
        near(
            walking_velocity([450.0, 0.0], [0.0; 2], 1.0 / 60.0, o).unwrap()[0],
            448.1666666666667,
        );
        assert_eq!(
            walking_velocity([1.0, 0.0], [0.0; 2], 0.05, o).unwrap(),
            [0.0; 2]
        );
        o.gravity_magnitude = 0.0;
        assert_eq!(
            walking_velocity([90.0, 120.0], [0.0; 2], 0.05, o).unwrap(),
            [90.0, 120.0]
        );
        o.gravity_magnitude = 1100.0;
        o.ground_friction = 0.0;
        assert_eq!(
            walking_velocity([90.0, 120.0], [0.0; 2], 0.05, o).unwrap(),
            [90.0, 120.0]
        );
    }
    #[test]
    fn turning_steers_existing_velocity_before_adding_clamped_acceleration() {
        let v = walking_velocity([300.0, 0.0], [0.0, 2048.0], 1.0 / 60.0, options()).unwrap();
        near(v[0], 260.0);
        near(v[1], 40.0 + 1024.0 / 60.0);
        let v = walking_velocity([0.0; 2], [512.0, 0.0], 1.0 / 60.0, options()).unwrap();
        near(v[0], 512.0 / 60.0);
    }
    #[test]
    fn cap_is_vector_based_and_stop_is_exact() {
        let mut o = options();
        o.speed_limit = 100.0;
        let v = walking_velocity([300.0, 400.0], [0.0; 2], 1.0 / 60.0, o).unwrap();
        near(v[0], 60.0);
        near(v[1], 80.0);
        o.speed_limit = 0.0;
        assert_eq!(
            walking_velocity([10.0, 0.0], [100.0, 0.0], 0.05, o).unwrap(),
            [0.0; 2]
        );
        assert_eq!(
            walking_velocity([0.0; 2], [0.0; 2], 0.05, o).unwrap(),
            [0.0; 2]
        );
    }
    #[test]
    fn invalid_inputs_are_explicit_errors() {
        let o = options();
        for dt in [0.0, -0.01, 0.051, f64::NAN] {
            assert!(walking_velocity([0.0; 2], [0.0; 2], dt, o).is_err());
        }
        assert!(walking_velocity([f64::INFINITY, 0.0], [0.0; 2], 0.01, o).is_err());
        assert!(walking_velocity([0.0; 2], [f64::NAN, 0.0], 0.01, o).is_err());
        for bad in [-1.0, f64::NAN, f64::INFINITY] {
            assert!(walking_velocity(
                [0.0; 2],
                [0.0; 2],
                0.01,
                WalkingVelocityOptions {
                    ground_friction: bad,
                    ..o
                }
            )
            .is_err());
        }
    }
}
