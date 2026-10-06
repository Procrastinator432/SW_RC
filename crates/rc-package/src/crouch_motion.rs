//! Reviewed normal crouch dispatch order around own static PC movement.
use crate::{
    controller::{ControlledBody, ViewInput},
    crouch::{CrouchBody, CrouchFrame, CrouchProfile},
    movement_profile::MovementProfile,
    pawn_jump::{JumpContext, JumpPhysics},
    pawn_motion::{MotionMode, PawnDiagnosticFrame, PawnRuntimeOptions},
    script_motion::{ScriptBody, ScriptMotionInput},
    volumes::VolumeWorld,
    walking_input::{walking_input_phase, WalkingInput, WalkingInputFrame, WalkingPawn},
    world_collision::StaticBodyWorld,
};
use serde::Serialize;
#[derive(Debug, Clone, Copy, Serialize)]
pub struct CrouchingScriptBody {
    pub script: ScriptBody,
    pub crouched: bool,
    pub try_to_uncrouch: bool,
    /// Native UncrouchTime snapshot; explicit HitWall adapter may arm it.
    pub uncrouch_time: f32,
}
pub struct CrouchMotionOptions<'a> {
    pub runtime: PawnRuntimeOptions,
    pub shape: &'a CrouchProfile,
}
#[derive(Debug, Clone, Copy)]
pub struct CrouchPhysicsInput {
    /// Diagnostic movement axes only; jump must be false.
    pub view: ViewInput,
    pub can_crouch: bool,
}
#[derive(Debug, Serialize)]
pub struct CrouchingPhysicsFrame {
    pub state: CrouchingScriptBody,
    pub before_movement: Option<CrouchFrame>,
    pub movement: PawnDiagnosticFrame,
    pub after_movement: Option<CrouchFrame>,
}
#[derive(Debug, Serialize)]
pub struct CrouchingScriptFrame {
    pub state: CrouchingScriptBody,
    pub input_phase: WalkingInputFrame,
    pub before_movement: Option<CrouchFrame>,
    pub movement: PawnDiagnosticFrame,
    pub after_movement: Option<CrouchFrame>,
}
impl VolumeWorld {
    fn crouch_support(
        &self,
        world: &StaticBodyWorld,
        state: &CrouchingScriptBody,
        dt: f64,
        profile: &MovementProfile,
        shape: &CrouchProfile,
    ) -> Result<bool, String> {
        if !state.uncrouch_time.is_finite()
            || !dt.is_finite()
            || dt <= 0.0
            || dt > 0.05
            || profile.physics.extent != shape.standing
        {
            return Err("invalid timer/dt or standing shape mismatch".into());
        }
        let checked = world.crouch_diagnostic(
            &CrouchBody {
                body: state.script.body,
                crouched: state.crouched,
            },
            state.crouched,
            shape,
        )?;
        let volume = self.select(state.script.body.position.map(|v| v as f32))?;
        if !volume.complete {
            return Err("crouch motion volume selection incomplete".into());
        }
        let physics = volume.settings.apply(profile.physics)?;
        Ok(state.script.body.velocity[2] <= 0.0
            && world
                .floor_contact(
                    state.script.body.position,
                    checked.extent,
                    physics.support_distance,
                    physics.minimum_up,
                )?
                .is_some())
    }
    /// Script phase -> native normal crouch ordering -> own static movement.
    /// Entire result is atomic. Timer arming/callback/native geometry parity is absent.
    pub fn crouching_script_tick(
        &self,
        world: &StaticBodyWorld,
        state: &CrouchingScriptBody,
        input: ScriptMotionInput,
        dt: f64,
        profile: &MovementProfile,
        options: CrouchMotionOptions<'_>,
    ) -> Result<CrouchingScriptFrame, String> {
        if input.view.jump {
            return Err("ambiguous script jump input".into());
        }
        let supported = self.crouch_support(world, state, dt, profile, options.shape)?;
        let input_phase = walking_input_phase(
            Some(WalkingPawn {
                velocity: state.script.body.velocity,
                jump: JumpContext {
                    physics: if supported {
                        JumpPhysics::Walking
                    } else {
                        JumpPhysics::Falling
                    },
                    crouched: state.crouched,
                    wants_to_crouch: state.script.wants_to_crouch,
                    walking: input.view.walking,
                    current_jump_z: options.runtime.current_jump_z,
                    default_jump_z: profile.controller.jump_speed,
                    floor: [0.0, 0.0, 1.0],
                    base: None,
                },
            }),
            WalkingInput {
                pressed_jump: state.script.pressed_jump || input.jump_event,
                cannot_jump_now: input.cannot_jump_now,
                duck: input.duck,
                can_crouch: input.can_crouch,
            },
        )?;
        let pawn = input_phase.pawn.as_ref().ok_or("missing crouch pawn")?;
        let mut prepared = *state;
        prepared.script.body.velocity = pawn.velocity;
        prepared.script.wants_to_crouch = pawn.jump.wants_to_crouch;
        prepared.script.pressed_jump = input_phase.pressed_jump_after;
        if input_phase.jump_started {
            prepared.script.body.grounded = false;
        }
        let mut physics = self.crouching_physics_tick(
            world,
            &prepared,
            CrouchPhysicsInput {
                view: input.view,
                can_crouch: input.can_crouch,
            },
            dt,
            profile,
            options,
        )?;
        physics.movement.jump_started = input_phase.jump_started;
        Ok(CrouchingScriptFrame {
            state: physics.state,
            input_phase,
            before_movement: physics.before_movement,
            movement: physics.movement,
            after_movement: physics.after_movement,
        })
    }
    /// Physics-only entry: preserves pending script jump and stored crouch request.
    /// It does not execute PlayerWalking, DoJump, AI destination logic or HitWall callbacks.
    pub fn crouching_physics_tick(
        &self,
        world: &StaticBodyWorld,
        state: &CrouchingScriptBody,
        input: CrouchPhysicsInput,
        dt: f64,
        profile: &MovementProfile,
        options: CrouchMotionOptions<'_>,
    ) -> Result<CrouchingPhysicsFrame, String> {
        if input.view.jump {
            return Err("physics-only crouch input cannot execute jump".into());
        }
        let walking = self.crouch_support(world, state, dt, profile, options.shape)?;
        let mut wants = state.script.wants_to_crouch;
        let mut try_to_uncrouch = state.try_to_uncrouch;
        let mut uncrouch_time = state.uncrouch_time;
        let mut body = CrouchBody {
            body: state.script.body,
            crouched: state.crouched,
        };
        // Native else-if: freshly entering crouch does not decrement this tick.
        // Only already crouched Walking + Wants + Can + Try reaches Pawn+0x650.
        if walking && wants && input.can_crouch && body.crouched && try_to_uncrouch {
            uncrouch_time -= dt as f32;
            if uncrouch_time <= 0.0 {
                wants = false;
                try_to_uncrouch = false;
            }
        }
        let before_movement = if walking && wants && input.can_crouch && !body.crouched {
            Some(world.crouch_diagnostic(&body, true, options.shape)?)
        } else if !walking && body.crouched {
            Some(world.crouch_diagnostic(&body, false, options.shape)?)
        } else {
            None
        };
        if let Some(frame) = &before_movement {
            body = frame.state;
        }
        let mut runtime = options.runtime;
        runtime.crouched = body.crouched;
        runtime.wants_to_crouch = wants;
        let mut physics = profile.physics;
        physics.extent = if body.crouched {
            options.shape.crouching
        } else {
            options.shape.standing
        };
        let shaped_profile = MovementProfile {
            controller: profile.controller,
            ratios: profile.ratios,
            physics,
            properties: std::collections::BTreeMap::new(),
        };
        let movement = self.pawn_shape_tick(
            world,
            &ControlledBody {
                body: body.body,
                jump_held: false,
            },
            input.view,
            dt,
            &shaped_profile,
            runtime,
        )?;
        body.body = movement.state.body;
        let after_movement =
            if body.crouched && (movement.next_mode != MotionMode::Walking || !wants) {
                Some(world.crouch_diagnostic(&body, false, options.shape)?)
            } else {
                None
            };
        if let Some(frame) = &after_movement {
            body = frame.state;
        }
        Ok(CrouchingPhysicsFrame {
            state: CrouchingScriptBody {
                script: ScriptBody {
                    body: body.body,
                    pressed_jump: state.script.pressed_jump,
                    wants_to_crouch: wants,
                },
                crouched: body.crouched,
                try_to_uncrouch,
                uncrouch_time,
            },
            before_movement,
            movement,
            after_movement,
        })
    }
}
