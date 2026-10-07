//! Base UObject.GotoState control flow on explicit snapshots. Callbacks are host
//! supplied, never implicitly treated as empty UnrealScript or native functions.
use crate::{
    event_lookup::EventNameSnapshot,
    probe_frame::{is_probing, ProbeFrame, StateProbeMasks},
    state_selection::ResolvedStateTarget,
};
use serde::Serialize;

pub const STATE_CHANGED: u32 = 0x1000;
pub const IN_END_STATE: u32 = 0x2000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct StateExecution {
    pub probe: ProbeFrame,
    /// FFrame.Node (+4), independently of FStateFrame.StateNode (+0x18).
    pub node: usize,
    /// Opaque host offset, not an original pointer or VM instruction mapping.
    pub code: Option<u32>,
    /// Explicit input: InitExecution does not initialize this field in the proof.
    pub latent_action: u16,
    pub object_flags: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum StateEvent {
    EndState,
    BeginState,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum GotoStateResult {
    /// Native EGotoState 0: absent frame or None/class fallback.
    NotFound,
    /// Native EGotoState 1, also returned for an unchanged non-None name.
    Success,
    /// Native EGotoState 2: a callback set STATE_CHANGED.
    Preempted,
}

/// Metadata (names and masks) is an immutable supplied snapshot. Host callbacks
/// may recursively invoke this adapter on the same execution snapshot. They must
/// preserve the object class and frame lifetime, as the native base path assumes.
/// Callback errors stop at that phase and retain preceding writes; no rollback.
pub fn goto_state(
    execution: Option<&mut StateExecution>,
    target: &ResolvedStateTarget,
    masks: StateProbeMasks,
    state_names: &[Option<EventNameSnapshot>],
    callback: &mut impl FnMut(&mut StateExecution, StateEvent) -> Result<(), String>,
) -> Result<GotoStateResult, String> {
    let Some(execution) = execution else {
        return Ok(GotoStateResult::NotFound);
    };
    execution.latent_action = 0;
    let old_handle = if execution.probe.state_node == execution.probe.object_class {
        0
    } else {
        state_names
            .get(execution.probe.state_node)
            .and_then(|name| *name)
            .ok_or("missing previous state name")?
            .handle
    };
    let changed_name = old_handle != target.name.handle;
    if old_handle != 0
        && changed_name
        && is_probing(317, Some(&execution.probe))
        && execution.object_flags & IN_END_STATE == 0
    {
        execution.object_flags = (execution.object_flags & !STATE_CHANGED) | IN_END_STATE;
        // On an unresolved callback error the native continuation has not run.
        callback(execution, StateEvent::EndState)?;
        execution.object_flags &= !IN_END_STATE;
        if execution.object_flags & STATE_CHANGED != 0 {
            return Ok(GotoStateResult::Preempted);
        }
    }
    execution.node = target.selection.state_node;
    execution.probe = execution
        .probe
        .resolved_state_masks(target.selection.state_node, masks);
    execution.code = None;
    if target.name.handle == 0 {
        return Ok(GotoStateResult::NotFound);
    }
    if changed_name && is_probing(316, Some(&execution.probe)) {
        execution.object_flags &= !STATE_CHANGED;
        callback(execution, StateEvent::BeginState)?;
        if execution.object_flags & STATE_CHANGED != 0 {
            return Ok(GotoStateResult::Preempted);
        }
    }
    execution.object_flags |= STATE_CHANGED;
    Ok(GotoStateResult::Success)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_selection::{NamedStateTarget, StateTargetRoute};
    const BEGIN: u64 = 1 << 16;
    const END: u64 = 1 << 17;
    fn name(handle: u32) -> EventNameSnapshot {
        EventNameSnapshot {
            handle,
            resolved_index: handle as i32 - 1,
        }
    }
    fn names() -> [Option<EventNameSnapshot>; 4] {
        [None, Some(name(41)), Some(name(42)), Some(name(43))]
    }
    fn target(node: usize) -> ResolvedStateTarget {
        ResolvedStateTarget {
            selection: NamedStateTarget {
                state_node: node,
                route: if node == 0 {
                    StateTargetRoute::ClassFallback
                } else {
                    StateTargetRoute::NamedState
                },
            },
            name: if node == 0 {
                EventNameSnapshot {
                    handle: 0,
                    resolved_index: 0,
                }
            } else {
                names()[node].unwrap()
            },
        }
    }
    fn masks(bits: u64) -> StateProbeMasks {
        StateProbeMasks {
            class_probe: bits,
            state_probe: 0,
            state_ignore: u64::MAX,
        }
    }
    fn execution(node: usize) -> StateExecution {
        StateExecution {
            probe: ProbeFrame {
                object_class: 0,
                state_node: node,
                probe_mask: u64::MAX,
            },
            node: 3,
            code: Some(99),
            latent_action: 7,
            object_flags: 0x80 | STATE_CHANGED,
        }
    }
    #[test]
    fn callbacks_observe_old_then_new_frame_and_masks() {
        let mut state = execution(1);
        let mut events = Vec::new();
        let result = goto_state(
            Some(&mut state),
            &target(2),
            masks(BEGIN),
            &names(),
            &mut |s, e| {
                events.push(e);
                assert_eq!(s.latent_action, 0);
                assert_eq!(s.object_flags & STATE_CHANGED, 0);
                match e {
                    StateEvent::EndState => {
                        assert_eq!(s.probe.state_node, 1);
                        assert_eq!(s.code, Some(99));
                        assert_ne!(s.object_flags & IN_END_STATE, 0);
                    }
                    StateEvent::BeginState => {
                        assert_eq!(s.probe.state_node, 2);
                        assert_eq!(s.node, 2);
                        assert_eq!(s.code, None);
                        assert_eq!(s.probe.probe_mask, BEGIN);
                        assert_eq!(s.object_flags & IN_END_STATE, 0);
                    }
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(result, GotoStateResult::Success);
        assert_eq!(events, [StateEvent::EndState, StateEvent::BeginState]);
        assert_eq!(state.object_flags, 0x80 | STATE_CHANGED);
    }
    #[test]
    fn absent_frame_same_name_and_none_have_distinct_writes() {
        let mut fail = |_: &mut StateExecution, _| Err("unexpected event".into());
        assert_eq!(
            goto_state(None, &target(2), masks(BEGIN | END), &[], &mut fail).unwrap(),
            GotoStateResult::NotFound
        );
        let mut state = execution(1);
        assert_eq!(
            goto_state(Some(&mut state), &target(1), masks(0), &names(), &mut fail).unwrap(),
            GotoStateResult::Success
        );
        assert_eq!(
            (
                state.node,
                state.code,
                state.latent_action,
                state.probe.probe_mask
            ),
            (1, None, 0, 0)
        );
        state.object_flags |= IN_END_STATE;
        assert_eq!(
            goto_state(
                Some(&mut state),
                &target(0),
                masks(END),
                &names(),
                &mut fail
            )
            .unwrap(),
            GotoStateResult::NotFound
        );
        assert_eq!(state.probe.state_node, 0);
        // Fallback does not set or clear a pre-existing STATE_CHANGED flag.
        assert_eq!(state.object_flags, 0x80 | STATE_CHANGED | IN_END_STATE);
    }
    #[test]
    fn old_end_mask_and_new_begin_mask_gate_independently() {
        for (old, new, expected) in [
            (0, 0, vec![]),
            (END, 0, vec![StateEvent::EndState]),
            (0, BEGIN, vec![StateEvent::BeginState]),
            (BEGIN, END, vec![]),
        ] {
            let mut state = execution(1);
            state.probe.probe_mask = old;
            let mut events = Vec::new();
            goto_state(
                Some(&mut state),
                &target(2),
                masks(new),
                &names(),
                &mut |_, e| {
                    events.push(e);
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(events, expected);
        }
    }
    #[test]
    fn recursive_end_redirect_preempts_outer_assignment_and_guard_blocks_reentry() {
        let mut state = execution(1);
        let mut events = Vec::new();
        let result = goto_state(
            Some(&mut state),
            &target(2),
            masks(BEGIN | END),
            &names(),
            &mut |s, e| {
                events.push(e);
                assert_eq!(e, StateEvent::EndState);
                // Inner switch must skip EndState because outer EndState is running.
                assert_eq!(
                    goto_state(Some(s), &target(3), masks(0), &names(), &mut |_, _| Err(
                        "recursive EndState".into()
                    ))?,
                    GotoStateResult::Success
                );
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(result, GotoStateResult::Preempted);
        assert_eq!(events, [StateEvent::EndState]);
        assert_eq!(state.probe.state_node, 3);
        assert_eq!(state.object_flags & IN_END_STATE, 0);
    }
    #[test]
    fn recursive_begin_redirect_preempts_and_none_redirect_does_not_set_flag() {
        for (redirect, expected, node) in [
            (3, GotoStateResult::Preempted, 3),
            (0, GotoStateResult::Success, 0),
        ] {
            let mut state = execution(0);
            let result = goto_state(
                Some(&mut state),
                &target(2),
                masks(BEGIN),
                &names(),
                &mut |s, e| {
                    assert_eq!(e, StateEvent::BeginState);
                    goto_state(
                        Some(s),
                        &target(redirect),
                        masks(0),
                        &names(),
                        &mut |_, _| Err("unexpected inner event".into()),
                    )?;
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(result, expected);
            assert_eq!(state.probe.state_node, node);
            assert_ne!(state.object_flags & STATE_CHANGED, 0);
        }
    }
    #[test]
    fn recursive_none_during_end_does_not_preempt_outer_target() {
        let mut state = execution(1);
        let result = goto_state(
            Some(&mut state),
            &target(2),
            masks(0),
            &names(),
            &mut |s, event| {
                assert_eq!(event, StateEvent::EndState);
                assert_eq!(
                    goto_state(Some(s), &target(0), masks(0), &names(), &mut |_, _| Err(
                        "unexpected inner event".into()
                    ))?,
                    GotoStateResult::NotFound
                );
                assert_eq!(s.object_flags & STATE_CHANGED, 0);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(result, GotoStateResult::Success);
        assert_eq!(state.probe.state_node, 2);
        assert_eq!(state.object_flags & IN_END_STATE, 0);
    }
    #[test]
    fn same_name_different_node_suppresses_callbacks_but_assigns_target() {
        let mut state = execution(1);
        let mut other = target(2);
        other.name = names()[1].unwrap();
        goto_state(
            Some(&mut state),
            &other,
            masks(BEGIN | END),
            &names(),
            &mut |_, _| Err("same-name callback".into()),
        )
        .unwrap();
        assert_eq!(state.probe.state_node, 2);
    }
    #[test]
    fn unresolved_callbacks_stop_at_the_observed_phase() {
        for node in [0, 1] {
            let mut state = execution(node);
            let error = goto_state(
                Some(&mut state),
                &target(2),
                masks(BEGIN),
                &names(),
                &mut |_, _| Err("unresolved handler".into()),
            )
            .unwrap_err();
            assert_eq!(error, "unresolved handler");
            assert_eq!(state.latent_action, 0);
            assert_eq!(state.probe.state_node, if node == 0 { 2 } else { 1 });
            assert_eq!(
                state.object_flags & IN_END_STATE,
                if node == 0 { 0 } else { IN_END_STATE }
            );
        }
    }
}
