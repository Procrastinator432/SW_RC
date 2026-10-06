//! Jump/crouch projection of Engine.PlayerController.PlayerWalking's authority path.
//! Snapshots only: no bindings, replication, acceleration, camera or Script VM.
use crate::pawn_jump::{jump_velocity, JumpContext, JumpPhysics};
use serde::Serialize;
#[derive(Debug, Clone, Copy, Serialize)]
pub struct WalkingInput {
    /// Pending script event, not a physical held-button state.
    pub pressed_jump: bool,
    /// Pawn.CannotJumpNow snapshot; the base implementation returns false.
    pub cannot_jump_now: bool,
    pub duck: u8,
    pub can_crouch: bool,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub struct WalkingPawn {
    pub jump: JumpContext,
    pub velocity: [f64; 3],
}
#[derive(Debug, Serialize)]
pub struct WalkingInputFrame {
    pub pawn: Option<WalkingPawn>,
    pub do_jump_called: bool,
    pub jump_started: bool,
    /// Records a ShouldCrouch call, including redundant assignments.
    pub crouch_request: Option<bool>,
    pub saved_jump: bool,
    pub pressed_jump_after: bool,
    pub double_jump_after: bool,
}
/// Jump preflight, relevant ProcessMove branches, and flag cleanup on authority.
pub fn walking_input_phase(
    pawn: Option<WalkingPawn>,
    input: WalkingInput,
) -> Result<WalkingInputFrame, String> {
    let saved_jump = input.pressed_jump && pawn.is_some() && input.cannot_jump_now;
    let do_jump_called = input.pressed_jump && pawn.is_some() && !saved_jump;
    let mut pawn = pawn;
    let mut jump_started = false;
    let mut crouch_request = None;
    if let Some(pawn) = pawn.as_mut() {
        if do_jump_called {
            if let Some(velocity) = jump_velocity(pawn.velocity, pawn.jump)? {
                pawn.velocity = velocity;
                pawn.jump.physics = JumpPhysics::Falling;
                jump_started = true;
            }
        }
        // DoJump may have changed Physics before this condition.
        if pawn.jump.physics != JumpPhysics::Falling {
            crouch_request = if input.duck == 0 {
                Some(false)
            } else if input.can_crouch {
                Some(true)
            } else {
                None
            };
            if let Some(value) = crouch_request {
                pawn.jump.wants_to_crouch = value;
            }
        }
    }
    Ok(WalkingInputFrame {
        pawn,
        do_jump_called,
        jump_started,
        crouch_request,
        saved_jump,
        pressed_jump_after: saved_jump,
        double_jump_after: false,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn pawn() -> WalkingPawn {
        WalkingPawn {
            velocity: [12.0, -8.0, 0.0],
            jump: JumpContext {
                physics: JumpPhysics::Walking,
                crouched: false,
                wants_to_crouch: false,
                walking: false,
                current_jump_z: 600.0,
                default_jump_z: 475.0,
                floor: [0.0, 0.0, 1.0],
                base: None,
            },
        }
    }
    fn input() -> WalkingInput {
        WalkingInput {
            pressed_jump: true,
            cannot_jump_now: false,
            duck: 0,
            can_crouch: true,
        }
    }
    #[test]
    fn simultaneous_jump_and_duck_jumps_before_crouch_update() {
        let f = walking_input_phase(Some(pawn()), WalkingInput { duck: 1, ..input() }).unwrap();
        assert!(f.do_jump_called && f.jump_started);
        assert_eq!(f.crouch_request, None);
        assert_eq!(f.pawn.unwrap().jump.physics, JumpPhysics::Falling);
        assert!(!f.pressed_jump_after && !f.double_jump_after);
    }
    #[test]
    fn prior_crouch_request_denies_jump_before_release_is_applied() {
        let mut p = pawn();
        p.jump.wants_to_crouch = true;
        let f = walking_input_phase(Some(p), input()).unwrap();
        assert!(f.do_jump_called && !f.jump_started);
        assert_eq!(f.crouch_request, Some(false));
        assert!(!f.pressed_jump_after);
        let idle = walking_input_phase(
            f.pawn,
            WalkingInput {
                pressed_jump: false,
                ..input()
            },
        )
        .unwrap();
        assert!(!idle.jump_started);
        let fresh = walking_input_phase(idle.pawn, input()).unwrap();
        assert!(fresh.jump_started);
    }
    #[test]
    fn cannot_jump_saves_event_and_retries_on_a_later_step() {
        let deferred = walking_input_phase(
            Some(pawn()),
            WalkingInput {
                cannot_jump_now: true,
                ..input()
            },
        )
        .unwrap();
        assert!(deferred.saved_jump && deferred.pressed_jump_after);
        assert!(!deferred.do_jump_called && !deferred.jump_started);
        let retried = walking_input_phase(
            deferred.pawn,
            WalkingInput {
                pressed_jump: deferred.pressed_jump_after,
                ..input()
            },
        )
        .unwrap();
        assert!(retried.jump_started && !retried.pressed_jump_after);
    }
    #[test]
    fn absent_pawn_and_airborne_failure_consume_event() {
        let f = walking_input_phase(
            None,
            WalkingInput {
                cannot_jump_now: true,
                ..input()
            },
        )
        .unwrap();
        assert!(!f.saved_jump && !f.do_jump_called && !f.pressed_jump_after);
        let mut p = pawn();
        p.jump.physics = JumpPhysics::Falling;
        let f = walking_input_phase(Some(p), WalkingInput { duck: 1, ..input() }).unwrap();
        assert!(f.do_jump_called && !f.jump_started);
        assert_eq!(f.crouch_request, None);
        assert!(!f.pressed_jump_after);
    }
    #[test]
    fn crouch_permission_preserves_request_until_duck_is_released() {
        let enabled = walking_input_phase(
            Some(pawn()),
            WalkingInput {
                pressed_jump: false,
                duck: 2,
                ..input()
            },
        )
        .unwrap();
        assert_eq!(enabled.crouch_request, Some(true));
        let p = enabled.pawn.unwrap();
        assert!(p.jump.wants_to_crouch);
        let f = walking_input_phase(
            Some(p),
            WalkingInput {
                pressed_jump: false,
                duck: 1,
                can_crouch: false,
                ..input()
            },
        )
        .unwrap();
        assert_eq!(f.crouch_request, None);
        assert!(f.pawn.unwrap().jump.wants_to_crouch);
        let released = walking_input_phase(
            f.pawn,
            WalkingInput {
                pressed_jump: false,
                can_crouch: false,
                ..input()
            },
        )
        .unwrap();
        assert_eq!(released.crouch_request, Some(false));
        assert!(!released.pawn.unwrap().jump.wants_to_crouch);
    }
    #[test]
    fn failed_velocity_calculation_has_no_partial_frame() {
        let mut p = pawn();
        p.jump.current_jump_z = f64::NAN;
        assert!(walking_input_phase(Some(p), input()).is_err());
        assert!(p.jump.current_jump_z.is_nan());
        assert_eq!(p.jump.physics, JumpPhysics::Walking);
    }
}
