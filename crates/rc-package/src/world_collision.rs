//! Static world diagnostic, not an engine trace or pawn movement solver.
use crate::{collision::SolidBsp, mesh_query::ActorTransform};
use serde::Serialize;
use std::sync::Arc;

pub enum StaticShape {
    Clear,
    Simple {
        solid: Arc<SolidBsp>,
        transform: ActorTransform,
    },
    Unsupported(String),
}
pub struct StaticActor {
    pub name: String,
    pub shape: StaticShape,
}
pub struct StaticWorld {
    pub world: Result<SolidBsp, String>,
    /// Only actors selected by the caller's player-blocking filter.
    pub actors: Vec<StaticActor>,
}
#[derive(Debug, PartialEq, Eq, Serialize)]
pub enum LineState {
    Clear,
    Blocked,
    Indeterminate,
}
#[derive(Debug, Serialize)]
pub struct QueryError {
    pub object: String,
    pub reason: String,
}
#[derive(Debug, Serialize)]
pub struct WorldLine {
    pub state: LineState,
    /// Completeness within Level-referenced world BSP + the supplied static actor set only.
    pub complete: bool,
    pub blocked_by: Vec<String>,
    pub errors: Vec<QueryError>,
}
pub enum BodyShape {
    Clear,
    Hulls(Arc<crate::hull::HullSet>),
    Unsupported(String),
}
pub struct BodyActor {
    pub name: String,
    pub shape: BodyShape,
}
pub struct StaticBodyWorld {
    pub world: Result<Arc<crate::hull::HullSet>, String>,
    pub actors: Vec<BodyActor>,
}
#[derive(Debug, Serialize)]
pub struct BodyContact {
    pub object: String,
    pub hit: crate::hull::SweepHit,
}
#[derive(Debug, Serialize)]
pub struct WorldSweep {
    pub state: LineState,
    pub complete: bool,
    /// Sorted by entry fraction; these are mathematical contacts, no native backoff.
    pub contacts: Vec<BodyContact>,
    pub errors: Vec<QueryError>,
}
impl StaticBodyWorld {
    pub fn sweep(
        &self,
        start: [f64; 3],
        end: [f64; 3],
        extent: [f64; 3],
    ) -> Result<WorldSweep, String> {
        self.sweep_with_policy(start, end, extent, false)
    }
    pub fn sweep_motion(
        &self,
        start: [f64; 3],
        end: [f64; 3],
        extent: [f64; 3],
    ) -> Result<WorldSweep, String> {
        self.sweep_with_policy(start, end, extent, true)
    }
    fn sweep_with_policy(
        &self,
        start: [f64; 3],
        end: [f64; 3],
        extent: [f64; 3],
        motion: bool,
    ) -> Result<WorldSweep, String> {
        if start
            .into_iter()
            .chain(end)
            .chain(extent)
            .any(|v| !v.is_finite() || v.abs() > 1e9)
            || extent.iter().any(|v| *v < 0.0)
        {
            return Err("invalid world body sweep".into());
        }
        let mut result = WorldSweep {
            state: LineState::Clear,
            complete: true,
            contacts: vec![],
            errors: vec![],
        };
        let mut record =
            |name: &str, query: Result<Option<crate::hull::SweepHit>, String>| match query {
                Ok(Some(hit)) => result.contacts.push(BodyContact {
                    object: name.into(),
                    hit,
                }),
                Ok(None) => {}
                Err(reason) => result.errors.push(QueryError {
                    object: name.into(),
                    reason,
                }),
            };
        record(
            "World.BSP",
            self.world.as_ref().map_err(Clone::clone).and_then(|h| {
                if motion {
                    h.sweep_motion(start, end, extent)
                } else {
                    h.sweep(start, end, extent)
                }
            }),
        );
        for actor in &self.actors {
            record(
                &actor.name,
                match &actor.shape {
                    BodyShape::Clear => Ok(None),
                    BodyShape::Hulls(h) => {
                        if motion {
                            h.sweep_motion(start, end, extent)
                        } else {
                            h.sweep(start, end, extent)
                        }
                    }
                    BodyShape::Unsupported(reason) => Err(reason.clone()),
                },
            );
        }
        result
            .contacts
            .sort_by(|a, b| a.hit.fraction.total_cmp(&b.hit.fraction));
        result.complete = result.errors.is_empty();
        result.state = if !result.contacts.is_empty() {
            LineState::Blocked
        } else if result.complete {
            LineState::Clear
        } else {
            LineState::Indeterminate
        };
        Ok(result)
    }
}
impl StaticWorld {
    pub fn line(&self, start: [f32; 3], end: [f32; 3]) -> Result<WorldLine, String> {
        if start
            .into_iter()
            .chain(end)
            .any(|v| !v.is_finite() || v.abs() > 1e9)
        {
            return Err("invalid world line endpoint".into());
        }
        let mut result = WorldLine {
            state: LineState::Clear,
            complete: true,
            blocked_by: vec![],
            errors: vec![],
        };
        let mut record = |name: &str, probe: Result<bool, String>| match probe {
            Ok(false) => result.blocked_by.push(name.into()),
            Ok(true) => {}
            Err(reason) => result.errors.push(QueryError {
                object: name.into(),
                reason,
            }),
        };
        record(
            "World.BSP",
            self.world
                .as_ref()
                .map_err(Clone::clone)
                .and_then(|b| b.line_clear(start, end)),
        );
        // Do not short-circuit: a known block must not hide missing geometry.
        for actor in &self.actors {
            let probe = match &actor.shape {
                StaticShape::Clear => Ok(true),
                StaticShape::Unsupported(reason) => Err(reason.clone()),
                StaticShape::Simple { solid, transform } => {
                    transform.local_point(start).and_then(|a| {
                        transform
                            .local_point(end)
                            .and_then(|b| solid.line_clear(a, b))
                    })
                }
            };
            record(&actor.name, probe);
        }
        result.complete = result.errors.is_empty();
        result.state = if !result.blocked_by.is_empty() {
            LineState::Blocked
        } else if result.complete {
            LineState::Clear
        } else {
            LineState::Indeterminate
        };
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::SolidNode;
    #[test]
    fn body_contacts_are_sorted_and_do_not_hide_missing_shapes() {
        let shape = |offset, x: f64| {
            Arc::new(crate::hull::HullSet {
                hulls: vec![crate::hull::Hull {
                    offset,
                    planes: vec![],
                    bounds: [[x, -1.0, -1.0], [x + 1.0, 1.0, 1.0]],
                }],
                missing_solid_leaves: 0,
                empty_root_solid: false,
            })
        };
        let w = StaticBodyWorld {
            world: Ok(shape(0, 8.0)),
            actors: vec![
                BodyActor {
                    name: "near mesh".into(),
                    shape: BodyShape::Hulls(shape(1, 3.0)),
                },
                BodyActor {
                    name: "missing".into(),
                    shape: BodyShape::Unsupported("no shape".into()),
                },
            ],
        };
        let r = w.sweep([0.0; 3], [10.0, 0.0, 0.0], [0.5; 3]).unwrap();
        assert_eq!(r.state, LineState::Blocked);
        assert!(!r.complete);
        assert_eq!(r.contacts[0].object, "near mesh");
        assert!((r.contacts[0].hit.fraction - 0.25).abs() < 1e-10);
        assert_eq!(r.errors.len(), 1);
        assert!(w.sweep([0.0; 3], [1.0; 3], [-1.0; 3]).is_err());
    }
    fn halfspace() -> SolidBsp {
        SolidBsp {
            nodes: vec![SolidNode {
                plane: [1.0, 0.0, 0.0, 0.0],
                front: -1,
                back: -1,
                flags: 0,
            }],
            root_outside: true,
            linked: false,
            zones: 0,
            bounds: 0,
            hull_indices: 0,
            hull_words: vec![],
            leaves: 0,
            lights: 0,
            decoded_bytes: 0,
            remaining_bytes: 0,
        }
    }
    #[test]
    fn world_block_does_not_hide_actor_errors_and_unknown_is_never_clear() {
        let mut world = StaticWorld {
            world: Ok(halfspace()),
            actors: vec![StaticActor {
                name: "missing".into(),
                shape: StaticShape::Unsupported("unsupported Model".into()),
            }],
        };
        let hit = world.line([1.0, 0.0, 0.0], [-1.0, 0.0, 0.0]).unwrap();
        assert_eq!(hit.state, LineState::Blocked);
        assert!(!hit.complete);
        assert_eq!(hit.errors.len(), 1);
        assert_eq!(
            world.line([1.0, 0.0, 0.0], [2.0, 0.0, 0.0]).unwrap().state,
            LineState::Indeterminate
        );
        world.actors.clear();
        assert_eq!(
            world.line([1.0, 0.0, 0.0], [2.0, 0.0, 0.0]).unwrap().state,
            LineState::Clear
        );
        world.world = Err("no world BSP".into());
        assert_eq!(
            world.line([1.0; 3], [2.0; 3]).unwrap().state,
            LineState::Indeterminate
        );
        assert!(world.line([f32::NAN; 3], [0.0; 3]).is_err());
    }
    #[test]
    fn transformed_actor_can_block_a_world_clear_segment_in_either_direction() {
        let mut empty = halfspace();
        empty.nodes.clear();
        let world = StaticWorld {
            world: Ok(empty),
            actors: vec![StaticActor {
                name: "mesh".into(),
                shape: StaticShape::Simple {
                    solid: Arc::new(halfspace()),
                    transform: ActorTransform {
                        location: [10.0, 0.0, 0.0],
                        rotation: [0; 3],
                        scale: [-2.0, 1.0, 1.0],
                        pivot: [0.0; 3],
                    },
                },
            }],
        };
        for (a, b) in [(9.0, 11.0), (11.0, 9.0)] {
            let result = world.line([a, 0.0, 0.0], [b, 0.0, 0.0]).unwrap();
            assert_eq!(result.state, LineState::Blocked);
            assert!(result.complete);
            assert_eq!(result.blocked_by, ["mesh"]);
        }
        assert_eq!(
            world.line([9.0, 0.0, 0.0], [8.0, 0.0, 0.0]).unwrap().state,
            LineState::Clear
        );
    }
}
