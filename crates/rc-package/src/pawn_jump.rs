//! Arithmetic and permission branches of extracted Engine.Pawn.DoJump.
//! Sound/events, SetPhysics and controller input processing are caller responsibilities.
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum JumpPhysics {
    Walking,
    Falling,
    Ladder,
    Spider,
    Other,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub struct JumpBase {
    pub world_geometry: bool,
    pub vertical_velocity: f64,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub struct JumpContext {
    pub physics: JumpPhysics,
    pub crouched: bool,
    pub wants_to_crouch: bool,
    pub walking: bool,
    pub current_jump_z: f64,
    pub default_jump_z: f64,
    pub floor: [f64; 3],
    pub base: Option<JumpBase>,
}

/// None means the script's entry condition denied the jump.
/// No bCanJump/health check exists in this extracted function.
pub fn jump_velocity(velocity: [f64; 3], context: JumpContext) -> Result<Option<[f64; 3]>, String> {
    if context.crouched
        || context.wants_to_crouch
        || matches!(context.physics, JumpPhysics::Falling | JumpPhysics::Other)
    {
        return Ok(None);
    }
    let mut result = match context.physics {
        JumpPhysics::Spider => context.floor.map(|v| v * context.current_jump_z),
        JumpPhysics::Ladder => [velocity[0], velocity[1], 0.0],
        JumpPhysics::Walking => [
            velocity[0],
            velocity[1],
            if context.walking {
                context.default_jump_z
            } else {
                context.current_jump_z
            },
        ],
        JumpPhysics::Falling | JumpPhysics::Other => unreachable!(),
    };
    if let Some(base) = context.base {
        if !base.world_geometry {
            result[2] += base.vertical_velocity;
        }
    }
    if !result.iter().all(|v| v.is_finite()) {
        return Err("nonfinite DoJump velocity".into());
    }
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context() -> JumpContext {
        JumpContext {
            physics: JumpPhysics::Walking,
            crouched: false,
            wants_to_crouch: false,
            walking: false,
            current_jump_z: 600.0,
            default_jump_z: 475.0,
            floor: [0.0, 0.6, 0.8],
            base: None,
        }
    }
    #[test]
    fn walking_uses_default_running_uses_current_and_preserves_xy() {
        let mut c = context();
        assert_eq!(
            jump_velocity([12.0, -8.0, -20.0], c).unwrap(),
            Some([12.0, -8.0, 600.0])
        );
        c.walking = true;
        assert_eq!(
            jump_velocity([12.0, -8.0, -20.0], c).unwrap(),
            Some([12.0, -8.0, 475.0])
        );
    }
    #[test]
    fn crouch_request_and_unsupported_physics_deny() {
        for c in [
            JumpContext {
                crouched: true,
                ..context()
            },
            JumpContext {
                wants_to_crouch: true,
                ..context()
            },
            JumpContext {
                physics: JumpPhysics::Other,
                ..context()
            },
            JumpContext {
                physics: JumpPhysics::Falling,
                ..context()
            },
        ] {
            assert_eq!(jump_velocity([0.0; 3], c).unwrap(), None);
        }
    }
    #[test]
    fn ladder_spider_and_nonworld_base_follow_script_order() {
        let mut c = context();
        c.base = Some(JumpBase {
            world_geometry: false,
            vertical_velocity: -30.0,
        });
        c.physics = JumpPhysics::Ladder;
        assert_eq!(
            jump_velocity([12.0, -8.0, 100.0], c).unwrap(),
            Some([12.0, -8.0, -30.0])
        );
        c.physics = JumpPhysics::Spider;
        c.walking = true; // Spider precedes bIsWalking and still uses current JumpZ.
        assert_eq!(
            jump_velocity([12.0, -8.0, 100.0], c).unwrap(),
            Some([0.0, 360.0, 450.0])
        );
        c.physics = JumpPhysics::Walking;
        c.base.as_mut().unwrap().world_geometry = true;
        assert_eq!(
            jump_velocity([12.0, -8.0, 100.0], c).unwrap(),
            Some([12.0, -8.0, 475.0])
        );
    }
    #[test]
    fn nonfinite_used_values_fail_without_a_candidate() {
        let mut c = context();
        c.current_jump_z = f64::NAN;
        assert!(jump_velocity([0.0; 3], c).is_err());
        c.current_jump_z = 600.0;
        c.base = Some(JumpBase {
            world_geometry: false,
            vertical_velocity: f64::INFINITY,
        });
        assert!(jump_velocity([0.0; 3], c).is_err());
    }
}
