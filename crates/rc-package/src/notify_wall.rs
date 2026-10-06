//! AController.NotifyHitWall wrapper gate and narrowly verified empty base handler.
//! Runtime function resolution, ProcessEvent and nonempty handlers remain external.
use crate::script::{Function, Operand};
use serde::Serialize;

#[derive(Debug, Clone, Copy)]
pub struct EmptyBaseNotifyWall(());
impl EmptyBaseNotifyWall {
    /// Caller must supply the function resolved from Engine's base Controller export,
    /// not a runtime subclass/state override. This recognizes only the reviewed AST.
    pub fn verify(export_path: &str, function: &Function) -> Result<Self, String> {
        let valid = export_path == "Controller.NotifyHitWall"
            && function.friendly_name == "NotifyHitWall"
            && function.native_index == 0
            && function.flags == 0x20800
            && function.logical_script_bytes == 2
            && function.serialized_script_bytes == 2
            && function.expressions.len() == 1;
        if valid {
            let ret = &function.expressions[0];
            if ret.opcode == 4
                && matches!(ret.operand, Operand::None)
                && ret.logical_offset == 0
                && ret.logical_end == 2
                && ret.children.len() == 1
            {
                let nothing = &ret.children[0];
                if nothing.opcode == 0x0b
                    && matches!(nothing.operand, Operand::None)
                    && nothing.logical_offset == 1
                    && nothing.logical_end == 2
                    && nothing.children.is_empty()
                {
                    return Ok(Self(()));
                }
            }
        }
        Err("NotifyHitWall is not the reviewed empty base event".into())
    }
}
#[derive(Debug, Clone, Copy)]
pub enum NotifyWallHandler {
    EmptyBase(EmptyBaseNotifyWall),
    SuppliedResult(bool),
    Unresolved,
}
#[derive(Debug, PartialEq, Eq, Serialize)]
pub enum NotifyWallRoute {
    MaskedOut,
    EmptyBase,
    SuppliedResult,
}
#[derive(Debug, Serialize)]
pub struct NotifyWallResult {
    pub handled: bool,
    pub route: NotifyWallRoute,
}
/// Resolved FName index, not Names table slot 0x161. A missing state frame bypasses
/// the native mask guard. Only indices 300..364 participate in its 64-bit mask.
pub fn notify_hit_wall(
    resolved_name_index: i32,
    state_probe_mask: Option<u64>,
    handler: NotifyWallHandler,
) -> Result<NotifyWallResult, String> {
    if probe_masked_out(resolved_name_index, state_probe_mask) {
        return Ok(NotifyWallResult {
            handled: false,
            route: NotifyWallRoute::MaskedOut,
        });
    }
    let (handled, route) = match handler {
        NotifyWallHandler::EmptyBase(_) => (false, NotifyWallRoute::EmptyBase),
        NotifyWallHandler::SuppliedResult(result) => (result, NotifyWallRoute::SuppliedResult),
        NotifyWallHandler::Unresolved => {
            return Err("enabled NotifyHitWall runtime handler unresolved".into())
        }
    };
    Ok(NotifyWallResult { handled, route })
}
pub(crate) fn probe_masked_out(index: i32, mask: Option<u64>) -> bool {
    (300..364).contains(&index) && mask.is_some_and(|mask| mask & (1u64 << (index - 300)) == 0)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn both_mask_words_and_range_boundaries() {
        for index in [300, 331, 332, 353, 363] {
            assert_eq!(
                notify_hit_wall(index, Some(0), NotifyWallHandler::Unresolved)
                    .unwrap()
                    .route,
                NotifyWallRoute::MaskedOut
            );
            assert!(notify_hit_wall(
                index,
                Some(1u64 << (index - 300)),
                NotifyWallHandler::Unresolved
            )
            .is_err());
        }
        for index in [-1, 299, 364] {
            assert!(notify_hit_wall(index, Some(0), NotifyWallHandler::Unresolved).is_err());
        }
        assert!(notify_hit_wall(353, None, NotifyWallHandler::Unresolved).is_err());
    }
    #[test]
    fn supplied_handler_runs_only_when_enabled() {
        for handled in [true, false] {
            let result =
                notify_hit_wall(353, None, NotifyWallHandler::SuppliedResult(handled)).unwrap();
            assert_eq!(result.handled, handled);
            assert_eq!(result.route, NotifyWallRoute::SuppliedResult);
            assert!(
                !notify_hit_wall(353, Some(0), NotifyWallHandler::SuppliedResult(handled))
                    .unwrap()
                    .handled
            );
        }
    }
    #[test]
    fn empty_base_ast_requires_exact_reviewed_shape() {
        use crate::script::Expression;
        let mut function = Function {
            friendly_name: "NotifyHitWall".into(),
            script_offset: 29,
            serialized_script_bytes: 2,
            logical_script_bytes: 2,
            native_index: 0,
            precedence: 0,
            flags: 0x20800,
            replication_offset: None,
            expressions: vec![Expression {
                logical_offset: 0,
                serialized_offset: 0,
                logical_end: 2,
                opcode: 4,
                operand: Operand::None,
                children: vec![Expression {
                    logical_offset: 1,
                    serialized_offset: 1,
                    logical_end: 2,
                    opcode: 11,
                    operand: Operand::None,
                    children: vec![],
                }],
            }],
        };
        let verified = EmptyBaseNotifyWall::verify("Controller.NotifyHitWall", &function).unwrap();
        let result = notify_hit_wall(353, None, NotifyWallHandler::EmptyBase(verified)).unwrap();
        assert!(!result.handled);
        assert_eq!(result.route, NotifyWallRoute::EmptyBase);
        assert!(EmptyBaseNotifyWall::verify("Derived.NotifyHitWall", &function).is_err());
        function.expressions[0].children[0].opcode = 0x27;
        assert!(EmptyBaseNotifyWall::verify("Controller.NotifyHitWall", &function).is_err());
    }
}
