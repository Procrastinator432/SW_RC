//! execGotoState decision flow after argument evaluation. Label execution and
//! virtual overrides are explicit host responsibilities; no VM or logging IO.
//! Supplied arguments must not mutate state while being evaluated: native captures
//! the old name before evaluating them, while this adapter captures it at entry.
use crate::{
    event_lookup::EventNameSnapshot,
    probe_frame::StateProbeMasks,
    state_selection::ResolvedStateTarget,
    state_transition::{goto_state, GotoStateResult, StateEvent, StateExecution},
};
use serde::Serialize;

#[derive(Clone, Copy)]
pub struct ScriptStateRequest {
    pub state: EventNameSnapshot,
    /// None or a None handle selects the fixed Begin label (native index100).
    pub label: Option<EventNameSnapshot>,
}
#[derive(Debug, PartialEq, Eq, Serialize)]
pub enum ScriptStateWarning {
    MissingState,
    MissingLabel,
}
#[derive(Debug, Serialize)]
pub struct ScriptStateResult {
    /// None means same-name request bypassed the virtual GotoState call.
    pub state_result: Option<GotoStateResult>,
    pub label_result: Option<bool>,
    pub warning: Option<ScriptStateWarning>,
}

pub fn script_goto_state(
    mut execution: Option<&mut StateExecution>,
    request: ScriptStateRequest,
    target: &ResolvedStateTarget,
    masks: StateProbeMasks,
    state_names: &[Option<EventNameSnapshot>],
    callback: &mut impl FnMut(&mut StateExecution, StateEvent) -> Result<(), String>,
    label: &mut impl FnMut(Option<&mut StateExecution>, EventNameSnapshot) -> Result<bool, String>,
) -> Result<ScriptStateResult, String> {
    let old_handle = match execution.as_deref() {
        Some(s) if s.probe.state_node != s.probe.object_class => {
            state_names
                .get(s.probe.state_node)
                .and_then(|n| *n)
                .ok_or("missing script previous state name")?
                .handle
        }
        _ => 0,
    };
    let state_result = if old_handle == request.state.handle {
        None
    } else {
        Some(goto_state(
            execution.as_deref_mut(),
            target,
            masks,
            state_names,
            callback,
        )?)
    };
    let mut result = ScriptStateResult {
        state_result,
        label_result: None,
        warning: None,
    };
    if state_result.is_none() || state_result == Some(GotoStateResult::Success) {
        let explicit = request.label.filter(|n| n.handle != 0);
        let label_name = explicit
            .unwrap_or(crate::name_bindings::hardcoded_name("Begin").ok_or("missing Begin label")?);
        let found = label(execution, label_name)?;
        result.label_result = Some(found);
        if !found && explicit.is_some() {
            result.warning = Some(ScriptStateWarning::MissingLabel);
        }
    } else if state_result == Some(GotoStateResult::NotFound)
        && request.state.handle != 0
        && request.state.resolved_index != 690
    {
        result.warning = Some(ScriptStateWarning::MissingState);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        probe_frame::ProbeFrame,
        state_selection::{NamedStateTarget, StateTargetRoute},
        state_transition::STATE_CHANGED,
    };
    fn name(index: i32) -> EventNameSnapshot {
        EventNameSnapshot {
            handle: if index == 0 { 0 } else { index as u32 + 1 },
            resolved_index: index,
        }
    }
    fn target(node: usize) -> ResolvedStateTarget {
        ResolvedStateTarget {
            selection: NamedStateTarget {
                state_node: node,
                route: StateTargetRoute::NamedState,
            },
            name: name(700 + node as i32),
        }
    }
    fn execution() -> StateExecution {
        StateExecution {
            probe: ProbeFrame {
                object_class: 0,
                state_node: 1,
                probe_mask: 0,
            },
            node: 1,
            code: Some(17),
            latent_action: 8,
            object_flags: 0,
        }
    }
    fn masks(bits: u64) -> StateProbeMasks {
        StateProbeMasks {
            class_probe: 0,
            state_probe: bits,
            state_ignore: u64::MAX,
        }
    }
    #[test]
    fn same_name_skips_base_and_default_begin_label_still_runs() {
        let mut state = execution();
        let mut calls = 0;
        let result = script_goto_state(
            Some(&mut state),
            ScriptStateRequest {
                state: name(701),
                label: None,
            },
            &target(2),
            masks(1 << 16),
            &[None, Some(name(701))],
            &mut |_, _| Err("base must be skipped".into()),
            &mut |s, n| {
                calls += 1;
                assert_eq!(n.resolved_index, 100);
                assert_eq!(s.unwrap().latent_action, 8);
                Ok(false)
            },
        )
        .unwrap();
        assert_eq!(result.state_result, None);
        assert_eq!(result.label_result, Some(false));
        assert_eq!(result.warning, None);
        assert_eq!(calls, 1);
        assert_eq!(state.code, Some(17));
    }
    #[test]
    fn success_labels_but_preempted_switch_does_not_label_again() {
        for preempt in [false, true] {
            let mut state = execution();
            let mut labels = 0;
            let result = script_goto_state(
                Some(&mut state),
                ScriptStateRequest {
                    state: name(702),
                    label: None,
                },
                &target(2),
                masks(1 << 16),
                &[None, Some(name(701))],
                &mut |s, _| {
                    if preempt {
                        s.object_flags |= STATE_CHANGED;
                    }
                    Ok(())
                },
                &mut |_, _| {
                    labels += 1;
                    Ok(true)
                },
            )
            .unwrap();
            assert_eq!(
                result.state_result,
                Some(if preempt {
                    GotoStateResult::Preempted
                } else {
                    GotoStateResult::Success
                })
            );
            assert_eq!(labels, usize::from(!preempt));
        }
    }
    #[test]
    fn missing_state_warning_excludes_none_and_auto_requests() {
        for index in [0, 690, 999] {
            let result = script_goto_state(
                None,
                ScriptStateRequest {
                    state: name(index),
                    label: None,
                },
                &target(2),
                masks(0),
                &[],
                &mut |_, _| Err("no frame".into()),
                &mut |_, _| Ok(false),
            )
            .unwrap();
            assert_eq!(
                result.warning,
                if index == 999 {
                    Some(ScriptStateWarning::MissingState)
                } else {
                    None
                }
            );
        }
    }
    #[test]
    fn only_non_none_explicit_labels_produce_label_warning() {
        for label_name in [None, Some(name(0)), Some(name(100)), Some(name(900))] {
            let result = script_goto_state(
                None,
                ScriptStateRequest {
                    state: name(0),
                    label: label_name,
                },
                &target(2),
                masks(0),
                &[],
                &mut |_, _| Err("same None".into()),
                &mut |_, n| {
                    assert_eq!(
                        n.resolved_index,
                        label_name
                            .filter(|n| n.handle != 0)
                            .map_or(100, |n| n.resolved_index)
                    );
                    Ok(false)
                },
            )
            .unwrap();
            assert_eq!(
                result.warning,
                if label_name.is_some_and(|n| n.handle != 0) {
                    Some(ScriptStateWarning::MissingLabel)
                } else {
                    None
                }
            );
        }
    }
}
