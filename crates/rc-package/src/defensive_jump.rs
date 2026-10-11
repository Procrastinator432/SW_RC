//! Prepared ctgame.dll FindNearestNavNode / DoDefensiveJump planning.
//! Linked-list order, flags, class membership, anchors and reachability are host
//! snapshots. This module does not execute the returned goals or flight physics.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Node {
    pub id: u32,
    pub position: [f32; 3],
    pub enabled: bool,
    pub available: bool,
    pub same_zone: bool,
    pub class_matches: bool,
    pub reachable: bool,
    pub clear: bool,
}
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Query {
    pub origin: [f32; 3],
    pub range: f32,
    pub anchor_present: bool,
    pub require_reachable: bool,
    pub require_same_zone: bool,
    pub require_clear: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum Call {
    Anchor,
    Available(u32),
    Reachable(u32),
    Clear(u32),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Search {
    pub selected: Option<u32>,
    pub calls: Vec<Call>,
}
/// Native 10012e00: call availability before zone/class/distance filtering.
/// Equal distances keep the first eligible node in the supplied native list.
pub fn nearest(nodes: &[Node], q: Query) -> Search {
    let mut result = Search {
        selected: None,
        calls: vec![],
    };
    if q.require_reachable {
        result.calls.push(Call::Anchor);
        if !q.anchor_present {
            return result;
        }
    }
    let range_squared = q.range * q.range;
    let mut best = f32::from_bits(0x60ad_78ec); // 1004dd98: 1e20, not infinity.
    for n in nodes {
        if !n.enabled {
            continue;
        }
        result.calls.push(Call::Available(n.id));
        if !n.available || (q.require_same_zone && !n.same_zone) || !n.class_matches {
            continue;
        }
        let [x, y, z] = std::array::from_fn(|i| n.position[i] - q.origin[i]);
        let d = (y * y + x * x) + z * z; // SSE reduction order 10012f10..10012f26.
        if !(d < range_squared && d < best) {
            continue;
        }
        if q.require_reachable {
            result.calls.push(Call::Reachable(n.id));
            if !n.reachable {
                continue;
            }
        }
        if q.require_clear {
            result.calls.push(Call::Clear(n.id));
            if !n.clear {
                continue;
            }
        }
        best = d;
        result.selected = Some(n.id);
    }
    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum Kind {
    Perch,
    JumpDest,
}
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Target {
    pub id: u32,
    pub position: [f32; 3],
}
/// 10039a80..10039b33: (z*z+y*y)+x*x; JBE picks JumpDest on ties/unordered.
pub fn select(
    origin: [f32; 3],
    perch: Option<Target>,
    jump: Option<Target>,
) -> Option<(Kind, Target)> {
    let squared = |t: Target| {
        let [x, y, z] = std::array::from_fn(|i| origin[i] - t.position[i]);
        (z * z + y * y) + x * x
    };
    match (perch, jump) {
        (None, None) => None,
        (Some(p), None) => Some((Kind::Perch, p)),
        (None, Some(j)) => Some((Kind::JumpDest, j)),
        (Some(p), Some(j)) => {
            if squared(j).partial_cmp(&squared(p)) == Some(std::cmp::Ordering::Greater) {
                Some((Kind::Perch, p))
            } else {
                Some((Kind::JumpDest, j))
            }
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum Subject {
    Node(u32),
    Enemy,
}
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum Goal {
    ChangeAimTurn(Subject),
    Goto { node: u32, tolerance: f32 },
    JumpTo(u32),
    TurnToEnemy,
    Sleep(f32),
    FollowEnemy { distance: f32, mode: i32 },
}
pub trait Host {
    /// Original arguments: range=10000, require_clear=false, reachable=true,
    /// same_zone=true. Class membership is Perch (including subclasses) / JumpDest.
    fn find_nearest(&mut self, kind: Kind) -> Option<Target>;
    fn reserve(&mut self, node: u32);
    /// IsClearTo flags here are true,true (nearest's optional clear uses true,false).
    fn clear_to(&mut self, node: u32) -> bool;
    fn reachable_to_enemy(&mut self, node: u32) -> bool;
    fn own_anchor(&mut self) -> Option<u32>;
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Flags {
    pub must_jump: bool,
    pub is_melee_attacking: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Plan {
    pub selected: Option<(Kind, u32)>,
    pub goals: Vec<Goal>,
    pub clear_strafe: bool,
}
/// Native 100399a0. State flag is cleared even when both queries return null;
/// IsMeleeAttacking and bot strafe clear only after a destination was selected.
pub fn plan(flags: &mut Flags, origin: [f32; 3], host: &mut impl Host) -> Plan {
    flags.must_jump = false;
    let p = host.find_nearest(Kind::Perch);
    let j = host.find_nearest(Kind::JumpDest);
    let mut out = Plan {
        selected: None,
        goals: vec![],
        clear_strafe: false,
    };
    let Some((kind, target)) = select(origin, p, j) else {
        return out;
    };
    let node = target.id;
    out.selected = Some((kind, node));
    host.reserve(node);
    out.goals.push(Goal::ChangeAimTurn(Subject::Node(node)));
    out.goals.push(if host.clear_to(node) {
        Goal::JumpTo(node)
    } else {
        Goal::Goto {
            node,
            tolerance: 0.,
        }
    });
    if kind == Kind::JumpDest {
        out.goals.push(Goal::ChangeAimTurn(Subject::Enemy));
        out.goals.push(Goal::TurnToEnemy);
    }
    out.goals.push(Goal::Sleep(0.5));
    if host.reachable_to_enemy(node) {
        out.goals.push(Goal::ChangeAimTurn(Subject::Enemy));
        out.goals.push(Goal::FollowEnemy {
            distance: 500.,
            mode: 1,
        });
    } else if host.own_anchor().is_some() {
        // Native calls GetAnchor again for ChangeAimTurn and Goto. Preserve
        // those observations: they can differ on an unstable supplied host.
        out.goals.push(Goal::ChangeAimTurn(Subject::Node(
            host.own_anchor().unwrap_or(0),
        )));
        out.goals.push(Goal::Goto {
            node: host.own_anchor().unwrap_or(0),
            tolerance: 0.,
        });
        out.goals.push(Goal::ChangeAimTurn(Subject::Enemy));
        out.goals.push(Goal::TurnToEnemy);
    }
    flags.is_melee_attacking = false;
    out.clear_strafe = true;
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn node(id: u32, x: f32) -> Node {
        Node {
            id,
            position: [x, 0., 0.],
            enabled: true,
            available: true,
            same_zone: true,
            class_matches: true,
            reachable: true,
            clear: true,
        }
    }
    fn query() -> Query {
        Query {
            origin: [0.; 3],
            range: 10000.,
            anchor_present: true,
            require_reachable: true,
            require_same_zone: true,
            require_clear: false,
        }
    }
    #[test]
    fn strict_radius_first_tie_and_failed_queries() {
        let mut nodes = vec![node(1, 10000.), node(2, 3.), node(3, -3.), node(4, 2.)];
        nodes[3].reachable = false;
        let s = nearest(&nodes, query());
        assert_eq!(s.selected, Some(2));
        assert_eq!(
            s.calls,
            vec![
                Call::Anchor,
                Call::Available(1),
                Call::Available(2),
                Call::Reachable(2),
                Call::Available(3),
                Call::Available(4),
                Call::Reachable(4)
            ]
        );
        assert_eq!(
            nearest(
                &nodes,
                Query {
                    anchor_present: false,
                    ..query()
                }
            )
            .calls,
            vec![Call::Anchor]
        );
    }
    #[test]
    fn availability_precedes_class_and_zone_and_clear_is_conditional() {
        let mut nodes = vec![node(1, 1.), node(2, 1.), node(3, 1.), node(4, 1.)];
        nodes[0].enabled = false;
        nodes[1].class_matches = false;
        nodes[2].same_zone = false;
        nodes[3].clear = false;
        let s = nearest(
            &nodes,
            Query {
                require_clear: true,
                ..query()
            },
        );
        assert_eq!(s.selected, None);
        assert_eq!(
            s.calls,
            vec![
                Call::Anchor,
                Call::Available(2),
                Call::Available(3),
                Call::Available(4),
                Call::Reachable(4),
                Call::Clear(4)
            ]
        );
    }
    #[test]
    fn native_best_ceiling_and_nan_rejection() {
        assert_eq!(
            nearest(
                &[node(1, 1e10)],
                Query {
                    range: 2e10,
                    ..query()
                }
            )
            .selected,
            None
        );
        assert_eq!(nearest(&[node(1, f32::NAN)], query()).selected, None);
        assert_eq!(
            nearest(
                &[node(1, 1.)],
                Query {
                    range: f32::NAN,
                    ..query()
                }
            )
            .selected,
            None
        );
    }
    #[test]
    fn destination_tie_and_unordered_choose_jump() {
        let p = Target {
            id: 1,
            position: [1., 0., 0.],
        };
        let j = Target {
            id: 2,
            position: [-1., 0., 0.],
        };
        assert_eq!(select([0.; 3], Some(p), Some(j)).unwrap().0, Kind::JumpDest);
        assert_eq!(
            select([f32::NAN, 0., 0.], Some(p), Some(j)).unwrap().0,
            Kind::JumpDest
        );
        assert_eq!(
            select(
                [0.; 3],
                Some(p),
                Some(Target {
                    position: [2., 0., 0.],
                    ..j
                })
            )
            .unwrap()
            .0,
            Kind::Perch
        );
        // FindNearest's y/x/z reduction ties these; destination selection's
        // z/y/x reduction distinguishes them by two at the 2^24 boundary.
        let mut first = node(1, 4096.);
        first.position = [4096., 1., 1.];
        assert_eq!(nearest(&[first, node(2, 4096.)], query()).selected, Some(1));
        assert_eq!(
            select(
                [0.; 3],
                Some(Target {
                    position: [4096., 0., 0.],
                    ..p
                }),
                Some(Target {
                    position: [4096., 1., 1.],
                    ..j
                })
            )
            .unwrap()
            .0,
            Kind::Perch
        );
    }
}
