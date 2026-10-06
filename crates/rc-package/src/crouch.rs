//! Native height compensation with own complete static AABB placement policy.
//! No native FarMoveActor/encroachment callbacks, replication or timing reconstruction.
use crate::{
    defaults::{Catalog, Resolved},
    movement::{Placement, PlacementState},
    physics::BodyState,
    properties::Value,
    world_collision::StaticBodyWorld,
};
use serde::Serialize;
use std::collections::BTreeMap;
#[derive(Debug, Serialize)]
pub struct CrouchProfile {
    pub standing: [f64; 3],
    pub crouching: [f64; 3],
    pub properties: BTreeMap<String, Resolved>,
}
impl CrouchProfile {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self
            .standing
            .iter()
            .chain(&self.crouching)
            .any(|v| !v.is_finite() || *v <= 0.0 || *v > 1e6)
            || self.standing[0] != self.standing[1]
            || self.crouching[0] != self.crouching[1]
            || self.crouching[2] >= self.standing[2]
            || (0..3).any(|i| self.crouching[i] > self.standing[i])
        {
            return Err(
                "unsupported crouch dimensions; diagnostic requires nested smaller shape".into(),
            );
        }
        Ok(())
    }
    pub fn read(catalog: &Catalog) -> Result<Self, String> {
        let mut properties = BTreeMap::new();
        let mut values = Vec::new();
        for name in [
            "CollisionRadius",
            "CollisionHeight",
            "CrouchRadius",
            "CrouchHeight",
        ] {
            let resolved = catalog.resolve("CTCharacters.PlayerCommando", name, 0)?;
            let value = match resolved.value {
                Value::Float(v) if v.is_finite() && v > 0.0 && v <= 1e6 => f64::from(v),
                _ => return Err(format!("invalid crouch dimension {name}")),
            };
            values.push(value);
            properties.insert(name.into(), resolved);
        }
        Ok(Self {
            standing: [values[0], values[0], values[1]],
            crouching: [values[2], values[2], values[3]],
            properties,
        })
    }
}
#[derive(Debug, Clone, Copy, Serialize)]
pub struct CrouchBody {
    pub body: BodyState,
    pub crouched: bool,
}
#[derive(Debug, PartialEq, Eq, Serialize)]
pub enum CrouchChange {
    Unchanged,
    Changed,
    Blocked,
}
#[derive(Debug, Serialize)]
pub struct CrouchFrame {
    pub state: CrouchBody,
    pub extent: [f64; 3],
    pub change: CrouchChange,
    /// Old height minus target height; abs is the native event HeightAdjust magnitude.
    pub height_adjustment: f64,
    pub target_placement: Option<Placement>,
}
impl StaticBodyWorld {
    /// Change shape and compensate center Z without moving the feet.
    /// A blocked target is a successful no-change decision; unknown geometry is Err.
    pub fn crouch_diagnostic(
        &self,
        state: &CrouchBody,
        want_crouch: bool,
        profile: &CrouchProfile,
    ) -> Result<CrouchFrame, String> {
        profile.validate()?;
        let extent = if state.crouched {
            profile.crouching
        } else {
            profile.standing
        };
        let initial = self.body_placement(state.body.position, extent)?;
        if initial.state == PlacementState::Penetrating {
            return Err("initial crouch body penetrates geometry".into());
        }
        if state.crouched == want_crouch {
            return Ok(CrouchFrame {
                state: *state,
                extent,
                change: CrouchChange::Unchanged,
                height_adjustment: 0.0,
                target_placement: None,
            });
        }
        let target = if want_crouch {
            profile.crouching
        } else {
            profile.standing
        };
        let height_adjustment = extent[2] - target[2];
        let mut body = state.body;
        body.position[2] -= height_adjustment;
        let placement = self.body_placement(body.position, target)?;
        if placement.state == PlacementState::Penetrating {
            return Ok(CrouchFrame {
                state: *state,
                extent,
                change: CrouchChange::Blocked,
                height_adjustment,
                target_placement: Some(placement),
            });
        }
        Ok(CrouchFrame {
            state: CrouchBody {
                body,
                crouched: want_crouch,
            },
            extent: target,
            change: CrouchChange::Changed,
            height_adjustment,
            target_placement: Some(placement),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::hull::{Hull, HullSet};
    use std::sync::Arc;
    fn fixture(ceiling: bool) -> (StaticBodyWorld, CrouchProfile, CrouchBody) {
        let mut hulls = vec![Hull {
            offset: 0,
            planes: vec![],
            bounds: [[-100.0, -100.0, -10.0], [100.0, 100.0, 0.0]],
        }];
        if ceiling {
            hulls.push(Hull {
                offset: 1,
                planes: vec![],
                bounds: [[-100.0, -100.0, 3.0], [100.0, 100.0, 4.0]],
            });
        }
        (
            StaticBodyWorld {
                world: Ok(Arc::new(HullSet {
                    hulls,
                    missing_solid_leaves: 0,
                    empty_root_solid: false,
                })),
                actors: vec![],
            },
            CrouchProfile {
                standing: [1.0, 1.0, 2.0],
                crouching: [1.0; 3],
                properties: BTreeMap::new(),
            },
            CrouchBody {
                body: BodyState {
                    position: [0.0, 0.0, 2.0],
                    velocity: [4.0, 0.0, 0.0],
                    grounded: true,
                },
                crouched: false,
            },
        )
    }
    #[test]
    fn round_trip_preserves_feet_velocity_and_grounded_state() {
        let (w, p, s) = fixture(false);
        let down = w.crouch_diagnostic(&s, true, &p).unwrap();
        assert_eq!(down.change, CrouchChange::Changed);
        assert_eq!(down.state.body.position, [0.0, 0.0, 1.0]);
        assert_eq!(down.state.body.velocity, s.body.velocity);
        assert!(down.state.body.grounded);
        let up = w.crouch_diagnostic(&down.state, false, &p).unwrap();
        assert_eq!(up.state.body.position, s.body.position);
        assert!(!up.state.crouched);
        let idle = w.crouch_diagnostic(&up.state, false, &p).unwrap();
        assert_eq!(idle.change, CrouchChange::Unchanged);
    }
    #[test]
    fn low_ceiling_blocks_standing_without_changing_state() {
        let (w, p, mut s) = fixture(true);
        s.crouched = true;
        s.body.position[2] = 1.0;
        let blocked = w.crouch_diagnostic(&s, false, &p).unwrap();
        assert_eq!(blocked.change, CrouchChange::Blocked);
        assert!(blocked.state.crouched);
        assert_eq!(blocked.state.body.position, s.body.position);
        assert_eq!(blocked.extent, p.crouching);
    }
    #[test]
    fn touching_ceiling_is_allowed_and_radius_shrinks_with_height() {
        let (w, mut p, mut s) = fixture(true);
        p.standing[2] = 1.5;
        s.crouched = true;
        s.body.position[2] = 1.0;
        let up = w.crouch_diagnostic(&s, false, &p).unwrap();
        assert_eq!(up.change, CrouchChange::Changed);
        p.crouching = [0.5, 0.5, 1.0];
        assert_eq!(
            w.crouch_diagnostic(&up.state, true, &p).unwrap().extent,
            p.crouching
        );
    }
    #[test]
    fn invalid_initial_shape_dimensions_and_missing_geometry_fail() {
        let (mut w, mut p, mut s) = fixture(false);
        s.body.position[2] = 0.0;
        assert!(w.crouch_diagnostic(&s, true, &p).is_err());
        s.body.position[2] = 2.0;
        p.crouching[2] = 3.0;
        assert!(w.crouch_diagnostic(&s, true, &p).is_err());
        p.crouching[2] = 1.0;
        w.world = Err("missing geometry".into());
        assert!(w.crouch_diagnostic(&s, true, &p).is_err());
        assert!(!s.crouched);
    }
}
