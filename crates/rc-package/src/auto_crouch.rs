//! CanCrouchWalk flag/timer arming with own complete static traces.
//! Caller supplies the segment; processHitWall/controller dispatch is not implemented.
use crate::{
    crouch::CrouchProfile,
    crouch_motion::CrouchingScriptBody,
    world_collision::{StaticBodyWorld, WorldSweep},
};
use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub enum AutoCrouchDecision {
    Disabled,
    PointBlocked,
    ShapeBlocked,
    Armed,
}
#[derive(Debug, Serialize)]
pub struct AutoCrouchFrame {
    pub state: CrouchingScriptBody,
    pub decision: AutoCrouchDecision,
    pub shifted_segment: Option<[[f64; 3]; 2]>,
    pub point_trace: Option<WorldSweep>,
    pub shape_trace: Option<WorldSweep>,
}
impl StaticBodyWorld {
    /// Native ordering and flags, own static AABB queries (not trace masks 0x286/0x86).
    /// Arming does not move the body or set IsCrouched. Unknown queries are atomic errors.
    pub fn can_crouch_walk_diagnostic(
        &self,
        state: &CrouchingScriptBody,
        start: [f64; 3],
        end: [f64; 3],
        can_crouch: bool,
        profile: &CrouchProfile,
    ) -> Result<AutoCrouchFrame, String> {
        let mut frame = AutoCrouchFrame {
            state: *state,
            decision: AutoCrouchDecision::Disabled,
            shifted_segment: None,
            point_trace: None,
            shape_trace: None,
        };
        if !can_crouch {
            return Ok(frame);
        }
        profile.validate()?;
        if !state.uncrouch_time.is_finite() {
            return Err("invalid uncrouch timer snapshot".into());
        }
        let current_height = if state.crouched {
            profile.crouching[2]
        } else {
            profile.standing[2]
        };
        let shift = profile.crouching[2] - current_height;
        let mut start = start;
        let mut end = end;
        start[2] += shift;
        end[2] += shift;
        frame.shifted_segment = Some([start, end]);
        let point = self.sweep(start, end, [0.0; 3])?;
        if !point.complete {
            return Err("auto-crouch point trace incomplete".into());
        }
        let point_blocked = !point.contacts.is_empty();
        frame.point_trace = Some(point);
        if point_blocked {
            frame.decision = AutoCrouchDecision::PointBlocked;
            return Ok(frame);
        }
        let shape = self.sweep(start, end, profile.crouching)?;
        if !shape.complete {
            return Err("auto-crouch shape trace incomplete".into());
        }
        // Native tests Hit.Time == 1, permitting an endpoint-only contact.
        let blocked = shape.contacts.iter().any(|c| c.hit.fraction != 1.0);
        frame.shape_trace = Some(shape);
        if blocked {
            frame.decision = AutoCrouchDecision::ShapeBlocked;
            return Ok(frame);
        }
        frame.state.script.wants_to_crouch = true;
        frame.state.try_to_uncrouch = true;
        frame.state.uncrouch_time = 0.5;
        frame.decision = AutoCrouchDecision::Armed;
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
        world_collision::{BodyActor, BodyShape},
    };
    use std::sync::Arc;
    fn fixture(
        bounds: Option<[[f64; 3]; 2]>,
    ) -> (StaticBodyWorld, CrouchProfile, CrouchingScriptBody) {
        let world = StaticBodyWorld {
            world: Ok(Arc::new(HullSet {
                hulls: bounds
                    .into_iter()
                    .map(|bounds| Hull {
                        offset: 0,
                        planes: vec![],
                        bounds,
                    })
                    .collect(),
                missing_solid_leaves: 0,
                empty_root_solid: false,
            })),
            actors: vec![],
        };
        let profile = CrouchProfile {
            standing: [1.0, 1.0, 2.0],
            crouching: [1.0; 3],
            properties: Default::default(),
        };
        let state = CrouchingScriptBody {
            script: ScriptBody {
                body: BodyState {
                    position: [0.0, 0.0, 2.0],
                    velocity: [3.0, 0.0, 0.0],
                    grounded: true,
                },
                pressed_jump: true,
                wants_to_crouch: false,
            },
            crouched: false,
            try_to_uncrouch: false,
            uncrouch_time: 0.0,
        };
        (world, profile, state)
    }
    #[test]
    fn clear_segment_arms_without_moving_or_crouching_and_resets_timer() {
        let (w, p, s) = fixture(None);
        let f = w
            .can_crouch_walk_diagnostic(&s, [0.0, 0.0, 2.0], [4.0, 0.0, 2.0], true, &p)
            .unwrap();
        assert_eq!(f.decision, AutoCrouchDecision::Armed);
        assert_eq!(f.shifted_segment, Some([[0.0, 0.0, 1.0], [4.0, 0.0, 1.0]]));
        assert_eq!(f.state.script.body.position, s.script.body.position);
        assert_eq!(f.state.script.body.velocity, s.script.body.velocity);
        assert!(f.state.script.body.grounded && f.state.script.pressed_jump);
        assert!(!f.state.crouched);
        assert!(f.state.script.wants_to_crouch && f.state.try_to_uncrouch);
        assert_eq!(f.state.uncrouch_time, 0.5);
        let mut c = f.state;
        c.crouched = true;
        c.uncrouch_time = -0.1;
        let again = w
            .can_crouch_walk_diagnostic(&c, [0.0, 0.0, 1.0], [4.0, 0.0, 1.0], true, &p)
            .unwrap();
        assert_eq!(
            again.shifted_segment,
            Some([[0.0, 0.0, 1.0], [4.0, 0.0, 1.0]])
        );
        assert_eq!(again.state.uncrouch_time, 0.5);
    }
    #[test]
    fn point_and_shape_failures_do_not_arm_flags() {
        for (bounds, decision) in [
            (
                [[2.0, -1.0, 0.5], [3.0, 1.0, 1.5]],
                AutoCrouchDecision::PointBlocked,
            ),
            (
                [[2.0, 0.5, 0.5], [3.0, 1.5, 1.5]],
                AutoCrouchDecision::ShapeBlocked,
            ),
        ] {
            let (w, p, s) = fixture(Some(bounds));
            let f = w
                .can_crouch_walk_diagnostic(&s, [0.0, 0.0, 2.0], [4.0, 0.0, 2.0], true, &p)
                .unwrap();
            assert_eq!(f.decision, decision);
            assert!(!f.state.script.wants_to_crouch && !f.state.try_to_uncrouch);
            assert_eq!(f.state.uncrouch_time, 0.0);
        }
    }
    #[test]
    fn endpoint_shape_contact_is_allowed_but_earlier_contact_is_not() {
        let (w, p, s) = fixture(Some([[5.0, 0.5, 0.5], [6.0, 1.5, 1.5]]));
        let f = w
            .can_crouch_walk_diagnostic(&s, [0.0, 0.0, 2.0], [4.0, 0.0, 2.0], true, &p)
            .unwrap();
        assert_eq!(
            f.shape_trace.as_ref().unwrap().contacts[0].hit.fraction,
            1.0
        );
        assert_eq!(f.decision, AutoCrouchDecision::Armed);
    }
    #[test]
    fn disabled_skips_queries_and_unknown_geometry_never_arms() {
        let (mut w, p, s) = fixture(None);
        w.world = Err("missing world".into());
        let f = w
            .can_crouch_walk_diagnostic(&s, [f64::NAN; 3], [0.0; 3], false, &p)
            .unwrap();
        assert_eq!(f.decision, AutoCrouchDecision::Disabled);
        assert!(f.point_trace.is_none());
        assert!(w
            .can_crouch_walk_diagnostic(&s, [0.0, 0.0, 2.0], [4.0, 0.0, 2.0], true, &p)
            .is_err());
        w = fixture(None).0;
        w.actors.push(BodyActor {
            name: "unknown".into(),
            shape: BodyShape::Unsupported("dynamic".into()),
        });
        assert!(w
            .can_crouch_walk_diagnostic(&s, [0.0, 0.0, 2.0], [4.0, 0.0, 2.0], true, &p)
            .is_err());
        assert!(w
            .can_crouch_walk_diagnostic(&s, [f64::NAN; 3], [4.0, 0.0, 2.0], true, &p)
            .is_err());
        assert!(!s.try_to_uncrouch && s.uncrouch_time == 0.0);
    }
}
