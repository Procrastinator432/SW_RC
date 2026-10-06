//! Conservative PC displacement diagnostic; not reconstructed Pawn physics.
use crate::world_collision::{BodyContact, StaticBodyWorld};
use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub enum MoveState {
    Finished,
    Stopped,
    InitialContact,
    IterationLimit,
}
#[derive(Debug, Serialize)]
pub struct Movement {
    pub position: [f64; 3],
    pub state: MoveState,
    pub contacts: Vec<BodyContact>,
    pub iterations: usize,
}
#[derive(Debug, PartialEq, Eq, Serialize)]
pub enum PlacementState {
    Free,
    Touching,
    Penetrating,
}
#[derive(Debug, Serialize)]
pub struct Placement {
    pub position: [f64; 3],
    pub state: PlacementState,
    pub contacts: Vec<BodyContact>,
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] + b[i])
}
fn complete(query: &crate::world_collision::WorldSweep) -> Result<(), String> {
    if query.complete {
        Ok(())
    } else {
        Err(format!("movement query incomplete: {:?}", query.errors))
    }
}
impl StaticBodyWorld {
    /// Classify a proposed position without relocating it or selecting a game spawn.
    pub fn body_placement(
        &self,
        position: [f64; 3],
        extent: [f64; 3],
    ) -> Result<Placement, String> {
        let contact = self.sweep(position, position, extent)?;
        complete(&contact)?;
        let interior = self.sweep_motion(position, position, extent)?;
        complete(&interior)?;
        Ok(Placement {
            position,
            state: if !interior.contacts.is_empty() {
                PlacementState::Penetrating
            } else if !contact.contacts.is_empty() {
                PlacementState::Touching
            } else {
                PlacementState::Free
            },
            contacts: contact.contacts,
        })
    }
    /// Return a candidate only after complete queries. Any error discards all progress.
    /// Allows touching starts; strictly penetrating starts require placement handling.
    pub fn slide_body(
        &self,
        start: [f64; 3],
        displacement: [f64; 3],
        extent: [f64; 3],
        skin: f64,
        max_iterations: usize,
    ) -> Result<Movement, String> {
        if !skin.is_finite()
            || skin <= 0.0
            || skin > 100.0
            || !(1..=32).contains(&max_iterations)
            || displacement.iter().any(|v| !v.is_finite() || v.abs() > 1e9)
        {
            return Err("invalid movement options".into());
        }
        let initial = self.sweep_motion(start, start, extent)?;
        complete(&initial)?;
        // Validate the requested endpoint even when an initial contact prevents motion.
        let target = add(start, displacement);
        if target.iter().any(|v| !v.is_finite() || v.abs() > 1e9) {
            return Err("invalid movement endpoint".into());
        }
        let mut result = Movement {
            position: start,
            state: MoveState::Finished,
            contacts: vec![],
            iterations: 0,
        };
        if !initial.contacts.is_empty() {
            result.state = MoveState::InitialContact;
            result.contacts = initial.contacts;
            return Ok(result);
        }
        let mut remaining = displacement;
        let mut normals = Vec::new();
        for iteration in 0..max_iterations {
            if dot(remaining, remaining) < 1e-16 {
                return Ok(result);
            }
            result.iterations = iteration + 1;
            let query =
                self.sweep_motion(result.position, add(result.position, remaining), extent)?;
            complete(&query)?;
            let Some(contact) = query.contacts.into_iter().next() else {
                result.position = add(result.position, remaining);
                return Ok(result);
            };
            if contact.hit.start_overlapping || dot(contact.hit.normal, contact.hit.normal) < 0.5 {
                return Err("unexpected overlapping intermediate movement position".into());
            }
            let n = contact.hit.normal;
            let approach = -dot(remaining, n);
            if approach <= 0.0 {
                return Err("invalid movement contact direction".into());
            }
            let fraction = (contact.hit.fraction - skin / approach).max(0.0);
            result.position = add(result.position, remaining.map(|v| v * fraction));
            remaining = remaining.map(|v| v * (1.0 - fraction));
            normals.push(n);
            result.contacts.push(contact);
            // Reapply earlier planes so a second wall cannot push the body into the first.
            for _ in 0..8 {
                for &plane in &normals {
                    let inward = dot(remaining, plane).min(0.0);
                    remaining = std::array::from_fn(|i| remaining[i] - plane[i] * inward);
                }
            }
            if normals.iter().any(|&n| dot(remaining, n) < -1e-8)
                || dot(remaining, remaining) < 1e-16
            {
                result.state = MoveState::Stopped;
                return Ok(result);
            }
        }
        result.state = MoveState::IterationLimit;
        Ok(result)
    }

    /// Downward support diagnostic. Does not snap, apply gravity or select a spawn.
    pub fn floor_contact(
        &self,
        position: [f64; 3],
        extent: [f64; 3],
        distance: f64,
        minimum_up: f64,
    ) -> Result<Option<BodyContact>, String> {
        if !distance.is_finite()
            || distance <= 0.0
            || distance > 1e9
            || !minimum_up.is_finite()
            || !(0.0..=1.0).contains(&minimum_up)
        {
            return Err("invalid floor probe options".into());
        }
        let query = self.sweep_motion(position, add(position, [0.0, 0.0, -distance]), extent)?;
        complete(&query)?;
        let contact = query.contacts.into_iter().next();
        if contact.as_ref().is_some_and(|c| c.hit.start_overlapping) {
            return Err("floor probe starts penetrating geometry".into());
        }
        Ok(contact.filter(|c| c.hit.normal[2] >= minimum_up))
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
    fn world(bounds: Vec<[[f64; 3]; 2]>) -> StaticBodyWorld {
        StaticBodyWorld {
            world: Ok(Arc::new(HullSet {
                hulls: bounds
                    .into_iter()
                    .enumerate()
                    .map(|(offset, bounds)| Hull {
                        offset,
                        planes: vec![],
                        bounds,
                    })
                    .collect(),
                missing_solid_leaves: 0,
                empty_root_solid: false,
            })),
            actors: vec![],
        }
    }
    #[test]
    fn wall_sliding_and_fast_thin_wall_do_not_tunnel() {
        let w = world(vec![[[0.0, -100.0, -100.0], [0.1, 100.0, 100.0]]]);
        let r = w
            .slide_body([-5.0, 0.0, 0.0], [20.0, 10.0, 0.0], [1.0; 3], 0.1, 8)
            .unwrap();
        assert_eq!(r.state, MoveState::Finished);
        assert!((r.position[0] + 1.1).abs() < 1e-10);
        assert!((r.position[1] - 10.0).abs() < 1e-10);
        assert!(w
            .sweep(r.position, r.position, [1.0; 3])
            .unwrap()
            .contacts
            .is_empty());
        let away = w
            .slide_body(r.position, [-10.0, 0.0, 0.0], [1.0; 3], 0.1, 8)
            .unwrap();
        assert!(away.position[0] < -11.0);
        assert_eq!(
            w.slide_body([-1.0, 0.0, 0.0], [-10.0, 0.0, 0.0], [1.0; 3], 0.1, 8)
                .unwrap()
                .state,
            MoveState::Finished
        );
    }
    #[test]
    fn corner_contacts_and_iteration_budget_are_explicit() {
        let w = world(vec![
            [[0.0, -100.0, -100.0], [1.0, 100.0, 100.0]],
            [[-100.0, 0.0, -100.0], [100.0, 1.0, 100.0]],
        ]);
        let r = w
            .slide_body([-5.0, -6.0, 0.0], [10.0, 10.0, 0.0], [1.0; 3], 0.1, 8)
            .unwrap();
        assert_eq!(r.state, MoveState::Stopped);
        assert_eq!(r.contacts.len(), 2);
        assert!(r.position[0] <= -1.1 + 1e-10 && r.position[1] <= -1.1 + 1e-10);
        assert_eq!(
            w.slide_body([-5.0, -6.0, 0.0], [10.0, 10.0, 0.0], [1.0; 3], 0.1, 1)
                .unwrap()
                .state,
            MoveState::IterationLimit
        );
    }
    #[test]
    fn floor_overlap_unknown_geometry_and_invalid_options() {
        let mut w = world(vec![[[-100.0, -100.0, -10.0], [100.0, 100.0, 0.0]]]);
        let hit = w
            .floor_contact([0.0, 0.0, 5.0], [1.0; 3], 10.0, 0.7)
            .unwrap()
            .unwrap();
        assert!((hit.hit.fraction - 0.4).abs() < 1e-10);
        assert_eq!(hit.hit.normal, [0.0, 0.0, 1.0]);
        assert!(w.floor_contact([0.0; 3], [1.0; 3], 10.0, 0.7).is_err());
        assert!(w
            .floor_contact([0.0, 0.0, 50.0], [1.0; 3], 10.0, 0.7)
            .unwrap()
            .is_none());
        w.actors.push(BodyActor {
            name: "unknown".into(),
            shape: BodyShape::Unsupported("missing".into()),
        });
        assert!(w
            .slide_body([0.0, 0.0, 5.0], [10.0, 0.0, 0.0], [1.0; 3], 0.1, 8)
            .is_err());
        assert!(w
            .floor_contact([0.0, 0.0, 5.0], [1.0; 3], 10.0, 0.7)
            .is_err());
        assert!(w.slide_body([0.0; 3], [0.0; 3], [1.0; 3], 0.0, 8).is_err());
    }
    #[test]
    fn floor_touch_allows_tangent_departure_and_blocks_inward_motion() {
        let w = world(vec![[[-100.0, -100.0, -10.0], [100.0, 100.0, 0.0]]]);
        let p = [0.0, 0.0, 1.0];
        assert_eq!(
            w.body_placement(p, [1.0; 3]).unwrap().state,
            PlacementState::Touching
        );
        assert_eq!(
            w.body_placement([0.0; 3], [1.0; 3]).unwrap().state,
            PlacementState::Penetrating
        );
        for delta in [[10.0, 0.0, 0.0], [0.0, 0.0, 10.0]] {
            let r = w.slide_body(p, delta, [1.0; 3], 0.1, 8).unwrap();
            assert_eq!(r.state, MoveState::Finished);
            assert_eq!(r.position, add(p, delta));
        }
        let r = w
            .slide_body(p, [10.0, 0.0, -5.0], [1.0; 3], 0.1, 8)
            .unwrap();
        assert_eq!(r.position, [10.0, 0.0, 1.0]);
        let floor = w.floor_contact(p, [1.0; 3], 10.0, 0.7).unwrap().unwrap();
        assert_eq!(floor.hit.fraction, 0.0);
        assert_eq!(floor.hit.normal, [0.0, 0.0, 1.0]);
    }
    #[test]
    fn tangent_hull_does_not_hide_later_wall_or_penetrating_hull() {
        let mut w = world(vec![
            [[-100.0, -100.0, -10.0], [100.0, 100.0, 0.0]],
            [[5.0, -100.0, -100.0], [6.0, 100.0, 100.0]],
        ]);
        let r = w
            .slide_body([0.0, 0.0, 1.0], [20.0, 0.0, 0.0], [1.0; 3], 0.1, 8)
            .unwrap();
        assert_eq!(r.state, MoveState::Stopped);
        assert!((r.position[0] - 3.9).abs() < 1e-10);
        w.actors.push(BodyActor {
            name: "overlapping".into(),
            shape: BodyShape::Hulls(Arc::new(HullSet {
                hulls: vec![Hull {
                    offset: 0,
                    planes: vec![],
                    bounds: [[-2.0; 3], [2.0; 3]],
                }],
                missing_solid_leaves: 0,
                empty_root_solid: false,
            })),
        });
        assert_eq!(
            w.body_placement([0.0, 0.0, 1.0], [1.0; 3]).unwrap().state,
            PlacementState::Penetrating
        );
        assert_eq!(
            w.slide_body([0.0, 0.0, 1.0], [20.0, 0.0, 0.0], [1.0; 3], 0.1, 8)
                .unwrap()
                .state,
            MoveState::InitialContact
        );
        w.actors.push(BodyActor {
            name: "unknown".into(),
            shape: BodyShape::Unsupported("missing".into()),
        });
        assert!(w.body_placement([0.0, 0.0, 1.0], [1.0; 3]).is_err());
    }
    #[test]
    fn inclined_touch_and_convex_corner_departure_keep_closed_sweep_semantics() {
        let diagonal = 1.0 / 2.0f64.sqrt();
        let w = StaticBodyWorld {
            world: Ok(Arc::new(HullSet {
                hulls: vec![Hull {
                    offset: 0,
                    planes: vec![[diagonal, 0.0, diagonal, 0.0]],
                    bounds: [[-100.0; 3], [100.0; 3]],
                }],
                missing_solid_leaves: 0,
                empty_root_solid: false,
            })),
            actors: vec![],
        };
        let position = [0.0, 0.0, 2.0];
        assert_eq!(
            w.body_placement(position, [1.0; 3]).unwrap().state,
            PlacementState::Touching
        );
        let r = w
            .slide_body(position, [10.0, 0.0, -10.0], [1.0; 3], 0.1, 8)
            .unwrap();
        assert_eq!(r.position, [10.0, 0.0, -8.0]);
        assert!(w
            .floor_contact(position, [1.0; 3], 10.0, 0.8)
            .unwrap()
            .is_none());
        assert!(w
            .floor_contact(position, [1.0; 3], 10.0, 0.7)
            .unwrap()
            .is_some());
        let h = Hull {
            offset: 0,
            planes: vec![],
            bounds: [[-1.0; 3], [1.0; 3]],
        };
        let a = [-2.0, -2.0, 0.0];
        let b = [-1.0, -3.0, 0.0];
        assert!(h.sweep(a, b, [1.0; 3]).unwrap().is_some());
        assert!(h.sweep_motion(a, b, [1.0; 3]).unwrap().is_none());
    }
}
