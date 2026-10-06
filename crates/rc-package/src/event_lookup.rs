//! Reviewed UObject.FindFunction/UState.FindStruct lookup on explicit runtime snapshots.
//! Does not construct native post-load hash tables or resolve package-only state identity.
use crate::notify_wall::{notify_hit_wall, NotifyWallHandler, NotifyWallResult};
use serde::Serialize;
use std::collections::HashSet;

pub struct StateLookupSnapshot {
    pub parent: Option<usize>,
    /// Native +0x90: None means no table, Some(empty table) is still a table.
    pub hash_table: Option<[Option<usize>; 128]>,
}
pub struct StructLookupSnapshot {
    /// Native FName handle identity; distinct from its resolved numeric index.
    pub name_handle: u32,
    pub next_in_hash: Option<usize>,
    /// Native checks the candidate's class flags & 0x80000, not function flags.
    pub is_function_class: bool,
    pub handler: NotifyWallHandler,
}
pub struct EventLookupSnapshot {
    pub states: Vec<StateLookupSnapshot>,
    pub structs: Vec<StructLookupSnapshot>,
}
#[derive(Clone, Copy)]
pub struct EventNameSnapshot {
    pub handle: u32,
    pub resolved_index: i32,
}
#[derive(Debug, PartialEq, Eq, Serialize)]
pub enum FunctionLookupSource {
    ActiveState,
    ObjectClass,
}
#[derive(Debug, Serialize)]
pub struct FunctionSelection {
    pub struct_index: usize,
    pub source: FunctionLookupSource,
}
impl EventLookupSnapshot {
    fn find_struct(&self, start: usize, name: EventNameSnapshot) -> Result<Option<usize>, String> {
        let mut state = Some(start);
        let mut seen = HashSet::new();
        let table = loop {
            let Some(index) = state else { return Ok(None) };
            if !seen.insert(index) || seen.len() > 4096 {
                return Err("state lookup ancestry cycle/limit".into());
            }
            let entry = self
                .states
                .get(index)
                .ok_or("missing runtime state lookup snapshot")?;
            if let Some(table) = &entry.hash_table {
                break table;
            }
            state = entry.parent;
        };
        // FindStruct uses low seven bits of the resolved index for the bucket,
        // then compares the original name handle, including a null handle.
        let mut candidate = table[(name.resolved_index as u32 & 0x7f) as usize];
        let mut seen = HashSet::new();
        while let Some(index) = candidate {
            if !seen.insert(index) || seen.len() > 65536 {
                return Err("struct hash chain cycle/limit".into());
            }
            let entry = self
                .structs
                .get(index)
                .ok_or("missing runtime hash entry snapshot")?;
            if entry.name_handle == name.handle {
                return Ok(Some(index));
            }
            candidate = entry.next_in_hash;
        }
        // A present table with a missing name does not trigger another parent walk.
        Ok(None)
    }
    /// Native FindFunction: state first unless global_only, class only if state
    /// lookup misses. A nonfunction state match suppresses the class fallback.
    pub fn find_function(
        &self,
        object_class: usize,
        active_state: Option<usize>,
        global_only: bool,
        name: EventNameSnapshot,
    ) -> Result<Option<FunctionSelection>, String> {
        let state_match = if global_only {
            None
        } else if let Some(state) = active_state {
            self.find_struct(state, name)?
        } else {
            None
        };
        let (candidate, source) = if let Some(candidate) = state_match {
            (Some(candidate), FunctionLookupSource::ActiveState)
        } else {
            (
                self.find_struct(object_class, name)?,
                FunctionLookupSource::ObjectClass,
            )
        };
        Ok(candidate
            .filter(|&i| self.structs[i].is_function_class)
            .map(|struct_index| FunctionSelection {
                struct_index,
                source,
            }))
    }
    /// Native wrapper mask before name-based resolution; enabled missing handlers
    /// error here instead of emulating FindFunctionChecked's native fatal log.
    pub fn notify_hit_wall(
        &self,
        object_class: usize,
        active_state: Option<usize>,
        name: EventNameSnapshot,
        state_probe_mask: Option<u64>,
    ) -> Result<NotifyWallResult, String> {
        if crate::notify_wall::probe_masked_out(name.resolved_index, state_probe_mask) {
            return notify_hit_wall(
                name.resolved_index,
                state_probe_mask,
                NotifyWallHandler::Unresolved,
            );
        }
        let selection = self
            .find_function(object_class, active_state, false, name)?
            .ok_or("enabled NotifyHitWall function not found")?;
        notify_hit_wall(
            name.resolved_index,
            state_probe_mask,
            self.structs[selection.struct_index].handler,
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const NAME: EventNameSnapshot = EventNameSnapshot {
        handle: 0x1234,
        resolved_index: 353,
    };
    fn fixture() -> EventLookupSnapshot {
        let mut class = [None; 128];
        class[97] = Some(0);
        let mut state = [None; 128];
        state[97] = Some(1);
        EventLookupSnapshot {
            states: vec![
                StateLookupSnapshot {
                    parent: None,
                    hash_table: Some(class),
                },
                StateLookupSnapshot {
                    parent: None,
                    hash_table: Some(state),
                },
            ],
            structs: vec![
                StructLookupSnapshot {
                    name_handle: NAME.handle,
                    next_in_hash: None,
                    is_function_class: true,
                    handler: NotifyWallHandler::SuppliedResult(false),
                },
                StructLookupSnapshot {
                    name_handle: NAME.handle,
                    next_in_hash: None,
                    is_function_class: true,
                    handler: NotifyWallHandler::SuppliedResult(true),
                },
            ],
        }
    }
    #[test]
    fn state_override_class_fallback_and_global_lookup() {
        let mut graph = fixture();
        assert!(
            graph
                .notify_hit_wall(0, Some(1), NAME, None)
                .unwrap()
                .handled
        );
        assert_eq!(
            graph
                .find_function(0, Some(1), true, NAME)
                .unwrap()
                .unwrap()
                .source,
            FunctionLookupSource::ObjectClass
        );
        graph.states[1].hash_table = Some([None; 128]);
        assert!(
            !graph
                .notify_hit_wall(0, Some(1), NAME, None)
                .unwrap()
                .handled
        );
        graph.states[1].hash_table = fixture().states.remove(1).hash_table;
        graph.structs[1].is_function_class = false;
        assert!(graph
            .find_function(0, Some(1), false, NAME)
            .unwrap()
            .is_none());
        assert!(graph.notify_hit_wall(0, Some(1), NAME, None).is_err());
    }
    #[test]
    fn table_presence_and_handle_identity_control_parent_and_chain_walks() {
        let mut graph = fixture();
        graph.states[1].hash_table = None;
        graph.states[1].parent = Some(0);
        assert_eq!(graph.find_struct(1, NAME).unwrap(), Some(0));
        graph.states[1].hash_table = Some([None; 128]);
        assert_eq!(graph.find_struct(1, NAME).unwrap(), None);
        graph.structs[0].name_handle += 1;
        graph.structs[0].next_in_hash = Some(1);
        assert_eq!(graph.find_struct(0, NAME).unwrap(), Some(1));
        graph.structs[1].name_handle = 0;
        assert_eq!(
            graph
                .find_struct(0, EventNameSnapshot { handle: 0, ..NAME })
                .unwrap(),
            Some(1)
        );
    }
    #[test]
    fn incomplete_tables_cycles_unknown_handlers_and_masked_lookup() {
        let mut graph = fixture();
        graph.structs[1].handler = NotifyWallHandler::Unresolved;
        assert!(graph.notify_hit_wall(0, Some(1), NAME, None).is_err());
        assert!(
            !graph
                .notify_hit_wall(999, Some(999), NAME, Some(0))
                .unwrap()
                .handled
        );
        graph.states[1].hash_table = None;
        graph.states[1].parent = Some(1);
        assert!(graph.find_function(0, Some(1), false, NAME).is_err());
        graph.states[1].parent = Some(999);
        assert!(graph.find_function(0, Some(1), false, NAME).is_err());
        graph.structs[0].name_handle += 1;
        graph.structs[0].next_in_hash = Some(0);
        assert!(graph.find_function(0, None, false, NAME).is_err());
        graph.structs[0].next_in_hash = Some(999);
        assert!(graph.find_function(0, None, false, NAME).is_err());
    }
}
