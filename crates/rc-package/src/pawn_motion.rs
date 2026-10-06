//! Own state/input routing between reconstructed PC arithmetic and static collision adapters.
use crate::{
    controller::{ControlledBody, MovementRatios, ViewInput},
    falling::FreeFallOptions,
    falling_collision::FallingCollisionFrame,
    movement_profile::MovementProfile,
    pawn_jump::{jump_velocity, JumpContext, JumpPhysics},
    pawn_velocity::{SpeedFactors, WalkingDiagnosticFrame, WalkingDiagnosticOptions},
    volumes::{VolumeSelection, VolumeWorld},
    world_collision::StaticBodyWorld,
};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum MotionMode {
    Walking,
    Falling,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub struct PawnRuntimeOptions {
    pub maximum_desired_speed: f64,
    pub weapon_modifier: f64,
    pub crouch_ratio: f64,
    pub wounded_ratio: f64,
    pub crouched: bool,
    pub wounded: bool,
    pub wants_to_crouch: bool,
    /// Runtime JumpZ; the profile holds the resolved class default.
    pub current_jump_z: f64,
}
#[derive(Debug, Serialize)]
#[serde(tag = "mode", content = "frame")]
pub enum PawnMotion {
    Walking(WalkingDiagnosticFrame),
    Falling(FallingCollisionFrame),
}
#[derive(Debug, Serialize)]
pub struct PawnDiagnosticFrame {
    pub state: ControlledBody,
    pub volume: VolumeSelection,
    pub mode: MotionMode,
    pub next_mode: MotionMode,
    pub jump_started: bool,
    pub motion: PawnMotion,
}
impl VolumeWorld {
    /// Select volume and real floor support at tick start; latch jump input only on success.
    /// Own jump edge policy with extracted DoJump conditions; no script events or within-tick handoff.
    pub fn pawn_diagnostic_tick(
        &self,
        world: &StaticBodyWorld,
        state: &ControlledBody,
        view: ViewInput,
        dt: f64,
        profile: &MovementProfile,
        runtime: PawnRuntimeOptions,
    ) -> Result<PawnDiagnosticFrame, String> {
        if runtime.crouched {
            return Err("unsupported crouched body shape in own-edge diagnostic".into());
        }
        self.pawn_shape_tick(world, state, view, dt, profile, runtime)
    }
    /// Internal shared solver; caller must provide validated current body dimensions.
    pub(crate) fn pawn_shape_tick(
        &self,
        world: &StaticBodyWorld,
        state: &ControlledBody,
        view: ViewInput,
        dt: f64,
        profile: &MovementProfile,
        runtime: PawnRuntimeOptions,
    ) -> Result<PawnDiagnosticFrame, String> {
        if !dt.is_finite()
            || dt <= 0.0
            || dt > 0.05
            || !runtime.maximum_desired_speed.is_finite()
            || !(0.0..=1e6).contains(&runtime.maximum_desired_speed)
            || !profile.controller.jump_speed.is_finite()
            || !(0.0..=1e6).contains(&profile.controller.jump_speed)
            || profile.controller.jump_speed == 0.0
            || !runtime.current_jump_z.is_finite()
            || !(0.0..=1e6).contains(&runtime.current_jump_z)
            || runtime.current_jump_z == 0.0
        {
            return Err(
                "invalid pawn diagnostic options or unsupported crouched body shape".into(),
            );
        }
        let factors = SpeedFactors {
            ratios: profile.ratios,
            crouch_ratio: runtime.crouch_ratio,
            wounded_ratio: runtime.wounded_ratio,
            walking: view.walking,
            crouched: runtime.crouched,
            wounded: runtime.wounded,
            weapon_modifier: runtime.weapon_modifier,
        };
        factors.local_factor([0.0; 3])?;
        let volume = self.select(state.body.position.map(|v| v as f32))?;
        if !volume.complete {
            return Err("pawn diagnostic volume selection incomplete".into());
        }
        let physics = volume.settings.apply(profile.physics)?;
        let friction = f64::from(volume.settings.ground_friction);
        if !friction.is_finite() || !(0.0..=1e6).contains(&friction) {
            return Err("invalid selected GroundFriction".into());
        }
        let direction = view
            .world_input(MovementRatios {
                walk: 1.0,
                back: 1.0,
                side: 1.0,
            })?
            .direction;
        let acceleration = direction.map(|v| v * profile.controller.acceleration);
        let supported = state.body.velocity[2] <= 0.0
            && world
                .floor_contact(
                    state.body.position,
                    physics.extent,
                    physics.support_distance,
                    physics.minimum_up,
                )?
                .is_some();
        let jump = if view.jump && !state.jump_held {
            jump_velocity(
                state.body.velocity,
                JumpContext {
                    physics: if supported {
                        JumpPhysics::Walking
                    } else {
                        JumpPhysics::Other
                    },
                    crouched: runtime.crouched,
                    wants_to_crouch: runtime.wants_to_crouch,
                    walking: view.walking,
                    current_jump_z: runtime.current_jump_z,
                    default_jump_z: profile.controller.jump_speed,
                    floor: [0.0, 0.0, 1.0], // Unused in the Walking branch.
                    base: None,             // Static diagnostic: no moving base actor is modeled.
                },
            )?
        } else {
            None
        };
        let jump_started = jump.is_some();
        let mut body = state.body;
        if let Some(velocity) = jump {
            body.velocity = velocity;
            body.grounded = false;
        }
        let (mode, motion) = if supported && !jump_started {
            let frame = world.walking_diagnostic_tick(
                &body,
                acceleration,
                dt,
                WalkingDiagnosticOptions {
                    base_acceleration: profile.controller.acceleration,
                    base_speed: profile.controller.speed,
                    maximum_desired_speed: runtime.maximum_desired_speed,
                    ground_friction: friction,
                    yaw: view.yaw,
                    factors,
                },
                physics,
            )?;
            (MotionMode::Walking, PawnMotion::Walking(frame))
        } else {
            let frame = world.falling_diagnostic_tick(
                &body,
                acceleration,
                dt,
                FreeFallOptions {
                    acceleration_rate: profile.controller.acceleration,
                    air_control: profile.controller.air_control,
                    ground_speed: profile.controller.speed,
                    terminal_speed: physics.terminal_speed,
                    gravity: [0.0, 0.0, -physics.gravity],
                    zone_velocity: [0.0; 3],
                },
                physics.into(),
            )?;
            (MotionMode::Falling, PawnMotion::Falling(frame))
        };
        let body = match &motion {
            PawnMotion::Walking(f) => f.motion.body,
            PawnMotion::Falling(f) => f.motion.body,
        };
        let next_mode = if body.grounded && body.velocity[2] <= 0.0 {
            MotionMode::Walking
        } else {
            MotionMode::Falling
        };
        Ok(PawnDiagnosticFrame {
            state: ControlledBody {
                body,
                jump_held: view.jump,
            },
            volume,
            mode,
            next_mode,
            jump_started,
            motion,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        controller::ControllerOptions,
        hull::{Hull, HullSet},
        physics::{BodyState, PhysicsOptions},
        volumes::VolumeSettings,
    };
    use std::{collections::BTreeMap, sync::Arc};
    fn fixture() -> (
        VolumeWorld,
        StaticBodyWorld,
        MovementProfile,
        PawnRuntimeOptions,
    ) {
        (
            VolumeWorld {
                default: VolumeSettings {
                    gravity: [0.0, 0.0, -100.0],
                    terminal_speed: 1000.0,
                    ground_friction: 8.0,
                    water: false,
                    zone_velocity: [0.0; 3],
                },
                volumes: vec![],
            },
            StaticBodyWorld {
                world: Ok(Arc::new(HullSet {
                    hulls: vec![Hull {
                        offset: 0,
                        planes: vec![],
                        bounds: [[-100.0, -100.0, -1.0], [100.0, 100.0, 0.0]],
                    }],
                    missing_solid_leaves: 0,
                    empty_root_solid: false,
                })),
                actors: vec![],
            },
            MovementProfile {
                controller: ControllerOptions {
                    speed: 10.0,
                    acceleration: 20.0,
                    braking: 600.0,
                    air_control: 0.35,
                    jump_speed: 30.0,
                },
                ratios: MovementRatios {
                    walk: 0.5,
                    back: 0.8,
                    side: 0.95,
                },
                physics: PhysicsOptions {
                    extent: [1.0; 3],
                    gravity: 100.0,
                    terminal_speed: 1000.0,
                    skin: 0.1,
                    support_distance: 0.5,
                    minimum_up: 0.7,
                    max_iterations: 8,
                    step_height: 0.0,
                },
                properties: BTreeMap::new(),
            },
            PawnRuntimeOptions {
                maximum_desired_speed: 10.0,
                weapon_modifier: 1.0,
                crouch_ratio: 0.5,
                wounded_ratio: 0.833,
                crouched: false,
                wounded: false,
                wants_to_crouch: false,
                current_jump_z: 30.0,
            },
        )
    }
    fn state(z: f64) -> ControlledBody {
        ControlledBody {
            body: BodyState {
                position: [0.0, 0.0, z],
                velocity: [0.0; 3],
                grounded: false,
            },
            jump_held: false,
        }
    }
    fn input(jump: bool) -> ViewInput {
        ViewInput {
            forward: 0.0,
            strafe: 0.0,
            yaw: 0,
            walking: false,
            jump,
        }
    }
    fn script_state(z: f64) -> crate::script_motion::ScriptBody {
        crate::script_motion::ScriptBody {
            body: state(z).body,
            pressed_jump: false,
            wants_to_crouch: false,
        }
    }
    fn script_input(event: bool) -> crate::script_motion::ScriptMotionInput {
        crate::script_motion::ScriptMotionInput {
            view: input(false),
            jump_event: event,
            cannot_jump_now: false,
            duck: 0,
            can_crouch: true,
        }
    }
    fn crouch_shape() -> crate::crouch::CrouchProfile {
        crate::crouch::CrouchProfile {
            standing: [1.0; 3],
            crouching: [1.0, 1.0, 0.5],
            properties: BTreeMap::new(),
        }
    }
    fn crouching_state() -> crate::crouch_motion::CrouchingScriptBody {
        crate::crouch_motion::CrouchingScriptBody {
            script: script_state(1.0),
            crouched: false,
            try_to_uncrouch: false,
            uncrouch_time: 0.0,
        }
    }
    #[test]
    fn static_wall_contact_arms_crouch_even_when_end_velocity_is_zero() {
        use crate::{
            ai_contact::{AiContactOptions, AiControllerSnapshot, WallEventSnapshot},
            crouch_motion::{CrouchMotionOptions, CrouchPhysicsInput},
            hit_wall::HitWallRecoveryDecision,
        };
        let (v, mut w, p, r) = fixture();
        let shape = crouch_shape();
        w.world = Ok(Arc::new(HullSet {
            hulls: vec![
                Hull {
                    offset: 0,
                    planes: vec![],
                    bounds: [[-100.0, -100.0, -1.0], [100.0, 100.0, 0.0]],
                },
                Hull {
                    offset: 1,
                    planes: vec![],
                    bounds: [[2.0, -100.0, 1.2], [100.0, 100.0, 3.0]],
                },
            ],
            missing_solid_leaves: 0,
            empty_root_solid: false,
        }));
        let controller = AiControllerSnapshot {
            controller_present: true,
            direct_hit_wall: false,
            human_controlled: false,
            destination: [20.0, 0.0, 1.1],
            minimum_hit_wall: 0.0,
            walls: BTreeMap::from([(
                "World.BSP".into(),
                WallEventSnapshot {
                    excluded_wall_class: false,
                    notify_handled: false,
                },
            )]),
        };
        let mut s = crouching_state();
        s.script.body.position = [0.8, 0.0, 1.1];
        s.script.body.velocity = [10.0, 0.0, 0.0];
        s.script.pressed_jump = true;
        let mut view = input(false);
        view.forward = 1.0;
        let f = v
            .ai_crouching_contact_tick(
                &w,
                &s,
                CrouchPhysicsInput {
                    view,
                    can_crouch: true,
                },
                0.05,
                &p,
                AiContactOptions {
                    motion: CrouchMotionOptions {
                        runtime: r,
                        shape: &shape,
                    },
                    controller: &controller,
                },
            )
            .unwrap();
        assert_eq!(f.wall_object.as_deref(), Some("World.BSP"));
        assert_eq!(f.incoming_velocity, Some([10.0, 0.0, 0.0]));
        assert_eq!(f.state.script.body.velocity, [0.0; 3]);
        assert_eq!(
            f.dispatch.unwrap().decision,
            HitWallRecoveryDecision::ArmedFirst
        );
        assert!(
            f.state.script.wants_to_crouch
                && f.state.try_to_uncrouch
                && f.state.script.pressed_jump
        );
        assert!(!f.state.crouched);
        let next = v
            .ai_crouching_contact_tick(
                &w,
                &f.state,
                CrouchPhysicsInput {
                    view,
                    can_crouch: true,
                },
                0.05,
                &p,
                AiContactOptions {
                    motion: CrouchMotionOptions {
                        runtime: r,
                        shape: &shape,
                    },
                    controller: &controller,
                },
            )
            .unwrap();
        assert!(
            next.state.crouched
                && next.state.script.body.position[0] > f.state.script.body.position[0]
        );
        assert!(next.state.script.pressed_jump);
        assert!(next.dispatch.is_none());
    }
    #[test]
    fn human_contact_is_dispatched_but_never_arms_ai_crouch() {
        use crate::{
            ai_contact::{AiContactOptions, AiControllerSnapshot, WallEventSnapshot},
            crouch_motion::{CrouchMotionOptions, CrouchPhysicsInput},
            hit_wall::{HitWallDecision, HitWallRecoveryDecision},
        };
        let (v, mut w, p, r) = fixture();
        let shape = crouch_shape();
        w.world = Ok(Arc::new(HullSet {
            hulls: vec![
                Hull {
                    offset: 0,
                    planes: vec![],
                    bounds: [[-100.0, -100.0, -1.0], [100.0, 100.0, 0.0]],
                },
                Hull {
                    offset: 1,
                    planes: vec![],
                    bounds: [[2.0, -100.0, 1.2], [100.0, 100.0, 3.0]],
                },
            ],
            missing_solid_leaves: 0,
            empty_root_solid: false,
        }));
        let controller = AiControllerSnapshot {
            controller_present: true,
            direct_hit_wall: false,
            human_controlled: true,
            destination: [20.0, 0.0, 1.1],
            minimum_hit_wall: 0.0,
            walls: BTreeMap::from([(
                "World.BSP".into(),
                WallEventSnapshot {
                    excluded_wall_class: false,
                    notify_handled: false,
                },
            )]),
        };
        let mut s = crouching_state();
        s.script.body.position = [0.8, 0.0, 1.1];
        s.script.body.velocity = [10.0, 0.0, 0.0];
        let mut view = input(false);
        view.forward = 1.0;
        let f = v
            .ai_crouching_contact_tick(
                &w,
                &s,
                CrouchPhysicsInput {
                    view,
                    can_crouch: true,
                },
                0.05,
                &p,
                AiContactOptions {
                    motion: CrouchMotionOptions {
                        runtime: r,
                        shape: &shape,
                    },
                    controller: &controller,
                },
            )
            .unwrap();
        let dispatch = f.dispatch.unwrap();
        assert_eq!(dispatch.decision, HitWallRecoveryDecision::Skipped);
        assert_eq!(dispatch.first.decision, HitWallDecision::HumanControlled);
        assert!(!f.state.try_to_uncrouch && !f.state.script.wants_to_crouch);
    }
    #[test]
    fn floor_only_and_airborne_ai_ticks_do_not_require_wall_metadata() {
        use crate::{
            ai_contact::{AiContactOptions, AiControllerSnapshot},
            crouch_motion::{CrouchMotionOptions, CrouchPhysicsInput},
        };
        let (v, w, p, r) = fixture();
        let shape = crouch_shape();
        let controller = AiControllerSnapshot {
            controller_present: true,
            direct_hit_wall: false,
            human_controlled: false,
            destination: [20.0, 0.0, 1.1],
            minimum_hit_wall: 0.0,
            walls: BTreeMap::new(),
        };
        for z in [1.1, 10.0] {
            let mut s = crouching_state();
            s.script.body.position[2] = z;
            let f = v
                .ai_crouching_contact_tick(
                    &w,
                    &s,
                    CrouchPhysicsInput {
                        view: input(false),
                        can_crouch: true,
                    },
                    0.05,
                    &p,
                    AiContactOptions {
                        motion: CrouchMotionOptions {
                            runtime: r,
                            shape: &shape,
                        },
                        controller: &controller,
                    },
                )
                .unwrap();
            assert!(f.dispatch.is_none() && f.wall_object.is_none());
            assert!(!f.state.script.wants_to_crouch);
        }
    }
    #[test]
    fn missing_contact_metadata_discards_completed_ai_motion_candidate() {
        use crate::{
            ai_contact::{AiContactOptions, AiControllerSnapshot},
            crouch_motion::{CrouchMotionOptions, CrouchPhysicsInput},
        };
        let (v, mut w, p, r) = fixture();
        let shape = crouch_shape();
        w.world = Ok(Arc::new(HullSet {
            hulls: vec![
                Hull {
                    offset: 0,
                    planes: vec![],
                    bounds: [[-100.0, -100.0, -1.0], [100.0, 100.0, 0.0]],
                },
                Hull {
                    offset: 1,
                    planes: vec![],
                    bounds: [[2.0, -100.0, 1.2], [100.0, 100.0, 3.0]],
                },
            ],
            missing_solid_leaves: 0,
            empty_root_solid: false,
        }));
        let controller = AiControllerSnapshot {
            controller_present: true,
            direct_hit_wall: false,
            human_controlled: false,
            destination: [20.0, 0.0, 1.1],
            minimum_hit_wall: 0.0,
            walls: BTreeMap::new(),
        };
        let mut s = crouching_state();
        s.script.body.position = [0.8, 0.0, 1.1];
        s.script.body.velocity = [10.0, 0.0, 0.0];
        let mut view = input(false);
        view.forward = 1.0;
        let error = v
            .ai_crouching_contact_tick(
                &w,
                &s,
                CrouchPhysicsInput {
                    view,
                    can_crouch: true,
                },
                0.05,
                &p,
                AiContactOptions {
                    motion: CrouchMotionOptions {
                        runtime: r,
                        shape: &shape,
                    },
                    controller: &controller,
                },
            )
            .unwrap_err();
        assert!(error.contains("missing AI wall event snapshot"));
        assert_eq!(s.script.body.position, [0.8, 0.0, 1.1]);
        assert_eq!(s.script.body.velocity, [10.0, 0.0, 0.0]);
        assert!(!s.try_to_uncrouch);
    }
    #[test]
    fn hit_wall_arming_runs_through_physics_without_player_duck_release() {
        use crate::{
            crouch_motion::{CrouchMotionOptions, CrouchPhysicsInput},
            hit_wall::{HitWallContext, HitWallDecision},
        };
        let (v, w, p, r) = fixture();
        let shape = crouch_shape();
        let mut s = crouching_state();
        s.script.body.position[2] = 1.1;
        s.script.body.velocity[0] = 1.0;
        s.script.pressed_jump = true;
        let context = HitWallContext {
            wall_present: true,
            excluded_wall_class: false,
            direct_hit_wall: false,
            controller_present: true,
            destination: [10.0, 0.0, 1.1],
            wall_normal: [-1.0, 0.0, 0.0],
            minimum_hit_wall: 0.0,
            notify_handled: false,
            physics: JumpPhysics::Walking,
            human_controlled: false,
            can_crouch: true,
        };
        let armed = w
            .hit_wall_auto_crouch_first_attempt(&s, context, &shape)
            .unwrap();
        assert_eq!(armed.decision, HitWallDecision::Armed);
        let tick = |state: &crate::crouch_motion::CrouchingScriptBody| {
            v.crouching_physics_tick(
                &w,
                state,
                CrouchPhysicsInput {
                    view: input(false),
                    can_crouch: true,
                },
                0.05,
                &p,
                CrouchMotionOptions {
                    runtime: r,
                    shape: &shape,
                },
            )
            .unwrap()
        };
        let entered = tick(&armed.state);
        assert!(entered.state.crouched && entered.state.script.wants_to_crouch);
        assert_eq!(entered.state.uncrouch_time, 0.5);
        assert!(entered.state.script.pressed_jump && !entered.movement.jump_started);
        s = entered.state;
        let mut countdown_ticks = 0;
        while s.try_to_uncrouch {
            let f = tick(&s);
            countdown_ticks += 1;
            assert!(countdown_ticks <= 11);
            assert!(f.state.script.pressed_jump && !f.movement.jump_started);
            s = f.state;
        }
        assert_eq!(countdown_ticks, 10);
        assert!(!s.crouched && !s.script.wants_to_crouch);
        assert!(s.uncrouch_time <= 0.0 && s.script.body.grounded);
        assert_eq!(s.script.body.velocity, [0.0; 3]);
    }
    #[test]
    fn physics_only_timer_expiry_under_ceiling_retries_without_new_input_phase() {
        use crate::crouch_motion::{CrouchMotionOptions, CrouchPhysicsInput};
        let (v, mut w, p, r) = fixture();
        let shape = crouch_shape();
        let clear = w.world.clone();
        w.world = Ok(Arc::new(HullSet {
            hulls: vec![
                Hull {
                    offset: 0,
                    planes: vec![],
                    bounds: [[-100.0, -100.0, -1.0], [100.0, 100.0, 0.0]],
                },
                Hull {
                    offset: 1,
                    planes: vec![],
                    bounds: [[3.0, -100.0, 1.2], [100.0, 100.0, 3.0]],
                },
            ],
            missing_solid_leaves: 0,
            empty_root_solid: false,
        }));
        let mut s = crouching_state();
        s.script.body.position[2] = 1.1;
        s.script.body.velocity[0] = 5.0;
        let armed = w
            .can_crouch_walk_diagnostic(&s, s.script.body.position, [1.0, 0.0, 1.1], true, &shape)
            .unwrap();
        assert_eq!(
            armed.decision,
            crate::auto_crouch::AutoCrouchDecision::Armed
        );
        s = armed.state;
        let mut view = input(false);
        view.forward = 1.0;
        for _ in 0..11 {
            let f = v
                .crouching_physics_tick(
                    &w,
                    &s,
                    CrouchPhysicsInput {
                        view,
                        can_crouch: true,
                    },
                    0.05,
                    &p,
                    CrouchMotionOptions {
                        runtime: r,
                        shape: &shape,
                    },
                )
                .unwrap();
            s = f.state;
        }
        assert!(s.crouched && !s.try_to_uncrouch && !s.script.wants_to_crouch);
        let blocked = v
            .crouching_physics_tick(
                &w,
                &s,
                CrouchPhysicsInput {
                    view: input(false),
                    can_crouch: true,
                },
                0.05,
                &p,
                CrouchMotionOptions {
                    runtime: r,
                    shape: &shape,
                },
            )
            .unwrap();
        assert_eq!(
            blocked.after_movement.unwrap().change,
            crate::crouch::CrouchChange::Blocked
        );
        w.world = clear;
        let retry = v
            .crouching_physics_tick(
                &w,
                &blocked.state,
                CrouchPhysicsInput {
                    view: input(false),
                    can_crouch: true,
                },
                0.05,
                &p,
                CrouchMotionOptions {
                    runtime: r,
                    shape: &shape,
                },
            )
            .unwrap();
        assert!(!retry.state.crouched && !retry.state.script.wants_to_crouch);
    }
    #[test]
    fn physics_only_crouch_rejects_jump_and_bad_geometry_atomically() {
        use crate::crouch_motion::{CrouchMotionOptions, CrouchPhysicsInput};
        let (v, mut w, p, r) = fixture();
        let shape = crouch_shape();
        let mut s = crouching_state();
        s.script.wants_to_crouch = true;
        s.script.pressed_jump = true;
        s.try_to_uncrouch = true;
        s.uncrouch_time = 0.5;
        assert!(v
            .crouching_physics_tick(
                &w,
                &s,
                CrouchPhysicsInput {
                    view: input(true),
                    can_crouch: true
                },
                0.05,
                &p,
                CrouchMotionOptions {
                    runtime: r,
                    shape: &shape
                }
            )
            .is_err());
        w.world = Err("missing geometry".into());
        assert!(v
            .crouching_physics_tick(
                &w,
                &s,
                CrouchPhysicsInput {
                    view: input(false),
                    can_crouch: true
                },
                0.05,
                &p,
                CrouchMotionOptions {
                    runtime: r,
                    shape: &shape
                }
            )
            .is_err());
        assert!(s.script.pressed_jump && s.script.wants_to_crouch && s.try_to_uncrouch);
        assert_eq!(s.uncrouch_time, 0.5);
        assert!(!s.crouched);
    }
    #[test]
    fn uncrouch_timer_expires_after_f32_countdown_and_stands_after_motion() {
        use crate::crouch_motion::CrouchMotionOptions;
        let (v, w, p, r) = fixture();
        let shape = crouch_shape();
        let mut s = crouching_state();
        s.try_to_uncrouch = true;
        s.uncrouch_time = 0.075;
        let mut i = script_input(false);
        i.duck = 1;
        i.view.forward = 1.0;
        let tick = |s: &crate::crouch_motion::CrouchingScriptBody| {
            v.crouching_script_tick(
                &w,
                s,
                i,
                0.05,
                &p,
                CrouchMotionOptions {
                    runtime: r,
                    shape: &shape,
                },
            )
            .unwrap()
        };
        let entered = tick(&s);
        assert!(entered.state.crouched && entered.state.try_to_uncrouch);
        assert_eq!(entered.state.uncrouch_time, 0.075);
        let counting = tick(&entered.state);
        assert_eq!(counting.state.uncrouch_time, 0.075_f32 - 0.05_f32);
        assert!(counting.state.script.wants_to_crouch && counting.state.try_to_uncrouch);
        let expired = tick(&counting.state);
        assert!(
            expired
                .input_phase
                .pawn
                .as_ref()
                .unwrap()
                .jump
                .wants_to_crouch
        );
        assert!(!expired.state.script.wants_to_crouch && !expired.state.try_to_uncrouch);
        assert_eq!(
            expired.state.uncrouch_time,
            (0.075_f32 - 0.05_f32) - 0.05_f32
        );
        assert_eq!(expired.movement.state.body.position[2], 0.5);
        assert_eq!(expired.movement.state.body.velocity[0], 1.5);
        assert!(!expired.state.crouched);
        // Held duck input requests crouch again on the next script phase.
        let reentered = tick(&expired.state);
        assert!(reentered.state.crouched && reentered.state.script.wants_to_crouch);
        assert!(!reentered.state.try_to_uncrouch);
        assert_eq!(reentered.state.uncrouch_time, expired.state.uncrouch_time);
    }
    #[test]
    fn uncrouch_timer_gates_preserve_inactive_snapshot() {
        use crate::crouch_motion::CrouchMotionOptions;
        let (v, w, p, r) = fixture();
        let shape = crouch_shape();
        for case in 0..4 {
            let mut s = crouching_state();
            s.crouched = true;
            s.script.body.position[2] = 0.5;
            s.script.wants_to_crouch = true;
            s.try_to_uncrouch = case != 0;
            s.uncrouch_time = 0.05;
            let mut i = script_input(false);
            i.duck = 1;
            if case == 1 {
                i.can_crouch = false;
            }
            if case == 2 {
                i.duck = 0;
            }
            if case == 3 {
                s.script.body.position[2] = 10.0;
            }
            let f = v
                .crouching_script_tick(
                    &w,
                    &s,
                    i,
                    0.05,
                    &p,
                    CrouchMotionOptions {
                        runtime: r,
                        shape: &shape,
                    },
                )
                .unwrap();
            assert_eq!(f.state.uncrouch_time, s.uncrouch_time);
            assert_eq!(f.state.try_to_uncrouch, s.try_to_uncrouch);
        }
    }
    #[test]
    fn expired_uncrouch_timer_clears_flags_even_when_standing_is_blocked() {
        use crate::crouch_motion::CrouchMotionOptions;
        let (v, mut w, p, r) = fixture();
        let shape = crouch_shape();
        let clear = w.world.clone();
        w.world = Ok(Arc::new(HullSet {
            hulls: vec![
                Hull {
                    offset: 0,
                    planes: vec![],
                    bounds: [[-100.0, -100.0, -1.0], [100.0, 100.0, 0.0]],
                },
                Hull {
                    offset: 1,
                    planes: vec![],
                    bounds: [[-100.0, -100.0, 1.2], [100.0, 100.0, 3.0]],
                },
            ],
            missing_solid_leaves: 0,
            empty_root_solid: false,
        }));
        let mut s = crouching_state();
        s.crouched = true;
        s.script.body.position[2] = 0.5;
        s.try_to_uncrouch = true;
        s.uncrouch_time = 0.05;
        let mut i = script_input(false);
        i.duck = 1;
        let f = v
            .crouching_script_tick(
                &w,
                &s,
                i,
                0.05,
                &p,
                CrouchMotionOptions {
                    runtime: r,
                    shape: &shape,
                },
            )
            .unwrap();
        assert_eq!(f.state.uncrouch_time, 0.0);
        assert!(f.state.crouched);
        assert!(!f.state.script.wants_to_crouch && !f.state.try_to_uncrouch);
        assert_eq!(
            f.after_movement.as_ref().unwrap().change,
            crate::crouch::CrouchChange::Blocked
        );
        w.world = clear;
        let retry = v
            .crouching_script_tick(
                &w,
                &f.state,
                script_input(false),
                0.05,
                &p,
                CrouchMotionOptions {
                    runtime: r,
                    shape: &shape,
                },
            )
            .unwrap();
        assert!(!retry.state.crouched);
        assert_eq!(retry.state.uncrouch_time, 0.0);
    }
    #[test]
    fn crouch_movement_caps_speed_and_release_stands_after_integration() {
        use crate::crouch_motion::CrouchMotionOptions;
        let (v, w, p, r) = fixture();
        let shape = crouch_shape();
        let mut s = crouching_state();
        let mut i = script_input(false);
        i.duck = 1;
        i.view.forward = 1.0;
        for _ in 0..20 {
            let f = v
                .crouching_script_tick(
                    &w,
                    &s,
                    i,
                    0.05,
                    &p,
                    CrouchMotionOptions {
                        runtime: r,
                        shape: &shape,
                    },
                )
                .unwrap();
            assert!(f.state.crouched);
            assert_eq!(f.state.script.body.position[2], 0.5);
            s = f.state;
        }
        assert_eq!(s.script.body.velocity[0], 5.0);
        i.duck = 0;
        let released = v
            .crouching_script_tick(
                &w,
                &s,
                i,
                0.05,
                &p,
                CrouchMotionOptions {
                    runtime: r,
                    shape: &shape,
                },
            )
            .unwrap();
        assert_eq!(released.movement.state.body.position[2], 0.5);
        assert_eq!(released.movement.state.body.velocity[0], 5.0);
        assert!(released.after_movement.is_some());
        assert!(!released.state.crouched);
        assert_eq!(released.state.script.body.position[2], 1.0);
        let next = v
            .crouching_script_tick(
                &w,
                &released.state,
                i,
                0.05,
                &p,
                CrouchMotionOptions {
                    runtime: r,
                    shape: &shape,
                },
            )
            .unwrap();
        assert_eq!(next.state.script.body.velocity[0], 6.0);
    }
    #[test]
    fn blocked_standing_preserves_crouch_and_retries_after_ceiling_is_removed() {
        use crate::crouch_motion::CrouchMotionOptions;
        let (v, mut w, p, r) = fixture();
        let shape = crouch_shape();
        let clear = w.world.clone();
        w.world = Ok(Arc::new(HullSet {
            hulls: vec![
                Hull {
                    offset: 0,
                    planes: vec![],
                    bounds: [[-100.0, -100.0, -1.0], [100.0, 100.0, 0.0]],
                },
                Hull {
                    offset: 1,
                    planes: vec![],
                    bounds: [[-100.0, -100.0, 1.2], [100.0, 100.0, 3.0]],
                },
            ],
            missing_solid_leaves: 0,
            empty_root_solid: false,
        }));
        let mut s = crouching_state();
        s.crouched = true;
        s.script.body.position[2] = 0.5;
        s.script.wants_to_crouch = true;
        let f = v
            .crouching_script_tick(
                &w,
                &s,
                script_input(true),
                0.05,
                &p,
                CrouchMotionOptions {
                    runtime: r,
                    shape: &shape,
                },
            )
            .unwrap();
        assert!(!f.movement.jump_started && f.state.crouched && !f.state.script.pressed_jump);
        assert!(!f.state.script.wants_to_crouch);
        assert_eq!(
            f.after_movement.as_ref().unwrap().change,
            crate::crouch::CrouchChange::Blocked
        );
        w.world = clear;
        let retry = v
            .crouching_script_tick(
                &w,
                &f.state,
                script_input(false),
                0.05,
                &p,
                CrouchMotionOptions {
                    runtime: r,
                    shape: &shape,
                },
            )
            .unwrap();
        assert!(!retry.state.crouched && !retry.movement.jump_started);
        let jump = v
            .crouching_script_tick(
                &w,
                &retry.state,
                script_input(true),
                0.05,
                &p,
                CrouchMotionOptions {
                    runtime: r,
                    shape: &shape,
                },
            )
            .unwrap();
        assert!(jump.movement.jump_started);
    }
    #[test]
    fn airborne_crouch_attempts_standing_before_falling_motion() {
        use crate::crouch_motion::CrouchMotionOptions;
        let (v, w, p, r) = fixture();
        let shape = crouch_shape();
        let mut s = crouching_state();
        s.crouched = true;
        s.script.body.position[2] = 10.0;
        s.script.wants_to_crouch = true;
        let f = v
            .crouching_script_tick(
                &w,
                &s,
                script_input(false),
                0.05,
                &p,
                CrouchMotionOptions {
                    runtime: r,
                    shape: &shape,
                },
            )
            .unwrap();
        assert_eq!(
            f.before_movement.as_ref().unwrap().state.body.position[2],
            10.5
        );
        assert!(!f.state.crouched);
        assert_eq!(f.movement.mode, MotionMode::Falling);
        assert_eq!(f.state.script.body.position[2], 10.25);
        assert!(f.state.script.wants_to_crouch); // Falling ProcessMove doesn't clear request.
    }
    #[test]
    fn crouch_dispatch_errors_do_not_commit_shape_or_script_flags() {
        use crate::crouch_motion::CrouchMotionOptions;
        let (mut v, mut w, p, r) = fixture();
        let shape = crouch_shape();
        let mut s = crouching_state();
        s.script.pressed_jump = true;
        s.uncrouch_time = f32::NAN;
        assert!(v
            .crouching_script_tick(
                &w,
                &s,
                script_input(false),
                0.05,
                &p,
                CrouchMotionOptions {
                    runtime: r,
                    shape: &shape
                }
            )
            .is_err());
        s.uncrouch_time = 0.0;
        v.default.water = true;
        assert!(v
            .crouching_script_tick(
                &w,
                &s,
                script_input(false),
                0.05,
                &p,
                CrouchMotionOptions {
                    runtime: r,
                    shape: &shape
                }
            )
            .is_err());
        v.default.water = false;
        w.world = Err("missing geometry".into());
        assert!(v
            .crouching_script_tick(
                &w,
                &s,
                script_input(false),
                0.05,
                &p,
                CrouchMotionOptions {
                    runtime: r,
                    shape: &shape
                }
            )
            .is_err());
        assert!(!s.crouched && s.script.pressed_jump);
        assert_eq!(s.script.body.position[2], 1.0);
    }
    #[test]
    fn script_saved_event_retries_through_collision_without_new_button_edge() {
        let (v, w, p, r) = fixture();
        let f = v
            .script_motion_tick(
                &w,
                &script_state(1.0),
                crate::script_motion::ScriptMotionInput {
                    cannot_jump_now: true,
                    ..script_input(true)
                },
                0.05,
                &p,
                r,
            )
            .unwrap();
        assert!(!f.jump_started && f.state.pressed_jump);
        assert_eq!(f.mode, MotionMode::Walking);
        let retry = v
            .script_motion_tick(&w, &f.state, script_input(false), 0.05, &p, r)
            .unwrap();
        assert!(retry.jump_started && !retry.state.pressed_jump);
        assert_eq!(retry.mode, MotionMode::Falling);
        assert_eq!(retry.state.body.velocity[2], 25.0);
    }
    #[test]
    fn script_air_event_is_consumed_and_landing_requires_new_event() {
        let (v, w, p, r) = fixture();
        let mut s = script_state(2.0);
        s.body.velocity[2] = -40.0;
        let f = v
            .script_motion_tick(&w, &s, script_input(true), 0.05, &p, r)
            .unwrap();
        assert!(!f.jump_started && !f.state.pressed_jump);
        assert_eq!(f.next_mode, MotionMode::Walking);
        let idle = v
            .script_motion_tick(&w, &f.state, script_input(false), 0.05, &p, r)
            .unwrap();
        assert!(!idle.jump_started && idle.state.body.grounded);
        let fresh = v
            .script_motion_tick(&w, &idle.state, script_input(true), 0.05, &p, r)
            .unwrap();
        assert!(fresh.jump_started);
    }
    #[test]
    fn script_crouch_order_is_preserved_and_body_transition_is_explicit() {
        let (v, w, p, r) = fixture();
        let mut s = script_state(1.0);
        s.wants_to_crouch = true;
        let released = v
            .script_motion_tick(&w, &s, script_input(true), 0.05, &p, r)
            .unwrap();
        assert!(!released.jump_started && !released.state.wants_to_crouch);
        assert!(!released.state.pressed_jump);
        let simultaneous = v
            .script_motion_tick(
                &w,
                &released.state,
                crate::script_motion::ScriptMotionInput {
                    duck: 1,
                    ..script_input(true)
                },
                0.05,
                &p,
                r,
            )
            .unwrap();
        assert!(simultaneous.jump_started && !simultaneous.state.wants_to_crouch);
        assert_eq!(simultaneous.input_phase.crouch_request, None);
        assert!(v
            .script_motion_tick(
                &w,
                &script_state(1.0),
                crate::script_motion::ScriptMotionInput {
                    duck: 1,
                    ..script_input(false)
                },
                0.05,
                &p,
                r
            )
            .unwrap_err()
            .contains("body-size"));
    }
    #[test]
    fn script_geometry_volume_and_ambiguous_input_fail_without_consuming_flags() {
        let (mut v, mut w, p, r) = fixture();
        let mut s = script_state(1.0);
        s.pressed_jump = true;
        let mut ambiguous = script_input(false);
        ambiguous.view.jump = true;
        assert!(v
            .script_motion_tick(&w, &s, ambiguous, 0.05, &p, r)
            .is_err());
        v.default.water = true;
        assert!(v
            .script_motion_tick(&w, &s, script_input(false), 0.05, &p, r)
            .is_err());
        v.default.water = false;
        w.world = Err("missing world".into());
        assert!(v
            .script_motion_tick(&w, &s, script_input(false), 0.05, &p, r)
            .is_err());
        assert!(s.pressed_jump && !s.wants_to_crouch);
        assert_eq!(s.body.position, [0.0, 0.0, 1.0]);
    }
    #[test]
    fn walking_jump_uses_profile_default_and_running_uses_runtime() {
        let (v, w, p, mut r) = fixture();
        r.current_jump_z = 60.0;
        for (walking, expected) in [(false, 55.0), (true, 25.0)] {
            let mut view = input(true);
            view.walking = walking;
            let f = v
                .pawn_diagnostic_tick(&w, &state(1.0), view, 0.05, &p, r)
                .unwrap();
            assert!(f.jump_started);
            assert_eq!(f.mode, MotionMode::Falling);
            assert_eq!(f.state.body.velocity[2], expected);
        }
    }
    #[test]
    fn crouch_request_denies_jump_and_consumes_diagnostic_edge() {
        let (v, w, p, mut r) = fixture();
        r.wants_to_crouch = true;
        let f = v
            .pawn_diagnostic_tick(&w, &state(1.0), input(true), 0.05, &p, r)
            .unwrap();
        assert!(!f.jump_started);
        assert_eq!(f.mode, MotionMode::Walking);
        assert!(f.state.jump_held);
        r.wants_to_crouch = false;
        let held = v
            .pawn_diagnostic_tick(&w, &f.state, input(true), 0.05, &p, r)
            .unwrap();
        assert!(!held.jump_started);
        let released = v
            .pawn_diagnostic_tick(&w, &held.state, input(false), 0.05, &p, r)
            .unwrap();
        let fresh = v
            .pawn_diagnostic_tick(&w, &released.state, input(true), 0.05, &p, r)
            .unwrap();
        assert!(fresh.jump_started);
    }
    #[test]
    fn held_jump_lands_switches_to_walking_and_requires_release() {
        let (v, w, p, r) = fixture();
        let mut s = state(1.1);
        let mut jumps = 0;
        let mut walked = false;
        for _ in 0..30 {
            let f = v
                .pawn_diagnostic_tick(&w, &s, input(true), 0.05, &p, r)
                .unwrap();
            jumps += usize::from(f.jump_started);
            walked |= f.mode == MotionMode::Walking;
            s = f.state;
        }
        assert_eq!(jumps, 1);
        assert!(walked && s.body.grounded);
        assert_eq!(s.body.velocity, [0.0; 3]);
        s = v
            .pawn_diagnostic_tick(&w, &s, input(false), 0.05, &p, r)
            .unwrap()
            .state;
        let f = v
            .pawn_diagnostic_tick(&w, &s, input(true), 0.05, &p, r)
            .unwrap();
        assert!(f.jump_started);
        assert_eq!(f.mode, MotionMode::Falling);
        assert_eq!(f.next_mode, MotionMode::Falling);
    }
    #[test]
    fn airborne_press_is_consumed_and_landing_brakes_next_tick() {
        let (v, w, p, r) = fixture();
        let mut s = state(2.0);
        s.body.grounded = true;
        s.body.velocity = [10.0, 0.0, -40.0];
        let f = v
            .pawn_diagnostic_tick(&w, &s, input(true), 0.05, &p, r)
            .unwrap();
        assert!(!f.jump_started);
        assert_eq!(f.mode, MotionMode::Falling);
        assert_eq!(f.next_mode, MotionMode::Walking);
        assert_eq!(f.state.body.velocity[0], 10.0);
        let f = v
            .pawn_diagnostic_tick(&w, &f.state, input(true), 0.05, &p, r)
            .unwrap();
        assert!(!f.jump_started);
        assert_eq!(f.mode, MotionMode::Walking);
        assert_eq!(f.state.body.velocity[0], 5.0);
        let f = v
            .pawn_diagnostic_tick(&w, &f.state, input(false), 0.05, &p, r)
            .unwrap();
        assert_eq!(f.state.body.velocity, [0.0; 3]);
    }
    #[test]
    fn leaving_edge_routes_air_input_to_falling_on_next_tick() {
        let (v, mut w, mut p, mut r) = fixture();
        w.world = Ok(Arc::new(HullSet {
            hulls: vec![Hull {
                offset: 0,
                planes: vec![],
                bounds: [[-100.0, -100.0, -1.0], [0.0, 100.0, 0.0]],
            }],
            missing_solid_leaves: 0,
            empty_root_solid: false,
        }));
        p.controller.acceleration = 1000.0;
        p.controller.speed = 100.0;
        r.maximum_desired_speed = 100.0;
        let view = ViewInput {
            forward: 1.0,
            ..input(false)
        };
        let f = v
            .pawn_diagnostic_tick(&w, &state(1.1), view, 0.05, &p, r)
            .unwrap();
        assert_eq!(f.mode, MotionMode::Walking);
        assert_eq!(f.next_mode, MotionMode::Falling);
        let f = v
            .pawn_diagnostic_tick(&w, &f.state, view, 0.05, &p, r)
            .unwrap();
        assert_eq!(f.mode, MotionMode::Falling);
        assert!(f.state.body.velocity[2] < 0.0);
    }
    #[test]
    fn failures_do_not_consume_jump_input_or_return_partial_state() {
        let (mut v, w, p, r) = fixture();
        let s = state(1.1);
        v.default.water = true;
        assert!(v
            .pawn_diagnostic_tick(&w, &s, input(true), 0.05, &p, r)
            .is_err());
        v.default.water = false;
        assert!(v
            .pawn_diagnostic_tick(
                &w,
                &s,
                input(true),
                0.05,
                &p,
                PawnRuntimeOptions {
                    crouched: true,
                    ..r
                }
            )
            .is_err());
        let missing = StaticBodyWorld {
            world: Err("missing".into()),
            actors: vec![],
        };
        assert!(v
            .pawn_diagnostic_tick(&missing, &s, input(true), 0.05, &p, r)
            .is_err());
        assert!(!s.jump_held);
        assert_eq!(s.body.velocity, [0.0; 3]);
    }
}
