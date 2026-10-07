//! Reviewed probe-mask portion of FStateFrame, not a complete GotoState/VM model.
use crate::{
    event_lookup::{EventLookupSnapshot, EventNameSnapshot},
    notify_wall::NotifyWallResult,
};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ProbeFrame {
    pub object_class: usize,
    pub state_node: usize,
    pub probe_mask: u64,
}
#[derive(Clone, Copy)]
pub struct StateProbeMasks {
    pub class_probe: u64,
    pub state_probe: u64,
    /// Native applies this word with AND, without complementing it.
    pub state_ignore: u64,
}
impl StateProbeMasks {
    pub fn allowed(self) -> u64 {
        (self.class_probe | self.state_probe) & self.state_ignore
    }
}
fn probe_bit(index: i32) -> Result<u64, String> {
    if !(300..364).contains(&index) {
        return Err("name is not a probe function".into());
    }
    Ok(1u64 << (index - 300))
}
impl ProbeFrame {
    /// InitExecution sets StateNode to the object class and both mask words to -1.
    /// No statement about other FStateFrame fields or constructor flags.
    pub fn init_execution(object_class: usize) -> Self {
        Self {
            object_class,
            state_node: object_class,
            probe_mask: u64::MAX,
        }
    }
    /// Only GotoState's resolved-state mask assignment. Caller must finish native
    /// target selection and EndState handling first. No callback/transition runs.
    pub fn resolved_state_masks(self, state_node: usize, masks: StateProbeMasks) -> Self {
        Self {
            state_node,
            probe_mask: masks.allowed(),
            ..self
        }
    }
    pub fn disable(&mut self, index: i32) -> Result<(), String> {
        self.probe_mask &= !probe_bit(index)?;
        Ok(())
    }
    /// Enable restores only bits permitted by the current class/state/ignore masks.
    /// Invalid probe names error here; native logs and leaves the mask unchanged.
    pub fn enable(&mut self, index: i32, masks: StateProbeMasks) -> Result<(), String> {
        self.probe_mask |= probe_bit(index)? & masks.allowed();
        Ok(())
    }
    pub fn notify_hit_wall(
        self,
        graph: &EventLookupSnapshot,
        name: EventNameSnapshot,
    ) -> Result<NotifyWallResult, String> {
        graph.notify_hit_wall(
            self.object_class,
            Some(self.state_node),
            name,
            Some(self.probe_mask),
        )
    }
}
/// Native IsProbing returns true for names outside the range or absent state frames.
pub fn is_probing(index: i32, frame: Option<&ProbeFrame>) -> bool {
    !crate::notify_wall::probe_masked_out(index, frame.map(|f| f.probe_mask))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn init_and_resolved_mask_assignment_are_distinct() {
        let initial = ProbeFrame::init_execution(7);
        assert_eq!(initial.state_node, 7);
        assert_eq!(initial.probe_mask, u64::MAX);
        let masks = StateProbeMasks {
            class_probe: (1u64 << 53) | 1,
            state_probe: 1u64 << 63,
            state_ignore: !1,
        };
        let selected = initial.resolved_state_masks(9, masks);
        assert_eq!(selected.object_class, 7);
        assert_eq!(selected.state_node, 9);
        assert_eq!(selected.probe_mask, (1u64 << 53) | (1u64 << 63));
        assert_eq!(initial.probe_mask, u64::MAX);
    }
    #[test]
    fn disable_enable_and_invalid_names_preserve_other_bits() {
        let mut frame = ProbeFrame::init_execution(0);
        for index in [300, 331, 332, 353, 363] {
            let bit = 1u64 << (index - 300);
            frame.disable(index).unwrap();
            assert!(!is_probing(index, Some(&frame)));
            let blocked = StateProbeMasks {
                class_probe: u64::MAX,
                state_probe: 0,
                state_ignore: !bit,
            };
            frame.enable(index, blocked).unwrap();
            assert_eq!(frame.probe_mask, !bit);
            let allowed = StateProbeMasks {
                state_ignore: u64::MAX,
                ..blocked
            };
            frame.enable(index, allowed).unwrap();
            assert_eq!(frame.probe_mask, u64::MAX);
        }
        let before = frame;
        for index in [-1, 299, 364] {
            assert!(frame.disable(index).is_err());
            assert!(frame
                .enable(
                    index,
                    StateProbeMasks {
                        class_probe: 0,
                        state_probe: 0,
                        state_ignore: 0
                    }
                )
                .is_err());
            assert_eq!(frame, before);
            assert!(is_probing(index, Some(&frame)));
        }
        assert!(is_probing(353, None));
        frame.probe_mask = 0;
        frame
            .enable(
                353,
                StateProbeMasks {
                    class_probe: 0,
                    state_probe: 0,
                    state_ignore: u64::MAX,
                },
            )
            .unwrap();
        assert_eq!(frame.probe_mask, 0);
    }
}
