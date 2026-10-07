//! Verified fixed native FName indices; opaque handles for own diagnostic tables.
//! Dynamic name bindings remain diagnostic, without claiming native load order.
use crate::event_lookup::EventNameSnapshot;
use std::collections::{BTreeMap, BTreeSet};
mod fixed {
    include!("hardcoded_names.rs");
}
pub use fixed::HARDCODED_NAMES;
#[derive(Clone, Copy)]
pub struct NameBinding {
    pub name: EventNameSnapshot,
    pub fixed_native_index: bool,
}
/// The index is native; the nonzero handle is an opaque token, not a PE address.
/// Native FName(None), including an empty string, uses a null handle.
pub fn hardcoded_name(name: &str) -> Option<EventNameSnapshot> {
    let name = if name.is_empty() { "None" } else { name };
    HARDCODED_NAMES
        .iter()
        .find(|(_, fixed)| name.eq_ignore_ascii_case(fixed))
        .map(|&(index, _)| EventNameSnapshot {
            resolved_index: index,
            handle: if index == 0 { 0 } else { index as u32 + 1 },
        })
}
/// Keep verified fixed indices; assign all remaining names diagnostic indices
/// above the maximum fixed index. This is not native dynamic registration.
pub fn diagnostic_bindings(
    names: &BTreeSet<String>,
) -> Result<BTreeMap<String, NameBinding>, String> {
    let mut canonical = BTreeSet::new();
    for name in names {
        if !name.is_ascii() || name.contains('\0') {
            return Err("diagnostic FName binding supports ASCII without NUL only".into());
        }
        canonical.insert(if name.is_empty() {
            "none".into()
        } else {
            name.to_ascii_lowercase()
        });
    }
    let mut next = HARDCODED_NAMES
        .iter()
        .map(|&(index, _)| index)
        .max()
        .ok_or("missing fixed names")?
        + 1;
    let mut bindings = BTreeMap::new();
    for key in canonical {
        let binding = if let Some(name) = hardcoded_name(&key) {
            NameBinding {
                name,
                fixed_native_index: true,
            }
        } else {
            let index = next;
            next = next
                .checked_add(1)
                .ok_or("diagnostic name index overflow")?;
            NameBinding {
                name: EventNameSnapshot {
                    handle: index as u32 + 1,
                    resolved_index: index,
                },
                fixed_native_index: false,
            }
        };
        bindings.insert(key, binding);
    }
    Ok(bindings)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::notify_wall::{notify_hit_wall, NotifyWallHandler};
    #[test]
    fn fixed_notify_name_drives_high_mask_word_and_none_is_null() {
        let name = hardcoded_name("notifyhitwall").unwrap();
        assert_eq!(name.resolved_index, 353);
        assert_eq!(name.resolved_index & 127, 97);
        assert_ne!(name.handle, 0);
        assert!(
            notify_hit_wall(
                name.resolved_index,
                Some(1u64 << 53),
                NotifyWallHandler::SuppliedResult(true)
            )
            .unwrap()
            .handled
        );
        assert!(
            !notify_hit_wall(
                name.resolved_index,
                Some(1u64 << 21),
                NotifyWallHandler::SuppliedResult(true)
            )
            .unwrap()
            .handled
        );
        for key in ["None", "NONE", ""] {
            let name = hardcoded_name(key).unwrap();
            assert_eq!(name.handle, 0);
            assert_eq!(name.resolved_index, 0);
        }
        assert!(hardcoded_name("NotAFixedName123").is_none());
    }
    #[test]
    fn diagnostic_unknowns_never_reuse_fixed_slots_and_ascii_names_share_identity() {
        let names = BTreeSet::from([
            "NotifyHitWall".into(),
            "notifyhitwall".into(),
            "ZZUnknownName".into(),
            "AAUnknownName".into(),
        ]);
        let bindings = diagnostic_bindings(&names).unwrap();
        assert_eq!(bindings.len(), 3);
        assert!(bindings["notifyhitwall"].fixed_native_index);
        assert_eq!(bindings["notifyhitwall"].name.resolved_index, 353);
        assert_eq!(bindings["aaunknownname"].name.resolved_index, 937);
        assert_eq!(bindings["zzunknownname"].name.resolved_index, 938);
        assert!(!bindings["aaunknownname"].fixed_native_index);
        for name in ["héllo", "bad\0name"] {
            assert!(diagnostic_bindings(&BTreeSet::from([name.into()])).is_err());
        }
    }
}
