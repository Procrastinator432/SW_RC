//! Named and automatic GotoState target phases; no callbacks or state execution.
use crate::event_lookup::{EventLookupSnapshot, EventNameSnapshot};
use serde::Serialize;

#[derive(Clone, Copy)]
pub enum StateStructType {
    /// Candidate class passes native class flags & 0x40000; node is in states[].
    StateNode(usize),
    NotState,
    Unresolved,
}
pub struct NamedStateLookup<'a> {
    pub graph: &'a EventLookupSnapshot,
    pub struct_types: &'a [StateStructType],
}
#[derive(Debug, PartialEq, Eq, Serialize)]
pub enum StateTargetRoute {
    NamedState,
    AutoState,
    ClassFallback,
}
pub struct AutoStateLookup<'a> {
    pub graph: &'a EventLookupSnapshot,
    /// Own State-typed children in original forward iterator order, not hash order.
    pub direct_states: &'a [Vec<usize>],
    /// Original StateFlags indexed by state node. None means missing metadata.
    pub state_flags: &'a [Option<u32>],
}
#[derive(Debug, Serialize)]
pub struct ResolvedStateTarget {
    pub selection: NamedStateTarget,
    /// GotoState's effective event name: None for class fallback; the candidate's
    /// own name for Auto; the requested name for a successful named lookup.
    pub name: EventNameSnapshot,
}
impl AutoStateLookup<'_> {
    pub fn resolve_target(
        &self,
        object_class: usize,
        requested: EventNameSnapshot,
        struct_types: &[StateStructType],
        state_names: &[Option<EventNameSnapshot>],
    ) -> Result<ResolvedStateTarget, String> {
        let selection = if requested.resolved_index == 690 {
            self.resolve_auto_target(object_class)?
        } else {
            NamedStateLookup {
                graph: self.graph,
                struct_types,
            }
            .resolve_named_target(object_class, requested)?
        };
        let name = match selection.route {
            StateTargetRoute::ClassFallback => EventNameSnapshot {
                handle: 0,
                resolved_index: 0,
            },
            StateTargetRoute::NamedState => requested,
            StateTargetRoute::AutoState => state_names
                .get(selection.state_node)
                .and_then(|name| *name)
                .ok_or("missing Auto candidate name")?,
        };
        Ok(ResolvedStateTarget { selection, name })
    }
    pub fn resolve_auto_target(&self, object_class: usize) -> Result<NamedStateTarget, String> {
        let mut next = Some(object_class);
        let mut seen = std::collections::HashSet::new();
        while let Some(class) = next {
            if !seen.insert(class) || seen.len() > 4096 {
                return Err("auto-state ancestry cycle/limit".into());
            }
            let owner = self
                .graph
                .states
                .get(class)
                .ok_or("missing auto-state class node")?;
            for &state in self
                .direct_states
                .get(class)
                .ok_or("missing ordered state children")?
            {
                if state >= self.graph.states.len() {
                    return Err("missing auto-state candidate node".into());
                }
                let flags = self
                    .state_flags
                    .get(state)
                    .and_then(|f| *f)
                    .ok_or("missing auto-state flags")?;
                if flags & 2 != 0 {
                    return Ok(NamedStateTarget {
                        state_node: state,
                        route: StateTargetRoute::AutoState,
                    });
                }
            }
            next = owner.parent;
        }
        Ok(NamedStateTarget {
            state_node: object_class,
            route: StateTargetRoute::ClassFallback,
        })
    }
}
#[derive(Debug, Serialize)]
pub struct NamedStateTarget {
    pub state_node: usize,
    pub route: StateTargetRoute,
}
impl NamedStateLookup<'_> {
    /// FindState searches only the object's class table, then checks the candidate
    /// type. A non-state shadow does not resume lookup at its parent's table.
    pub fn find_state(
        &self,
        object_class: usize,
        name: EventNameSnapshot,
    ) -> Result<Option<usize>, String> {
        let Some(candidate) = self.graph.find_struct(object_class, name)? else {
            return Ok(None);
        };
        match self
            .struct_types
            .get(candidate)
            .ok_or("missing state type snapshot")?
        {
            StateStructType::StateNode(node) => {
                if *node >= self.graph.states.len() {
                    return Err("missing selected state node".into());
                }
                Ok(Some(*node))
            }
            StateStructType::NotState => Ok(None),
            StateStructType::Unresolved => Err("state candidate type unresolved".into()),
        }
    }
    /// GotoState's named target phase only. Missing/ill-typed names select the
    /// object class. Auto (fixed native index690) needs separate ordered flag logic.
    pub fn resolve_named_target(
        &self,
        object_class: usize,
        name: EventNameSnapshot,
    ) -> Result<NamedStateTarget, String> {
        if name.resolved_index == 690 {
            return Err("Auto requires the ordered Auto-state resolver".into());
        }
        let (state_node, route) = if let Some(node) = self.find_state(object_class, name)? {
            (node, StateTargetRoute::NamedState)
        } else {
            (object_class, StateTargetRoute::ClassFallback)
        };
        Ok(NamedStateTarget { state_node, route })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        notify_wall::NotifyWallHandler,
        state_link::{link_states, StateLinkInput, StructLinkInput},
    };
    const NAME: EventNameSnapshot = EventNameSnapshot {
        handle: 15,
        resolved_index: 14,
    };
    fn graph() -> EventLookupSnapshot {
        link_states(
            &[
                StateLinkInput {
                    parent: None,
                    direct_structs: vec![0],
                },
                StateLinkInput {
                    parent: Some(0),
                    direct_structs: vec![],
                },
                StateLinkInput {
                    parent: Some(0),
                    direct_structs: vec![1],
                },
                StateLinkInput {
                    parent: None,
                    direct_structs: vec![],
                },
            ],
            &[
                StructLinkInput {
                    name: NAME,
                    is_function_class: false,
                    handler: NotifyWallHandler::Unresolved,
                },
                StructLinkInput {
                    name: NAME,
                    is_function_class: false,
                    handler: NotifyWallHandler::Unresolved,
                },
            ],
        )
        .unwrap()
    }
    #[test]
    fn inherited_named_state_and_override_then_nonstate_shadow() {
        let graph = graph();
        let mut kinds = [StateStructType::StateNode(3), StateStructType::StateNode(2)];
        let lookup = NamedStateLookup {
            graph: &graph,
            struct_types: &kinds,
        };
        assert_eq!(lookup.find_state(1, NAME).unwrap(), Some(3));
        assert_eq!(lookup.find_state(2, NAME).unwrap(), Some(2));
        kinds[1] = StateStructType::NotState;
        let lookup = NamedStateLookup {
            graph: &graph,
            struct_types: &kinds,
        };
        let target = lookup.resolve_named_target(2, NAME).unwrap();
        assert_eq!(target.route, StateTargetRoute::ClassFallback);
        assert_eq!(target.state_node, 2);
    }
    #[test]
    fn missing_none_and_auto_requests_are_distinct() {
        let graph = graph();
        let kinds = [StateStructType::StateNode(3), StateStructType::NotState];
        let lookup = NamedStateLookup {
            graph: &graph,
            struct_types: &kinds,
        };
        for name in [
            EventNameSnapshot {
                handle: 0,
                resolved_index: 0,
            },
            EventNameSnapshot {
                handle: 99,
                resolved_index: 98,
            },
        ] {
            let target = lookup.resolve_named_target(1, name).unwrap();
            assert_eq!(target.state_node, 1);
            assert_eq!(target.route, StateTargetRoute::ClassFallback);
        }
        assert!(lookup
            .resolve_named_target(
                1,
                EventNameSnapshot {
                    handle: 691,
                    resolved_index: 690
                }
            )
            .is_err());
    }
    #[test]
    fn unknown_type_missing_node_and_missing_graph_fail() {
        let graph = graph();
        for kinds in [
            vec![],
            vec![StateStructType::Unresolved],
            vec![StateStructType::StateNode(999)],
        ] {
            let lookup = NamedStateLookup {
                graph: &graph,
                struct_types: &kinds,
            };
            assert!(lookup.resolve_named_target(1, NAME).is_err());
        }
        assert!(NamedStateLookup {
            graph: &graph,
            struct_types: &[]
        }
        .resolve_named_target(999, NAME)
        .is_err());
    }
    #[test]
    fn auto_uses_forward_own_order_before_parent_candidates() {
        let graph = graph();
        let direct = [vec![3], vec![], vec![2, 3], vec![]];
        let mut flags = [None, None, Some(2), Some(2)];
        let auto = AutoStateLookup {
            graph: &graph,
            direct_states: &direct,
            state_flags: &flags,
        };
        assert_eq!(auto.resolve_auto_target(2).unwrap().state_node, 2);
        assert_eq!(auto.resolve_auto_target(1).unwrap().state_node, 3);
        flags[2] = Some(0);
        assert_eq!(
            AutoStateLookup {
                graph: &graph,
                direct_states: &direct,
                state_flags: &flags
            }
            .resolve_auto_target(2)
            .unwrap()
            .state_node,
            3
        );
    }
    #[test]
    fn auto_does_not_re_resolve_parent_candidate_by_its_name() {
        let graph = graph();
        // Both graph entries share NAME. Named lookup selects derived node2,
        // while Auto skips its non-auto flag and selects inherited node3 directly.
        let direct = [vec![3], vec![], vec![2], vec![]];
        let flags = [None, None, Some(0), Some(2)];
        let named = NamedStateLookup {
            graph: &graph,
            struct_types: &[StateStructType::StateNode(3), StateStructType::StateNode(2)],
        };
        assert_eq!(named.resolve_named_target(2, NAME).unwrap().state_node, 2);
        let target = AutoStateLookup {
            graph: &graph,
            direct_states: &direct,
            state_flags: &flags,
        }
        .resolve_auto_target(2)
        .unwrap();
        assert_eq!(target.state_node, 3);
        assert_eq!(target.route, StateTargetRoute::AutoState);
        let none = [None, None, Some(0), Some(0)];
        let fallback = AutoStateLookup {
            graph: &graph,
            direct_states: &direct,
            state_flags: &none,
        }
        .resolve_auto_target(2)
        .unwrap();
        assert_eq!(fallback.state_node, 2);
        assert_eq!(fallback.route, StateTargetRoute::ClassFallback);
    }
    #[test]
    fn unified_target_preserves_named_identity_and_replaces_auto_or_fallback_name() {
        let graph = graph();
        let direct = [vec![3], vec![], vec![2], vec![]];
        let flags = [None, None, Some(0), Some(2)];
        let auto = AutoStateLookup {
            graph: &graph,
            direct_states: &direct,
            state_flags: &flags,
        };
        let types = [StateStructType::StateNode(3), StateStructType::StateNode(2)];
        let names = [None, None, Some(NAME), Some(NAME)];
        let request = EventNameSnapshot {
            handle: 691,
            resolved_index: 690,
        };
        let selected = auto.resolve_target(2, request, &types, &names).unwrap();
        assert_eq!(selected.selection.state_node, 3);
        assert_eq!(selected.name, NAME);
        assert!(auto.resolve_target(2, request, &types, &[]).is_err());
        let selected = auto.resolve_target(2, NAME, &types, &[]).unwrap();
        assert_eq!(selected.selection.state_node, 2);
        assert_eq!(selected.name, NAME);
        let selected = auto
            .resolve_target(
                2,
                EventNameSnapshot {
                    handle: 99,
                    resolved_index: 98,
                },
                &types,
                &[],
            )
            .unwrap();
        assert_eq!(
            selected.name,
            EventNameSnapshot {
                handle: 0,
                resolved_index: 0
            }
        );
        assert_eq!(selected.selection.route, StateTargetRoute::ClassFallback);
    }
    #[test]
    fn auto_incomplete_snapshots_and_cycles_error_without_target() {
        let mut graph = graph();
        let direct = [vec![3], vec![], vec![], vec![]];
        for flags in [vec![], vec![None, None, None, None]] {
            assert!(AutoStateLookup {
                graph: &graph,
                direct_states: &direct,
                state_flags: &flags
            }
            .resolve_auto_target(0)
            .is_err());
        }
        assert!(AutoStateLookup {
            graph: &graph,
            direct_states: &[],
            state_flags: &[]
        }
        .resolve_auto_target(0)
        .is_err());
        assert!(AutoStateLookup {
            graph: &graph,
            direct_states: &direct,
            state_flags: &[]
        }
        .resolve_auto_target(999)
        .is_err());
        let bad = [vec![999], vec![], vec![], vec![]];
        assert!(AutoStateLookup {
            graph: &graph,
            direct_states: &bad,
            state_flags: &[]
        }
        .resolve_auto_target(0)
        .is_err());
        graph.states[1].parent = Some(1);
        assert!(AutoStateLookup {
            graph: &graph,
            direct_states: &direct,
            state_flags: &[]
        }
        .resolve_auto_target(1)
        .is_err());
    }
}
