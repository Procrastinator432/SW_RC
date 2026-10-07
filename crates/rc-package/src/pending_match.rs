//! Exact reviewed DMGame.PendingMatch.BeginState scalar semantics on host fields.
//! Not a VM or ProcessEvent implementation. Caller proves runtime handler identity.
use crate::{
    classes::Field,
    script::{Expression, Function, Operand},
};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PendingMatchSnapshot {
    pub waiting_to_start_match: bool,
    pub startup_stage: u8,
}
#[derive(Debug, Clone, Copy)]
pub struct PendingMatchBegin(());

impl PendingMatchBegin {
    /// Inputs must come from MPGame.DMGame's resolved handler, Engine.GameInfo's
    /// waiting field and MPGame.DMGame's startup field. No subclass overrides.
    pub fn verify(
        qualified_path: &str,
        function: &Function,
        waiting_field: &Field,
        startup_field: &Field,
    ) -> Result<Self, String> {
        let property = |field: &Field, name: &str, kind: &str| {
            field.name == name
                && field.kind == kind
                && field.dimension == 1
                && field.flags & 0x20 == 0
                && field.struct_name.is_none()
        };
        if qualified_path != "mpgame.DMGame.PendingMatch.BeginState"
            || function.friendly_name != "BeginState"
            || function.native_index != 0
            || function.precedence != 0
            || function.flags != 0x20002
            || function.replication_offset.is_some()
            || function.logical_script_bytes != 18
            || function.serialized_script_bytes != 14
            || function.expressions.len() != 3
            || !property(waiting_field, "bWaitingToStartMatch", "Core.BoolProperty")
            || !property(startup_field, "StartupStage", "Core.ByteProperty")
        {
            return Err("PendingMatch BeginState metadata differs from reviewed handler".into());
        }
        let mut nodes = Vec::new();
        let mut pending: Vec<&Expression> = function.expressions.iter().rev().collect();
        while let Some(e) = pending.pop() {
            if nodes.len() >= 9 || e.children.len() > 2 {
                return Err("PendingMatch BeginState AST exceeds reviewed shape".into());
            }
            nodes.push(e);
            pending.extend(e.children.iter().rev());
        }
        let shape = [
            (0x14, 0, 8, 0, 2),
            (0x2d, 1, 7, 1, 1),
            (1, 2, 7, 2, 0),
            (0x27, 7, 8, 5, 0),
            (0x0f, 8, 16, 6, 2),
            (1, 9, 14, 7, 0),
            (0x24, 14, 16, 10, 0),
            (4, 16, 18, 12, 1),
            (0x0b, 17, 18, 13, 0),
        ];
        let valid = nodes.len()==shape.len() && nodes.iter().zip(shape).enumerate().all(|(i,(e,(opcode,start,end,serialized,children)))| {
            let operand = match i {
                2 => matches!(&e.operand,Operand::Object {index:-257,path} if path=="Engine.GameInfo.bWaitingToStartMatch"),
                5 => matches!(&e.operand,Operand::Object {index:576,path} if path=="DMGame.StartupStage"),
                6 => matches!(e.operand,Operand::Byte(0)),
                _ => matches!(e.operand,Operand::None),
            };
            e.opcode==opcode && e.logical_offset==start && e.logical_end==end
                && e.serialized_offset==serialized && e.children.len()==children && operand
        });
        if !valid {
            return Err("PendingMatch BeginState AST differs from reviewed handler".into());
        }
        Ok(Self(()))
    }
    pub fn execute(self, snapshot: &mut PendingMatchSnapshot) {
        snapshot.waiting_to_start_match = true;
        snapshot.startup_stage = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Function {
        serde_json::from_str(include_str!("../tests/fixtures/pending_match_begin.json")).unwrap()
    }
    fn fields() -> [Field; 2] {
        [
            Field {
                name: "bWaitingToStartMatch".into(),
                kind: "Core.BoolProperty".into(),
                dimension: 1,
                flags: 0,
                struct_name: None,
            },
            Field {
                name: "StartupStage".into(),
                kind: "Core.ByteProperty".into(),
                dimension: 1,
                flags: 0,
                struct_name: None,
            },
        ]
    }
    fn verify(f: &Function) -> Result<PendingMatchBegin, String> {
        let fields = fields();
        PendingMatchBegin::verify(
            "mpgame.DMGame.PendingMatch.BeginState",
            f,
            &fields[0],
            &fields[1],
        )
    }
    #[test]
    fn original_ast_sets_both_fields_for_every_input_and_is_idempotent() {
        let token = verify(&fixture()).unwrap();
        for waiting in [false, true] {
            for stage in 0..=u8::MAX {
                let mut state = PendingMatchSnapshot {
                    waiting_to_start_match: waiting,
                    startup_stage: stage,
                };
                token.execute(&mut state);
                token.execute(&mut state);
                assert_eq!(
                    state,
                    PendingMatchSnapshot {
                        waiting_to_start_match: true,
                        startup_stage: 0
                    }
                );
            }
        }
    }
    #[test]
    fn changed_metadata_operands_tree_and_property_types_fail_closed() {
        for mutate in [
            |f: &mut Function| f.native_index = 1,
            |f: &mut Function| f.flags = 0,
            |f: &mut Function| f.logical_script_bytes = 17,
            |f: &mut Function| f.expressions[0].children[1].opcode = 0x28,
            |f: &mut Function| f.expressions[1].children[1].operand = Operand::Byte(1),
            |f: &mut Function| {
                f.expressions[1].children[0].operand = Operand::Object {
                    index: 576,
                    path: "Other.StartupStage".into(),
                }
            },
            |f: &mut Function| f.expressions[2].children.clear(),
            |f: &mut Function| f.expressions[0].logical_end = 9,
        ] {
            let mut f = fixture();
            mutate(&mut f);
            assert!(verify(&f).is_err());
        }
        let f = fixture();
        let mut fields = fields();
        fields[0].flags |= 0x20;
        assert!(PendingMatchBegin::verify(
            "mpgame.DMGame.PendingMatch.BeginState",
            &f,
            &fields[0],
            &fields[1]
        )
        .is_err());
        fields[0].flags = 0;
        fields[1].kind = "Core.IntProperty".into();
        assert!(PendingMatchBegin::verify(
            "mpgame.DMGame.PendingMatch.BeginState",
            &f,
            &fields[0],
            &fields[1]
        )
        .is_err());
        assert!(
            PendingMatchBegin::verify("Override.BeginState", &f, &fields[0], &fields[1]).is_err()
        );
    }
    #[test]
    fn verified_callback_completes_base_transition_without_redirect() {
        use crate::{
            event_lookup::EventNameSnapshot,
            probe_frame::{ProbeFrame, StateProbeMasks},
            state_selection::{NamedStateTarget, ResolvedStateTarget, StateTargetRoute},
            state_transition::{
                goto_state, GotoStateResult, StateEvent, StateExecution, STATE_CHANGED,
            },
        };
        let token = verify(&fixture()).unwrap();
        let mut scalar = PendingMatchSnapshot {
            waiting_to_start_match: false,
            startup_stage: 255,
        };
        let mut execution = StateExecution {
            probe: ProbeFrame::init_execution(0),
            node: 0,
            code: Some(123),
            latent_action: 7,
            object_flags: 0,
        };
        let target = ResolvedStateTarget {
            selection: NamedStateTarget {
                state_node: 1,
                route: StateTargetRoute::NamedState,
            },
            name: EventNameSnapshot {
                handle: 99,
                resolved_index: 98,
            },
        };
        let result = goto_state(
            Some(&mut execution),
            &target,
            StateProbeMasks {
                class_probe: 0,
                state_probe: 1 << 16,
                state_ignore: u64::MAX,
            },
            &[],
            &mut |s, event| {
                assert_eq!(event, StateEvent::BeginState);
                assert_eq!(s.probe.state_node, 1);
                token.execute(&mut scalar);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(result, GotoStateResult::Success);
        assert_eq!(execution.object_flags, STATE_CHANGED);
        assert_eq!(
            scalar,
            PendingMatchSnapshot {
                waiting_to_start_match: true,
                startup_stage: 0
            }
        );
    }
}
