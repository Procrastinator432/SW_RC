//! PC input policy atop static body physics; not original player-controller parity.
use crate::{
    physics::{BodyFrame, BodyState, PhysicsOptions},
    world_collision::StaticBodyWorld,
};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ControlledBody {
    pub body: BodyState,
    pub jump_held: bool,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub struct MovementInput {
    /// World XY axes, individual components in [-1,1]; diagonal length is clamped to one.
    pub direction: [f64; 2],
    pub jump: bool,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ControllerOptions {
    pub speed: f64,
    pub acceleration: f64,
    pub braking: f64,
    pub air_control: f64,
    pub jump_speed: f64,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub struct MovementRatios {
    pub walk: f64,
    pub back: f64,
    pub side: f64,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ViewInput {
    pub forward: f64,
    pub strafe: f64,
    pub yaw: i32,
    pub walking: bool,
    pub jump: bool,
}
impl ViewInput {
    /// Own directional weighting policy, using quantized Unreal yaw and ignoring view pitch.
    pub fn world_input(self, ratios: MovementRatios) -> Result<MovementInput, String> {
        if [self.forward, self.strafe]
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 1.0)
            || [ratios.walk, ratios.back, ratios.side]
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err("invalid view movement input/ratios".into());
        }
        let divisor = self.forward.hypot(self.strafe).max(1.0);
        let walking = if self.walking { ratios.walk } else { 1.0 };
        let forward =
            self.forward / divisor * if self.forward < 0.0 { ratios.back } else { 1.0 } * walking;
        let side = self.strafe / divisor * ratios.side * walking;
        let angle = f64::from((self.yaw >> 2) & 16383) * std::f64::consts::TAU / 16384.0;
        let (sin, cos) = angle.sin_cos();
        Ok(MovementInput {
            direction: [
                (forward * cos - side * sin).clamp(-1.0, 1.0),
                (forward * sin + side * cos).clamp(-1.0, 1.0),
            ],
            jump: self.jump,
        })
    }
}
#[derive(Debug, Serialize)]
pub struct ControllerFrame {
    pub state: ControlledBody,
    pub motion: BodyFrame,
    pub jump_started: bool,
}
fn approach(current: [f64; 2], target: [f64; 2], amount: f64) -> [f64; 2] {
    let difference = [target[0] - current[0], target[1] - current[1]];
    let length = difference[0].hypot(difference[1]);
    if length <= amount {
        target
    } else {
        std::array::from_fn(|i| current[i] + difference[i] * amount / length)
    }
}
impl StaticBodyWorld {
    /// Returns a whole new state only after input and collision queries succeed.
    pub fn control_tick(
        &self,
        state: &ControlledBody,
        input: MovementInput,
        dt: f64,
        controller: ControllerOptions,
        physics: PhysicsOptions,
    ) -> Result<ControllerFrame, String> {
        if !dt.is_finite()
            || dt <= 0.0
            || dt > 0.05
            || input
                .direction
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1.0)
            || [
                controller.speed,
                controller.acceleration,
                controller.braking,
                controller.jump_speed,
            ]
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.0 || *v > 1e6)
            || !controller.air_control.is_finite()
            || !(0.0..=1.0).contains(&controller.air_control)
            || state
                .body
                .velocity
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1e6)
        {
            return Err("invalid controller input/options".into());
        }
        let supported = state.body.velocity[2] <= 0.0
            && self
                .floor_contact(
                    state.body.position,
                    physics.extent,
                    physics.support_distance,
                    physics.minimum_up,
                )?
                .is_some();
        let length = input.direction[0].hypot(input.direction[1]);
        let wish = input.direction.map(|v| v / length.max(1.0));
        let current = [state.body.velocity[0], state.body.velocity[1]];
        let horizontal = if length > 0.0 {
            approach(
                current,
                wish.map(|v| v * controller.speed),
                controller.acceleration
                    * dt
                    * if supported {
                        1.0
                    } else {
                        controller.air_control
                    },
            )
        } else if supported {
            approach(current, [0.0; 2], controller.braking * dt)
        } else {
            current
        };
        let speed = horizontal[0].hypot(horizontal[1]);
        let horizontal = horizontal.map(|v| v * (controller.speed / speed.max(controller.speed)));
        let jump_started = input.jump && !state.jump_held && supported;
        let mut body = state.body;
        body.velocity[0] = horizontal[0];
        body.velocity[1] = horizontal[1];
        if jump_started {
            body.velocity[2] = controller.jump_speed;
        }
        let motion = self.body_tick(&body, dt, physics)?;
        Ok(ControllerFrame {
            state: ControlledBody {
                body: motion.body,
                jump_held: input.jump,
            },
            motion,
            jump_started,
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
    fn world() -> StaticBodyWorld {
        StaticBodyWorld {
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
        }
    }
    fn physics() -> PhysicsOptions {
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
    fn options() -> ControllerOptions {
        ControllerOptions {
            speed: 10.0,
            acceleration: 20.0,
            braking: 40.0,
            air_control: 0.25,
            jump_speed: 20.0,
        }
    }
    fn state() -> ControlledBody {
        ControlledBody {
            body: BodyState {
                position: [0.0, 0.0, 1.1],
                velocity: [0.0; 3],
                grounded: false,
            },
            jump_held: false,
        }
    }
    fn input(direction: [f64; 2], jump: bool) -> MovementInput {
        MovementInput { direction, jump }
    }
    #[test]
    fn acceleration_diagonal_limit_analog_and_braking() {
        let w = world();
        let mut s = state();
        let first = w
            .control_tick(&s, input([1.0, 1.0], false), 0.05, options(), physics())
            .unwrap();
        assert!(
            (first.state.body.velocity[0].hypot(first.state.body.velocity[1]) - 1.0).abs() < 1e-9
        );
        for _ in 0..20 {
            s = w
                .control_tick(&s, input([1.0, 1.0], false), 0.05, options(), physics())
                .unwrap()
                .state;
        }
        assert!((s.body.velocity[0].hypot(s.body.velocity[1]) - 10.0).abs() < 1e-9);
        for _ in 0..5 {
            s = w
                .control_tick(&s, input([0.0; 2], false), 0.05, options(), physics())
                .unwrap()
                .state;
        }
        assert_eq!(s.body.velocity, [0.0; 3]);
        let mut analog = state();
        for _ in 0..10 {
            analog = w
                .control_tick(
                    &analog,
                    input([0.5, 0.0], false),
                    0.05,
                    options(),
                    physics(),
                )
                .unwrap()
                .state;
        }
        assert_eq!(analog.body.velocity[0], 5.0);
    }
    #[test]
    fn jump_edge_requires_support_and_held_jump_does_not_repeat_on_landing() {
        let w = world();
        let mut s = state();
        let first = w
            .control_tick(&s, input([0.0; 2], true), 0.02, options(), physics())
            .unwrap();
        assert!(first.jump_started && !first.state.body.grounded);
        s = first.state;
        for _ in 0..60 {
            let f = w
                .control_tick(&s, input([0.0; 2], true), 0.02, options(), physics())
                .unwrap();
            assert!(!f.jump_started);
            s = f.state;
        }
        assert!(s.body.grounded);
        s = w
            .control_tick(&s, input([0.0; 2], false), 0.02, options(), physics())
            .unwrap()
            .state;
        assert!(
            w.control_tick(&s, input([0.0; 2], true), 0.02, options(), physics())
                .unwrap()
                .jump_started
        );
        let mut airborne = state();
        airborne.body.position[2] = 20.0;
        airborne.body.grounded = true;
        assert!(
            !w.control_tick(&airborne, input([0.0; 2], true), 0.02, options(), physics())
                .unwrap()
                .jump_started
        );
    }
    #[test]
    fn air_control_inertia_and_failed_input_preserve_original_state() {
        let mut w = world();
        let mut s = state();
        s.body.position[2] = 50.0;
        let f = w
            .control_tick(&s, input([1.0, 0.0], false), 0.05, options(), physics())
            .unwrap();
        assert_eq!(f.state.body.velocity[0], 0.25);
        let next = w
            .control_tick(&f.state, input([0.0; 2], false), 0.05, options(), physics())
            .unwrap();
        assert_eq!(next.state.body.velocity[0], 0.25);
        assert!(w
            .control_tick(&s, input([f64::NAN, 0.0], true), 0.05, options(), physics())
            .is_err());
        w.actors.push(BodyActor {
            name: "unknown".into(),
            shape: BodyShape::Unsupported("missing".into()),
        });
        assert!(w
            .control_tick(&s, input([1.0, 0.0], true), 0.05, options(), physics())
            .is_err());
        assert_eq!(s.body.velocity, [0.0; 3]);
        assert!(!s.jump_held);
    }
    #[test]
    fn view_modes_reach_weighted_target_speeds_through_body_physics() {
        let w = world();
        let ratios = MovementRatios {
            walk: 0.5,
            back: 0.8,
            side: 0.95,
        };
        for (forward, strafe, walking, expected) in [
            (1.0, 0.0, false, 10.0),
            (-1.0, 0.0, false, 8.0),
            (0.0, 1.0, false, 9.5),
            (1.0, 0.0, true, 5.0),
        ] {
            let view = ViewInput {
                forward,
                strafe,
                yaw: 16384,
                walking,
                jump: false,
            };
            let input = view.world_input(ratios).unwrap();
            let mut s = state();
            for _ in 0..40 {
                s = w
                    .control_tick(&s, input, 0.05, options(), physics())
                    .unwrap()
                    .state;
            }
            assert!((s.body.velocity[0].hypot(s.body.velocity[1]) - expected).abs() < 1e-9);
            if forward > 0.0 {
                assert!(s.body.position[1] > 0.0);
            }
            if forward < 0.0 {
                assert!(s.body.position[1] < 0.0);
            }
            if strafe > 0.0 {
                assert!(s.body.position[0] < 0.0);
            }
        }
    }
    #[test]
    fn view_axes_wrapping_quantization_and_directional_modes() {
        let ratios = MovementRatios {
            walk: 0.5,
            back: 0.8,
            side: 0.95,
        };
        let view = ViewInput {
            forward: 1.0,
            strafe: 0.0,
            yaw: 16384,
            walking: false,
            jump: true,
        };
        let result = view.world_input(ratios).unwrap();
        assert!(
            result.direction[0].abs() < 1e-12
                && (result.direction[1] - 1.0).abs() < 1e-12
                && result.jump
        );
        assert_eq!(
            view.world_input(ratios).unwrap().direction,
            ViewInput { yaw: 81920, ..view }
                .world_input(ratios)
                .unwrap()
                .direction
        );
        assert_eq!(
            view.world_input(ratios).unwrap().direction,
            ViewInput { yaw: 16387, ..view }
                .world_input(ratios)
                .unwrap()
                .direction
        );
        let back = ViewInput {
            forward: -1.0,
            yaw: 0,
            walking: true,
            ..view
        }
        .world_input(ratios)
        .unwrap();
        assert!((back.direction[0] + 0.4).abs() < 1e-12);
        let side = ViewInput {
            forward: 0.0,
            strafe: 1.0,
            yaw: 0,
            ..view
        }
        .world_input(ratios)
        .unwrap();
        assert!((side.direction[1] - 0.95).abs() < 1e-12);
        let diagonal = ViewInput {
            strafe: 1.0,
            yaw: 0,
            ..view
        }
        .world_input(ratios)
        .unwrap();
        assert!(diagonal.direction[0].hypot(diagonal.direction[1]) <= 1.0);
        assert!(ViewInput {
            forward: f64::NAN,
            ..view
        }
        .world_input(ratios)
        .is_err());
    }
}
