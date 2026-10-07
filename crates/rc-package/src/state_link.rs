//! First-link UState hash construction from ordered, filtered direct struct children.
//! Child extraction, native name registration and relinking remain external.
use crate::{
    event_lookup::{
        EventLookupSnapshot, EventNameSnapshot, StateLookupSnapshot, StructLookupSnapshot,
    },
    notify_wall::NotifyWallHandler,
};
use std::collections::{HashMap, HashSet};

pub struct StateLinkInput {
    pub parent: Option<usize>,
    /// Native UStruct iterator order, restricted to children whose Outer is this
    /// state/class. Properties and other non-UStruct fields must be filtered out.
    pub direct_structs: Vec<usize>,
}
pub struct StructLinkInput {
    pub name: EventNameSnapshot,
    pub is_function_class: bool,
    pub handler: NotifyWallHandler,
}

/// Build fresh tables parent-first. Native allocates only upon the first direct
/// UStruct child, copies the closest ancestor table, then prepends each own child.
/// Shared inherited chains remain intact; a childless state keeps a null table.
pub fn link_states(
    states: &[StateLinkInput],
    structs: &[StructLinkInput],
) -> Result<EventLookupSnapshot, String> {
    let mut owners = HashSet::new();
    let mut handles = HashMap::new();
    let mut indices = HashMap::new();
    for entry in structs {
        if handles
            .insert(entry.name.handle, entry.name.resolved_index)
            .is_some_and(|old| old != entry.name.resolved_index)
            || indices
                .insert(entry.name.resolved_index, entry.name.handle)
                .is_some_and(|old| old != entry.name.handle)
        {
            return Err("inconsistent global FName identity snapshot".into());
        }
    }
    for state in states {
        if state.parent.is_some_and(|parent| parent >= states.len()) {
            return Err("missing state link parent".into());
        }
        for &child in &state.direct_structs {
            if child >= structs.len() {
                return Err("missing direct struct link input".into());
            }
            if !owners.insert(child) {
                return Err("struct has duplicate direct ownership".into());
            }
        }
    }
    let mut graph = EventLookupSnapshot {
        states: states
            .iter()
            .map(|state| StateLookupSnapshot {
                parent: state.parent,
                hash_table: None,
            })
            .collect(),
        structs: structs
            .iter()
            .map(|entry| StructLookupSnapshot {
                name_handle: entry.name.handle,
                next_in_hash: None,
                is_function_class: entry.is_function_class,
                handler: entry.handler,
            })
            .collect(),
    };
    let mut completed = vec![false; states.len()];
    for start in 0..states.len() {
        let mut chain = Vec::new();
        let mut seen = HashSet::new();
        let mut next = Some(start);
        while let Some(index) = next {
            if completed[index] {
                break;
            }
            if !seen.insert(index) || seen.len() > 4096 {
                return Err("state link ancestry cycle/limit".into());
            }
            chain.push(index);
            next = states[index].parent;
        }
        for index in chain.into_iter().rev() {
            let state = &states[index];
            if !state.direct_structs.is_empty() {
                let mut table = [None; 128];
                let mut parent = state.parent;
                while let Some(ancestor) = parent {
                    if let Some(inherited) = graph.states[ancestor].hash_table {
                        table = inherited;
                        break;
                    }
                    parent = states[ancestor].parent;
                }
                for &child in &state.direct_structs {
                    let bucket = (structs[child].name.resolved_index as u32 & 0x7f) as usize;
                    graph.structs[child].next_in_hash = table[bucket];
                    table[bucket] = Some(child);
                }
                graph.states[index].hash_table = Some(table);
            }
            completed[index] = true;
        }
    }
    Ok(graph)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_lookup::FunctionLookupSource;
    const NAME: EventNameSnapshot = EventNameSnapshot {
        handle: 11,
        resolved_index: 353,
    };
    fn field(name: EventNameSnapshot, handled: bool) -> StructLinkInput {
        StructLinkInput {
            name,
            is_function_class: true,
            handler: NotifyWallHandler::SuppliedResult(handled),
        }
    }
    #[test]
    fn inherited_tables_override_and_childless_parent_walk() {
        // Deliberately child-first input order, with a tableless intermediate state.
        let states = [
            StateLinkInput {
                parent: Some(1),
                direct_structs: vec![1],
            },
            StateLinkInput {
                parent: Some(2),
                direct_structs: vec![],
            },
            StateLinkInput {
                parent: None,
                direct_structs: vec![0],
            },
            StateLinkInput {
                parent: Some(2),
                direct_structs: vec![],
            },
        ];
        let graph = link_states(&states, &[field(NAME, false), field(NAME, true)]).unwrap();
        assert!(graph.states[1].hash_table.is_none());
        assert!(graph.states[3].hash_table.is_none());
        assert_eq!(graph.structs[1].next_in_hash, Some(0));
        assert!(
            graph
                .notify_hit_wall(2, Some(0), NAME, None)
                .unwrap()
                .handled
        );
        assert!(!graph.notify_hit_wall(3, None, NAME, None).unwrap().handled);
        assert_eq!(
            graph
                .find_function(2, Some(0), false, NAME)
                .unwrap()
                .unwrap()
                .source,
            FunctionLookupSource::ActiveState
        );
        assert_eq!(graph.states[2].hash_table.unwrap()[97], Some(0));
        assert_eq!(graph.structs[0].next_in_hash, None);
    }
    #[test]
    fn collisions_prepend_in_iterator_order_and_preserve_sibling_chains() {
        let other = EventNameSnapshot {
            handle: 22,
            resolved_index: 481,
        };
        let states = [
            StateLinkInput {
                parent: None,
                direct_structs: vec![0],
            },
            StateLinkInput {
                parent: Some(0),
                direct_structs: vec![1, 2],
            },
            StateLinkInput {
                parent: Some(0),
                direct_structs: vec![3],
            },
        ];
        let mut fields = [
            field(NAME, false),
            field(NAME, false),
            field(other, true),
            field(NAME, true),
        ];
        let graph = link_states(&states, &fields).unwrap();
        assert_eq!(graph.states[1].hash_table.unwrap()[97], Some(2));
        assert_eq!(graph.structs[2].next_in_hash, Some(1));
        assert_eq!(graph.structs[1].next_in_hash, Some(0));
        assert_eq!(graph.structs[3].next_in_hash, Some(0));
        assert!(!graph.notify_hit_wall(1, None, NAME, None).unwrap().handled);
        assert!(graph.notify_hit_wall(2, None, NAME, None).unwrap().handled);
        assert!(graph.notify_hit_wall(1, None, other, None).unwrap().handled);
        fields[1].is_function_class = false;
        assert!(link_states(&states, &fields)
            .unwrap()
            .find_function(1, None, false, NAME)
            .unwrap()
            .is_none());
    }
    #[test]
    fn incomplete_ownership_name_identity_and_cycles_are_rejected() {
        let input = [field(NAME, false)];
        assert!(link_states(
            &[StateLinkInput {
                parent: Some(0),
                direct_structs: vec![]
            }],
            &input
        )
        .is_err());
        assert!(link_states(
            &[StateLinkInput {
                parent: Some(9),
                direct_structs: vec![]
            }],
            &input
        )
        .is_err());
        assert!(link_states(
            &[StateLinkInput {
                parent: None,
                direct_structs: vec![9]
            }],
            &input
        )
        .is_err());
        assert!(link_states(
            &[StateLinkInput {
                parent: None,
                direct_structs: vec![0, 0]
            }],
            &input
        )
        .is_err());
        assert!(link_states(
            &[
                StateLinkInput {
                    parent: None,
                    direct_structs: vec![0]
                },
                StateLinkInput {
                    parent: None,
                    direct_structs: vec![0]
                }
            ],
            &input
        )
        .is_err());
        assert!(link_states(
            &[],
            &[
                field(NAME, false),
                field(EventNameSnapshot { handle: 12, ..NAME }, false)
            ]
        )
        .is_err());
        assert!(link_states(
            &[],
            &[
                field(NAME, false),
                field(
                    EventNameSnapshot {
                        resolved_index: 354,
                        ..NAME
                    },
                    false
                )
            ]
        )
        .is_err());
        assert!(link_states(&[], &[]).unwrap().states.is_empty());
    }
}
