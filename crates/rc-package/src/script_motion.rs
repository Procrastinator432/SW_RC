//! Static PC movement adapter for reviewed authority jump/crouch script branches.
use crate::{
    controller::{ControlledBody, ViewInput},
    movement_profile::MovementProfile,
    pawn_jump::{JumpContext, JumpPhysics},
    pawn_motion::{MotionMode, PawnMotion, PawnRuntimeOptions},
    physics::BodyState,
    volumes::{VolumeSelection, VolumeWorld},
    walking_input::{walking_input_phase, WalkingInput, WalkingInputFrame, WalkingPawn},
    world_collision::StaticBodyWorld,
};
use serde::Serialize;
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ScriptBody {
    pub body: BodyState,
    pub pressed_jump: bool,
    pub wants_to_crouch: bool,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ScriptMotionInput {
    /// Movement axes only. view.jump must be false; use jump_event instead.
    pub view: ViewInput,
    pub jump_event: bool,
    pub cannot_jump_now: bool,
    pub duck: u8,
    pub can_crouch: bool,
}
#[derive(Debug, Serialize)]
pub struct ScriptMotionFrame {
    pub state: ScriptBody,
    /// Script phase snapshot before collision/integration, committed only on success.
    pub input_phase: WalkingInputFrame,
    pub volume: VolumeSelection,
    pub mode: MotionMode,
    pub next_mode: MotionMode,
    pub jump_started: bool,
    pub motion: PawnMotion,
}
impl VolumeWorld {
    /// Static standing-body diagnostic; no VM/events/network or dynamic crouch shape.
    pub fn script_motion_tick(
        &self,
        world: &StaticBodyWorld,
        state: &ScriptBody,
        input: ScriptMotionInput,
        dt: f64,
        profile: &MovementProfile,
        mut runtime: PawnRuntimeOptions,
    ) -> Result<ScriptMotionFrame, String> {
        if input.view.jump || runtime.crouched {
            return Err("script motion requires separate jump_event and standing body".into());
        }
        let volume = self.select(state.body.position.map(|v| v as f32))?;
        if !volume.complete {
            return Err("script motion volume selection incomplete".into());
        }
        let physics = volume.settings.apply(profile.physics)?;
        let supported = state.body.velocity[2] <= 0.0
            && world
                .floor_contact(
                    state.body.position,
                    physics.extent,
                    physics.support_distance,
                    physics.minimum_up,
                )?
                .is_some();
        let input_phase = walking_input_phase(
            Some(WalkingPawn {
                velocity: state.body.velocity,
                jump: JumpContext {
                    physics: if supported {
                        JumpPhysics::Walking
                    } else {
                        JumpPhysics::Falling
                    },
                    crouched: runtime.crouched,
                    wants_to_crouch: state.wants_to_crouch,
                    walking: input.view.walking,
                    current_jump_z: runtime.current_jump_z,
                    default_jump_z: profile.controller.jump_speed,
                    floor: [0.0, 0.0, 1.0],
                    base: None,
                },
            }),
            WalkingInput {
                pressed_jump: state.pressed_jump || input.jump_event,
                cannot_jump_now: input.cannot_jump_now,
                duck: input.duck,
                can_crouch: input.can_crouch,
            },
        )?;
        let pawn = input_phase
            .pawn
            .as_ref()
            .ok_or("script motion requires pawn")?;
        if pawn.jump.wants_to_crouch {
            return Err("pending crouch requires unsupported body-size transition".into());
        }
        let mut body = state.body;
        body.velocity = pawn.velocity;
        if input_phase.jump_started {
            body.grounded = false;
        }
        // Existing adapter provides the shared volume/velocity/static-collision path.
        // Its own jump edge is disabled; this phase already applied DoJump once.
        runtime.wants_to_crouch = false;
        let movement = self.pawn_diagnostic_tick(
            world,
            &ControlledBody {
                body,
                jump_held: false,
            },
            input.view,
            dt,
            profile,
            runtime,
        )?;
        Ok(ScriptMotionFrame {
            state: ScriptBody {
                body: movement.state.body,
                pressed_jump: input_phase.pressed_jump_after,
                wants_to_crouch: pawn.jump.wants_to_crouch,
            },
            jump_started: input_phase.jump_started,
            input_phase,
            volume: movement.volume,
            mode: movement.mode,
            next_mode: movement.next_mode,
            motion: movement.motion,
        })
    }
}
