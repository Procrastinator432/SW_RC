//! Reviewed Prop BeginState branch semantics. Broadcast and GotoState are explicit
//! actions for the host; no string conversion, game references, or generic VM.
use crate::{
    classes::Field,
    event_lookup::EventNameSnapshot,
    script::{Expression, Function, Operand},
};
use serde::Serialize;
enum ExpectedOperand {
    None,
    Object(i32, &'static str),
    Name(&'static str),
    String(&'static str),
    Native(u16),
    Target(u16),
    Byte(u8),
    Context(u16, u8),
}
struct Shape {
    opcode: u8,
    start: u32,
    end: u32,
    serialized: usize,
    children: usize,
    operand: ExpectedOperand,
}
include!("prop_begin_shapes.rs");

#[derive(Clone, Copy, Debug, Serialize)]
pub struct PropBeginSnapshot {
    pub health: i32,
    pub display_state_messages: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum PropBeginAction {
    Return,
    GotoDamagable,
    BroadcastRequired(&'static str),
}
#[derive(Clone, Copy)]
pub struct PropBegin {
    invulnerable: bool,
}
impl PropBegin {
    pub fn verify(
        path: &str,
        function: &Function,
        health: &Field,
        display: &Field,
    ) -> Result<Self, String> {
        let (invulnerable, shape, logical, serialized) = match path {
            "Prop.Invulnerable.BeginState" => (true, INVULNERABLE, 84, 74),
            "Prop.Damagable.BeginState" => (false, DAMAGABLE, 60, 53),
            _ => return Err("Prop BeginState handler identity unresolved".into()),
        };
        if health.name != "Health"
            || health.kind != "Core.IntProperty"
            || health.dimension != 1
            || display.name != "bDisplayStateMessages"
            || display.kind != "Core.BoolProperty"
            || display.dimension != 1
            || function.friendly_name != "BeginState"
            || function.native_index != 0
            || function.precedence != 0
            || function.flags != 0x20002
            || function.replication_offset.is_some()
            || function.logical_script_bytes != logical
            || function.serialized_script_bytes != serialized
            || function.expressions.len() != if invulnerable { 6 } else { 3 }
        {
            return Err("Prop BeginState metadata differs from reviewed original".into());
        }
        check_shape(function, shape)?;
        Ok(Self { invulnerable })
    }
    pub fn action(self, snapshot: PropBeginSnapshot) -> PropBeginAction {
        if self.invulnerable && snapshot.health > 0 {
            PropBeginAction::GotoDamagable
        } else if snapshot.display_state_messages {
            PropBeginAction::BroadcastRequired(if self.invulnerable {
                " Is Invulnerable"
            } else {
                " Is Damagable"
            })
        } else {
            PropBeginAction::Return
        }
    }
}
fn check_shape(function: &Function, shape: &[Shape]) -> Result<(), String> {
    let mut todo: Vec<&Expression> = function.expressions.iter().rev().collect();
    let mut count = 0;
    while let Some(e) = todo.pop() {
        let expected = shape.get(count).ok_or("Prop AST exceeds reviewed shape")?;
        count += 1;
        let operand = match expected.operand {
            ExpectedOperand::None => matches!(e.operand, Operand::None),
            ExpectedOperand::Object(index, path) => {
                matches!(&e.operand,Operand::Object {index:i,path:p} if *i==index&&p==path)
            }
            ExpectedOperand::Name(v) => matches!(&e.operand,Operand::Name(n) if n==v),
            ExpectedOperand::String(v) => matches!(&e.operand,Operand::String(n) if n==v),
            ExpectedOperand::Native(v) => matches!(e.operand,Operand::Native(n) if n==v),
            ExpectedOperand::Target(v) => matches!(e.operand,Operand::Target(n) if n==v),
            ExpectedOperand::Byte(v) => matches!(e.operand,Operand::Byte(n) if n==v),
            ExpectedOperand::Context(skip, result) => {
                matches!(e.operand,Operand::Context {skip:s,result_size:r} if s==skip&&r==result)
            }
        };
        if !operand
            || e.opcode != expected.opcode
            || e.logical_offset != expected.start
            || e.logical_end != expected.end
            || e.serialized_offset != expected.serialized
            || e.children.len() != expected.children
        {
            return Err("Prop BeginState AST differs from reviewed original".into());
        }
        todo.extend(e.children.iter().rev());
    }
    if count != shape.len() {
        return Err("Prop BeginState AST incomplete".into());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct HealthyAnimationSnapshot {
    pub name: EventNameSnapshot,
    pub looping: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum AnimationKind {
    PlayAnim,
    LoopAnim,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct AnimationRequest {
    pub kind: AnimationKind,
    pub name: EventNameSnapshot,
    pub native_index: u16,
}
#[derive(Clone, Copy)]
pub struct AnimPropBegin(());
impl AnimPropBegin {
    /// Caller supplies the resolved Engine.AnimProp handler and field definitions.
    pub fn verify(
        path: &str,
        function: &Function,
        healthy: &Field,
        name: &Field,
        looping: &Field,
    ) -> Result<Self, String> {
        if path != "AnimProp.Invulnerable.BeginState"
            || function.friendly_name != "BeginState"
            || function.native_index != 0
            || function.precedence != 0
            || function.flags != 0x20002
            || function.replication_offset.is_some()
            || function.logical_script_bytes != 71
            || function.serialized_script_bytes != 59
            || function.expressions.len() != 7
            || healthy.name != "AnimHealthy"
            || healthy.kind != "Core.StructProperty"
            || healthy.dimension != 1
            || healthy.struct_name.as_deref() != Some("AnimProp.PropAnimInfo")
            || name.name != "Anim"
            || name.kind != "Core.NameProperty"
            || name.dimension != 1
            || looping.name != "bLoop"
            || looping.kind != "Core.BoolProperty"
            || looping.dimension != 1
        {
            return Err("AnimProp BeginState metadata differs from reviewed original".into());
        }
        check_shape(function, ANIM_INVULNERABLE)?;
        Ok(Self(()))
    }
    /// Super must execute the fixed reviewed Prop handler and return the healthy
    /// animation fields read afterwards. A successful state redirect does not
    /// terminate this function. Errors stop before the tail; requests are external
    /// native operations, without modeled optional defaults or animation playback.
    pub fn run(
        self,
        super_begin: impl FnOnce() -> Result<HealthyAnimationSnapshot, String>,
        animation: &mut impl FnMut(AnimationRequest) -> Result<(), String>,
    ) -> Result<(), String> {
        let healthy = super_begin()?;
        if healthy.name.handle != 0 {
            animation(AnimationRequest {
                kind: if healthy.looping {
                    AnimationKind::LoopAnim
                } else {
                    AnimationKind::PlayAnim
                },
                name: healthy.name,
                native_index: if healthy.looping { 260 } else { 259 },
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn anim_fixture() -> Function {
        serde_json::from_str(include_str!("../tests/fixtures/anim_prop_begin.json")).unwrap()
    }
    fn anim_fields() -> [Field; 3] {
        [
            Field {
                name: "AnimHealthy".into(),
                kind: "Core.StructProperty".into(),
                dimension: 1,
                flags: 1,
                struct_name: Some("AnimProp.PropAnimInfo".into()),
            },
            Field {
                name: "Anim".into(),
                kind: "Core.NameProperty".into(),
                dimension: 1,
                flags: 1,
                struct_name: None,
            },
            Field {
                name: "bLoop".into(),
                kind: "Core.BoolProperty".into(),
                dimension: 1,
                flags: 1,
                struct_name: None,
            },
        ]
    }
    fn anim_handler() -> AnimPropBegin {
        let f = anim_fields();
        AnimPropBegin::verify(
            "AnimProp.Invulnerable.BeginState",
            &anim_fixture(),
            &f[0],
            &f[1],
            &f[2],
        )
        .unwrap()
    }
    #[test]
    fn anim_super_error_stops_before_any_animation_request() {
        let error = anim_handler()
            .run(|| Err("unresolved super Broadcast".into()), &mut |_| {
                Err("tail ran".into())
            })
            .unwrap_err();
        assert_eq!(error, "unresolved super Broadcast");
    }
    #[test]
    fn anim_tail_uses_fields_after_super_and_continues_after_state_redirect() {
        use std::cell::Cell;
        let redirected = Cell::new(false);
        for looping in [false, true] {
            let mut calls = 0;
            anim_handler()
                .run(
                    || {
                        redirected.set(true);
                        Ok(HealthyAnimationSnapshot {
                            name: EventNameSnapshot {
                                handle: 99,
                                resolved_index: 98,
                            },
                            looping,
                        })
                    },
                    &mut |request| {
                        assert!(redirected.get());
                        calls += 1;
                        assert_eq!(
                            request.kind,
                            if looping {
                                AnimationKind::LoopAnim
                            } else {
                                AnimationKind::PlayAnim
                            }
                        );
                        assert_eq!(request.native_index, if looping { 260 } else { 259 });
                        Ok(())
                    },
                )
                .unwrap();
            assert_eq!(calls, 1);
            redirected.set(false);
            anim_handler()
                .run(
                    || {
                        Ok(HealthyAnimationSnapshot {
                            name: EventNameSnapshot {
                                handle: 0,
                                resolved_index: 0,
                            },
                            looping,
                        })
                    },
                    &mut |_| Err("None must skip animation".into()),
                )
                .unwrap();
        }
    }
    #[test]
    fn anim_unknown_native_operation_is_not_silently_successful() {
        let error = anim_handler()
            .run(
                || {
                    Ok(HealthyAnimationSnapshot {
                        name: EventNameSnapshot {
                            handle: 99,
                            resolved_index: 98,
                        },
                        looping: false,
                    })
                },
                &mut |_| Err("native PlayAnim unresolved".into()),
            )
            .unwrap_err();
        assert_eq!(error, "native PlayAnim unresolved");
    }
    #[test]
    fn anim_changed_super_reference_native_index_and_struct_type_fail() {
        let f = anim_fields();
        for mutate in [
            |v: &mut Function| {
                v.expressions[0].operand = Operand::Object {
                    index: 11676,
                    path: "Other.BeginState".into(),
                }
            },
            |v: &mut Function| v.expressions[0].opcode = 0x1b,
            |v: &mut Function| v.expressions[3].operand = Operand::Native(259),
            |v: &mut Function| v.expressions[1].operand = Operand::Target(68),
            |v: &mut Function| v.expressions[6].children.clear(),
        ] {
            let mut v = anim_fixture();
            mutate(&mut v);
            assert!(AnimPropBegin::verify(
                "AnimProp.Invulnerable.BeginState",
                &v,
                &f[0],
                &f[1],
                &f[2]
            )
            .is_err());
        }
        let mut f = anim_fields();
        f[0].struct_name = Some("Other.Struct".into());
        assert!(AnimPropBegin::verify(
            "AnimProp.Invulnerable.BeginState",
            &anim_fixture(),
            &f[0],
            &f[1],
            &f[2]
        )
        .is_err());
    }
    fn fields() -> [Field; 2] {
        [
            Field {
                name: "Health".into(),
                kind: "Core.IntProperty".into(),
                dimension: 1,
                flags: 0,
                struct_name: None,
            },
            Field {
                name: "bDisplayStateMessages".into(),
                kind: "Core.BoolProperty".into(),
                dimension: 1,
                flags: 0,
                struct_name: None,
            },
        ]
    }
    fn fixture(invulnerable: bool) -> Function {
        serde_json::from_str(if invulnerable {
            include_str!("../tests/fixtures/prop_invulnerable_begin.json")
        } else {
            include_str!("../tests/fixtures/prop_damagable_begin.json")
        })
        .unwrap()
    }
    fn verify(invulnerable: bool, f: &Function) -> Result<PropBegin, String> {
        let fields = fields();
        PropBegin::verify(
            if invulnerable {
                "Prop.Invulnerable.BeginState"
            } else {
                "Prop.Damagable.BeginState"
            },
            f,
            &fields[0],
            &fields[1],
        )
    }
    #[test]
    fn signed_health_branch_takes_precedence_over_display_messages() {
        for invulnerable in [false, true] {
            let handler = verify(invulnerable, &fixture(invulnerable)).unwrap();
            for health in [i32::MIN, -1, 0, 1, i32::MAX] {
                for display in [false, true] {
                    let action = handler.action(PropBeginSnapshot {
                        health,
                        display_state_messages: display,
                    });
                    let expected = if invulnerable && health > 0 {
                        PropBeginAction::GotoDamagable
                    } else if display {
                        PropBeginAction::BroadcastRequired(if invulnerable {
                            " Is Invulnerable"
                        } else {
                            " Is Damagable"
                        })
                    } else {
                        PropBeginAction::Return
                    };
                    assert_eq!(action, expected);
                }
            }
        }
    }
    #[test]
    fn altered_string_comparison_jump_tree_and_field_kinds_are_rejected() {
        for mutate in [
            |f: &mut Function| f.expressions[0].children[0].operand = Operand::Native(150),
            |f: &mut Function| f.expressions[0].operand = Operand::Target(20),
            |f: &mut Function| f.expressions[1].children[0].operand = Operand::Name("Other".into()),
            |f: &mut Function| {
                f.expressions[4].children[1].children[1].children[1].operand =
                    Operand::String("altered".into())
            },
            |f: &mut Function| f.expressions[5].children.clear(),
            |f: &mut Function| f.flags = 0,
        ] {
            let mut f = fixture(true);
            mutate(&mut f);
            assert!(verify(true, &f).is_err());
        }
        let mut fields = fields();
        fields[0].kind = "Core.FloatProperty".into();
        assert!(PropBegin::verify(
            "Prop.Invulnerable.BeginState",
            &fixture(true),
            &fields[0],
            &fields[1]
        )
        .is_err());
        assert!(verify(false, &fixture(true)).is_err());
    }
    #[test]
    fn recursive_script_switch_preempts_outer_begin_after_label_attempt() {
        use crate::{
            event_lookup::EventNameSnapshot,
            probe_frame::{ProbeFrame, StateProbeMasks},
            script_state::{script_goto_state, ScriptStateRequest},
            state_selection::{NamedStateTarget, ResolvedStateTarget, StateTargetRoute},
            state_transition::{goto_state, GotoStateResult, StateEvent, StateExecution},
        };
        let invulnerable = verify(true, &fixture(true)).unwrap();
        let damagable = verify(false, &fixture(false)).unwrap();
        let input = PropBeginSnapshot {
            health: 1,
            display_state_messages: false,
        };
        let name = |i| EventNameSnapshot {
            handle: i + 1,
            resolved_index: i as i32,
        };
        let target = |node| ResolvedStateTarget {
            selection: NamedStateTarget {
                state_node: node,
                route: StateTargetRoute::NamedState,
            },
            name: name(node as u32 + 700),
        };
        let names = [None, Some(name(701)), Some(name(702))];
        let masks = StateProbeMasks {
            class_probe: 0,
            state_probe: 1 << 16,
            state_ignore: u64::MAX,
        };
        let mut execution = StateExecution {
            probe: ProbeFrame::init_execution(0),
            node: 0,
            code: None,
            latent_action: 7,
            object_flags: 0,
        };
        let mut labels = 0;
        let result = goto_state(
            Some(&mut execution),
            &target(1),
            masks,
            &names,
            &mut |s, event| {
                assert_eq!(event, StateEvent::BeginState);
                assert_eq!(invulnerable.action(input), PropBeginAction::GotoDamagable);
                let inner = script_goto_state(
                    Some(s),
                    ScriptStateRequest {
                        state: name(702),
                        label: None,
                    },
                    &target(2),
                    masks,
                    &names,
                    &mut |_, e| {
                        assert_eq!(e, StateEvent::BeginState);
                        assert_eq!(damagable.action(input), PropBeginAction::Return);
                        Ok(())
                    },
                    &mut |s, label| {
                        labels += 1;
                        assert_eq!(label.resolved_index, 100);
                        s.unwrap().code = None;
                        Ok(false)
                    },
                )?;
                assert_eq!(inner.state_result, Some(GotoStateResult::Success));
                assert_eq!(inner.warning, None);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(result, GotoStateResult::Preempted);
        assert_eq!(execution.probe.state_node, 2);
        assert_eq!(labels, 1);
    }
}
