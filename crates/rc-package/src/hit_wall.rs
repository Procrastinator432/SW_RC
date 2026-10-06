//! Reviewed first auto-crouch attempt in APawn.processHitWall.
//! Controller/event outcomes are supplied snapshots; no callback or recovery movement.
use crate::{
    auto_crouch::{AutoCrouchDecision, AutoCrouchFrame},
    crouch::CrouchProfile,
    crouch_motion::CrouchingScriptBody,
    pawn_jump::JumpPhysics,
    world_collision::{StaticBodyWorld, WorldSweep},
};
use serde::Serialize;

#[derive(Debug, Clone, Copy)]
pub struct HitWallContext {
    pub wall_present: bool,
    /// Caller supplies the native excluded-class test; class identity is not inferred.
    pub excluded_wall_class: bool,
    pub direct_hit_wall: bool,
    pub controller_present: bool,
    pub destination: [f64; 3],
    pub wall_normal: [f64; 3],
    pub minimum_hit_wall: f64,
    /// Result of native NotifyHitWall, not a callback executed by this adapter.
    pub notify_handled: bool,
    pub physics: JumpPhysics,
    pub human_controlled: bool,
    pub can_crouch: bool,
}
#[derive(Debug, PartialEq, Eq, Serialize)]
pub enum HitWallDecision {
    NoWall,
    ExcludedWall,
    DirectHitWall,
    NoController,
    Stationary,
    AngleFiltered,
    NotifyHandled,
    NotWalking,
    HumanControlled,
    CannotCrouch,
    AlreadyCrouched,
    Armed,
    RecoveryRequired,
}
#[derive(Debug, Serialize)]
pub struct HitWallFrame {
    pub state: CrouchingScriptBody,
    pub decision: HitWallDecision,
    pub destination_direction: Option<[f64; 3]>,
    pub wall_direction: Option<[f64; 3]>,
    pub direction_dot: Option<f64>,
    pub first_attempt: Option<AutoCrouchFrame>,
}
#[derive(Debug, PartialEq, Eq, Serialize)]
pub enum HitWallRecoveryDecision {
    Skipped,
    ArmedFirst,
    ArmedSecond,
    Unresolved,
}
#[derive(Debug, Serialize)]
pub struct DownwardRecovery {
    pub requested_delta: [f64; 3],
    pub position: [f64; 3],
    pub travel_fraction: f64,
    pub sweep: WorldSweep,
}
#[derive(Debug, Serialize)]
pub struct HitWallRecoveryFrame {
    pub state: CrouchingScriptBody,
    pub decision: HitWallRecoveryDecision,
    pub first: HitWallFrame,
    pub recovery: Option<DownwardRecovery>,
    pub second: Option<AutoCrouchFrame>,
}
fn normalized(v: [f64; 3]) -> [f64; 3] {
    let length = v[0].hypot(v[1]).hypot(v[2]);
    if length == 0.0 {
        v
    } else {
        v.map(|x| x / length)
    }
}
impl StaticBodyWorld {
    /// First attempt only. RecoveryRequired explicitly leaves the native downward
    /// MoveActor and second attempt unexecuted; it is not a full HitWall outcome.
    pub fn hit_wall_auto_crouch_first_attempt(
        &self,
        state: &CrouchingScriptBody,
        context: HitWallContext,
        profile: &CrouchProfile,
    ) -> Result<HitWallFrame, String> {
        let mut frame = HitWallFrame {
            state: *state,
            decision: HitWallDecision::NoWall,
            destination_direction: None,
            wall_direction: None,
            direction_dot: None,
            first_attempt: None,
        };
        let early = if !context.wall_present {
            Some(HitWallDecision::NoWall)
        } else if context.excluded_wall_class {
            Some(HitWallDecision::ExcludedWall)
        } else if context.direct_hit_wall {
            Some(HitWallDecision::DirectHitWall)
        } else if !context.controller_present {
            Some(HitWallDecision::NoController)
        } else {
            None
        };
        if let Some(decision) = early {
            frame.decision = decision;
            return Ok(frame);
        }
        if state.script.body.velocity.iter().any(|v| !v.is_finite()) {
            return Err("invalid HitWall velocity".into());
        }
        if state.script.body.velocity == [0.0; 3] {
            frame.decision = HitWallDecision::Stationary;
            return Ok(frame);
        }
        if state
            .script
            .body
            .position
            .iter()
            .chain(&context.destination)
            .chain(&context.wall_normal)
            .any(|v| !v.is_finite() || v.abs() > 1e9)
            || !context.minimum_hit_wall.is_finite()
        {
            return Err("invalid HitWall direction snapshot".into());
        }
        // Native uses Controller.Destination, not Velocity, for the angle filter.
        let mut direction = normalized(std::array::from_fn(|i| {
            context.destination[i] - state.script.body.position[i]
        }));
        let mut wall = context.wall_normal;
        if context.physics == JumpPhysics::Walking {
            direction[2] = 0.0;
            wall[2] = 0.0;
            direction = normalized(direction);
            wall = normalized(wall);
        }
        let dot: f64 = (0..3).map(|i| direction[i] * wall[i]).sum();
        frame.destination_direction = Some(direction);
        frame.wall_direction = Some(wall);
        frame.direction_dot = Some(dot);
        let gate = if dot > context.minimum_hit_wall {
            Some(HitWallDecision::AngleFiltered)
        } else if context.notify_handled {
            Some(HitWallDecision::NotifyHandled)
        } else if context.physics != JumpPhysics::Walking {
            Some(HitWallDecision::NotWalking)
        } else if context.human_controlled {
            Some(HitWallDecision::HumanControlled)
        } else if !context.can_crouch {
            Some(HitWallDecision::CannotCrouch)
        } else if state.crouched {
            Some(HitWallDecision::AlreadyCrouched)
        } else {
            None
        };
        if let Some(decision) = gate {
            frame.decision = decision;
            return Ok(frame);
        }
        profile.validate()?;
        let start = state.script.body.position;
        let end = std::array::from_fn(|i| start[i] + profile.standing[0] * direction[i]);
        let attempt = self.can_crouch_walk_diagnostic(state, start, end, true, profile)?;
        frame.decision = if attempt.decision == AutoCrouchDecision::Armed {
            HitWallDecision::Armed
        } else {
            HitWallDecision::RecoveryRequired
        };
        frame.state = attempt.state;
        frame.first_attempt = Some(attempt);
        Ok(frame)
    }
    /// Eligible Walking/nonhuman branch only: first probe, own downward35 sweep,
    /// second probe. No native MoveActor callbacks/filters or HitWall fallback.
    pub fn hit_wall_auto_crouch_recovery(
        &self,
        state: &CrouchingScriptBody,
        context: HitWallContext,
        profile: &CrouchProfile,
        skin: f64,
    ) -> Result<HitWallRecoveryFrame, String> {
        let first = self.hit_wall_auto_crouch_first_attempt(state, context, profile)?;
        let decision = if first.decision == HitWallDecision::Armed {
            HitWallRecoveryDecision::ArmedFirst
        } else {
            HitWallRecoveryDecision::Skipped
        };
        let mut frame = HitWallRecoveryFrame {
            state: first.state,
            decision,
            first,
            recovery: None,
            second: None,
        };
        if frame.first.decision != HitWallDecision::RecoveryRequired {
            return Ok(frame);
        }
        if !skin.is_finite() || skin <= 0.0 || skin > 35.0 {
            return Err("invalid HitWall recovery skin".into());
        }
        let start = state.script.body.position;
        let initial = self.body_placement(start, profile.standing)?;
        if initial.state == crate::movement::PlacementState::Penetrating {
            return Err("HitWall recovery initial penetration".into());
        }
        let end = [start[0], start[1], start[2] - 35.0];
        let sweep = self.sweep_motion(start, end, profile.standing)?;
        if !sweep.complete {
            return Err("HitWall recovery sweep incomplete".into());
        }
        let fraction = if let Some(contact) = sweep.contacts.first() {
            let approach = 35.0 * contact.hit.normal[2];
            if contact.hit.start_overlapping || !approach.is_finite() || approach <= 0.0 {
                return Err("invalid HitWall recovery contact".into());
            }
            (contact.hit.fraction - skin / approach).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let position = [start[0], start[1], start[2] - 35.0 * fraction];
        let placement = self.body_placement(position, profile.standing)?;
        if placement.state == crate::movement::PlacementState::Penetrating {
            return Err("HitWall recovery candidate penetrates".into());
        }
        let mut recovered = frame.state;
        recovered.script.body.position = position;
        let direction = frame
            .first
            .destination_direction
            .ok_or("missing HitWall direction")?;
        let end = std::array::from_fn(|i| position[i] + profile.standing[0] * direction[i]);
        // Native ignores MoveActor's return and reuses its saved direction.
        let second = self.can_crouch_walk_diagnostic(&recovered, position, end, true, profile)?;
        frame.decision = if second.decision == AutoCrouchDecision::Armed {
            HitWallRecoveryDecision::ArmedSecond
        } else {
            HitWallRecoveryDecision::Unresolved
        };
        frame.state = second.state;
        frame.recovery = Some(DownwardRecovery {
            requested_delta: [0.0, 0.0, -35.0],
            position,
            travel_fraction: fraction,
            sweep,
        });
        frame.second = Some(second);
        Ok(frame)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        hull::{Hull, HullSet},
        physics::BodyState,
        script_motion::ScriptBody,
    };
    use std::sync::Arc;
    fn fixture() -> (
        StaticBodyWorld,
        CrouchProfile,
        CrouchingScriptBody,
        HitWallContext,
    ) {
        let world = StaticBodyWorld {
            world: Ok(Arc::new(HullSet {
                hulls: vec![],
                missing_solid_leaves: 0,
                empty_root_solid: false,
            })),
            actors: vec![],
        };
        let profile = CrouchProfile {
            standing: [2.0, 2.0, 2.0],
            crouching: [1.0; 3],
            properties: Default::default(),
        };
        let state = CrouchingScriptBody {
            script: ScriptBody {
                body: BodyState {
                    position: [0.0, 0.0, 2.0],
                    velocity: [-2.0, 0.0, 0.0],
                    grounded: true,
                },
                pressed_jump: false,
                wants_to_crouch: false,
            },
            crouched: false,
            try_to_uncrouch: false,
            uncrouch_time: 0.0,
        };
        let context = HitWallContext {
            wall_present: true,
            excluded_wall_class: false,
            direct_hit_wall: false,
            controller_present: true,
            destination: [10.0, 0.0, 20.0],
            wall_normal: [-1.0, 0.0, 0.5],
            minimum_hit_wall: 0.0,
            notify_handled: false,
            physics: JumpPhysics::Walking,
            human_controlled: false,
            can_crouch: true,
        };
        (world, profile, state, context)
    }
    #[test]
    fn recovery_moves_down_and_second_probe_can_arm() {
        let (mut w, p, mut s, mut c) = fixture();
        s.script.body.position[2] = 40.0;
        c.destination = [10.0, 0.0, 40.0];
        // Thin beam across the probe, outside the standing body's X footprint.
        w.world = Ok(Arc::new(HullSet {
            hulls: vec![Hull {
                offset: 0,
                planes: vec![],
                bounds: [[2.5, 0.5, 38.0], [3.0, 1.5, 40.0]],
            }],
            missing_solid_leaves: 0,
            empty_root_solid: false,
        }));
        let f = w.hit_wall_auto_crouch_recovery(&s, c, &p, 0.1).unwrap();
        assert_eq!(f.first.decision, HitWallDecision::RecoveryRequired);
        assert_eq!(f.decision, HitWallRecoveryDecision::ArmedSecond);
        assert_eq!(f.state.script.body.position, [0.0, 0.0, 5.0]);
        assert_eq!(f.recovery.unwrap().requested_delta, [0.0, 0.0, -35.0]);
        assert_eq!(f.state.script.body.velocity, s.script.body.velocity);
        assert_eq!(f.state.script.body.grounded, s.script.body.grounded);
        assert!(!f.state.crouched && f.state.try_to_uncrouch);
        assert_eq!(f.state.uncrouch_time, 0.5);
    }
    #[test]
    fn partially_blocked_downward_move_retries_from_the_actual_position() {
        let (mut w, p, mut s, mut c) = fixture();
        s.script.body.position[2] = 10.0;
        c.destination = [10.0, 0.0, 10.0];
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
                    bounds: [[2.5, 0.5, 8.0], [3.0, 1.5, 11.0]],
                },
            ],
            missing_solid_leaves: 0,
            empty_root_solid: false,
        }));
        let f = w.hit_wall_auto_crouch_recovery(&s, c, &p, 0.1).unwrap();
        assert_eq!(f.decision, HitWallRecoveryDecision::ArmedSecond);
        let recovery = f.recovery.unwrap();
        assert!(recovery.travel_fraction > 0.0 && recovery.travel_fraction < 1.0);
        assert!((recovery.position[2] - 2.1).abs() < 1e-12);
        let segment = f.second.unwrap().shifted_segment.unwrap();
        assert!((segment[0][2] - 1.1).abs() < 1e-12);
        assert_eq!(segment[1][0], 2.0);
        assert_eq!(segment[0][2], segment[1][2]);
    }
    #[test]
    fn floor_limits_recovery_and_blocked_second_probe_remains_unresolved() {
        let (mut w, p, mut s, mut c) = fixture();
        s.script.body.position[2] = 2.1;
        c.destination = [10.0, 0.0, 2.1];
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
                    bounds: [[2.5, 0.5, 0.0], [3.0, 1.5, 4.0]],
                },
            ],
            missing_solid_leaves: 0,
            empty_root_solid: false,
        }));
        let f = w.hit_wall_auto_crouch_recovery(&s, c, &p, 0.1).unwrap();
        assert_eq!(f.decision, HitWallRecoveryDecision::Unresolved);
        let recovery = f.recovery.unwrap();
        assert!(recovery.travel_fraction < 1e-12);
        assert!((f.state.script.body.position[2] - 2.1).abs() < 1e-12);
        assert!(f.second.is_some());
        assert!(!f.state.try_to_uncrouch && !f.state.script.wants_to_crouch);
    }
    #[test]
    fn successful_first_probe_and_gates_skip_recovery() {
        let (w, p, s, mut c) = fixture();
        let f = w
            .hit_wall_auto_crouch_recovery(&s, c, &p, f64::NAN)
            .unwrap();
        assert_eq!(f.decision, HitWallRecoveryDecision::ArmedFirst);
        assert!(f.recovery.is_none() && f.second.is_none());
        c.human_controlled = true;
        let f = w
            .hit_wall_auto_crouch_recovery(&s, c, &p, f64::NAN)
            .unwrap();
        assert_eq!(f.decision, HitWallRecoveryDecision::Skipped);
        assert_eq!(f.state.script.body.position, s.script.body.position);
    }
    #[test]
    fn recovery_invalid_skin_and_initial_penetration_are_atomic_errors() {
        let (mut w, p, s, c) = fixture();
        // Initial body intersects obstacle; first point probe detects it.
        w.world = Ok(Arc::new(HullSet {
            hulls: vec![Hull {
                offset: 0,
                planes: vec![],
                bounds: [[-0.5, -0.5, 0.5], [0.5, 0.5, 2.5]],
            }],
            missing_solid_leaves: 0,
            empty_root_solid: false,
        }));
        assert!(w.hit_wall_auto_crouch_recovery(&s, c, &p, 0.0).is_err());
        assert!(w.hit_wall_auto_crouch_recovery(&s, c, &p, 0.1).is_err());
        w.world = Err("missing".into());
        assert!(w.hit_wall_auto_crouch_recovery(&s, c, &p, 0.1).is_err());
        assert_eq!(s.script.body.position, [0.0, 0.0, 2.0]);
        assert!(!s.try_to_uncrouch);
    }
    #[test]
    fn destination_drives_flattened_walking_probe_despite_opposite_velocity() {
        let (w, p, s, c) = fixture();
        let f = w.hit_wall_auto_crouch_first_attempt(&s, c, &p).unwrap();
        assert_eq!(f.decision, HitWallDecision::Armed);
        assert_eq!(f.destination_direction, Some([1.0, 0.0, 0.0]));
        assert_eq!(f.direction_dot, Some(-1.0));
        assert_eq!(
            f.first_attempt.unwrap().shifted_segment,
            Some([[0.0, 0.0, 1.0], [2.0, 0.0, 1.0]])
        );
        assert_eq!(f.state.uncrouch_time, 0.5);
        assert_eq!(f.state.script.body.position, s.script.body.position);
        assert!(!f.state.crouched);
    }
    #[test]
    fn controller_gates_skip_queries_and_preserve_state() {
        let (mut w, p, mut s, c) = fixture();
        w.world = Err("queries must not run".into());
        let decisions = [
            HitWallDecision::NoWall,
            HitWallDecision::ExcludedWall,
            HitWallDecision::DirectHitWall,
            HitWallDecision::NoController,
            HitWallDecision::Stationary,
            HitWallDecision::AngleFiltered,
            HitWallDecision::NotifyHandled,
            HitWallDecision::NotWalking,
            HitWallDecision::HumanControlled,
            HitWallDecision::CannotCrouch,
            HitWallDecision::AlreadyCrouched,
        ];
        for (case, expected) in decisions.into_iter().enumerate() {
            let mut context = c;
            s.script.body.velocity = [-2.0, 0.0, 0.0];
            s.crouched = false;
            match case {
                0 => context.wall_present = false,
                1 => context.excluded_wall_class = true,
                2 => context.direct_hit_wall = true,
                3 => context.controller_present = false,
                4 => s.script.body.velocity = [0.0; 3],
                5 => context.wall_normal = [1.0, 0.0, 0.0],
                6 => context.notify_handled = true,
                7 => {
                    context.physics = JumpPhysics::Falling;
                    context.minimum_hit_wall = 2.0;
                }
                8 => context.human_controlled = true,
                9 => context.can_crouch = false,
                10 => s.crouched = true,
                _ => unreachable!(),
            }
            let f = w
                .hit_wall_auto_crouch_first_attempt(&s, context, &p)
                .unwrap();
            assert_eq!(f.decision, expected);
            assert!(f.first_attempt.is_none());
            assert!(!f.state.try_to_uncrouch && !f.state.script.wants_to_crouch);
        }
    }
    #[test]
    fn angle_equality_and_zero_destination_are_not_filtered() {
        let (w, p, s, mut c) = fixture();
        c.minimum_hit_wall = -1.0;
        assert_eq!(
            w.hit_wall_auto_crouch_first_attempt(&s, c, &p)
                .unwrap()
                .decision,
            HitWallDecision::Armed
        );
        c.destination = s.script.body.position;
        c.minimum_hit_wall = 0.0;
        let f = w.hit_wall_auto_crouch_first_attempt(&s, c, &p).unwrap();
        assert_eq!(f.direction_dot, Some(0.0));
        assert_eq!(f.decision, HitWallDecision::Armed);
    }
    #[test]
    fn blocked_attempt_requires_recovery_and_unknown_or_invalid_input_is_atomic_error() {
        let (mut w, p, s, mut c) = fixture();
        w.world = Ok(Arc::new(HullSet {
            hulls: vec![Hull {
                offset: 0,
                planes: vec![],
                bounds: [[1.0, -1.0, 0.5], [2.0, 1.0, 1.5]],
            }],
            missing_solid_leaves: 0,
            empty_root_solid: false,
        }));
        let f = w.hit_wall_auto_crouch_first_attempt(&s, c, &p).unwrap();
        assert_eq!(f.decision, HitWallDecision::RecoveryRequired);
        assert!(!f.state.try_to_uncrouch && !f.state.crouched);
        w.world = Err("unknown".into());
        assert!(w.hit_wall_auto_crouch_first_attempt(&s, c, &p).is_err());
        c.destination[0] = f64::NAN;
        assert!(w.hit_wall_auto_crouch_first_attempt(&s, c, &p).is_err());
        assert_eq!(s.uncrouch_time, 0.0);
    }
}
