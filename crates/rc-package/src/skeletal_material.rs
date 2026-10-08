//! ULodMeshInstance::GetMaterial, 1044f750, for nonnegative material slots.
/// `Some(actor_skins)` denotes an actor, including when the array is empty.
/// The override virtual call is deliberately repeated on a non-null result.
pub fn select_lod_material<T: Clone>(
    slot: usize,
    mesh: &[Option<T>],
    actor_skins: Option<&[Option<T>]>,
    mut actor_override: impl FnMut(usize) -> Option<T>,
) -> Option<T> {
    if actor_skins.is_some() && actor_override(slot).is_some() {
        return actor_override(slot);
    }
    if slot == 0 || slot >= mesh.len() {
        if let Some(skins) = actor_skins {
            if !skins.is_empty() {
                return skins[0].clone();
            }
        }
        if slot >= mesh.len() {
            return mesh.first().cloned().flatten();
        }
    }
    mesh[slot].clone()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absent_actor_never_dispatches_override() {
        assert_eq!(
            select_lod_material(1, &[Some(1), Some(2)], None, |_| panic!()),
            Some(2)
        );
    }
    #[test]
    fn override_calls_twice_and_returns_second_result() {
        let mut n = 0;
        assert_eq!(
            select_lod_material(0, &[Some(1)], Some(&[]), |_| {
                n += 1;
                Some(n)
            }),
            Some(2)
        );
        assert_eq!(n, 2);
    }
    #[test]
    fn second_override_can_become_null() {
        let mut n = 0;
        assert_eq!(
            select_lod_material(0, &[Some(1)], Some(&[]), |_| {
                n += 1;
                if n == 1 {
                    Some(9)
                } else {
                    None
                }
            }),
            None
        );
    }
    #[test]
    fn slot_zero_prefers_first_actor_skin() {
        assert_eq!(
            select_lod_material(0, &[Some(1)], Some(&[Some(8)]), |_| None),
            Some(8)
        );
    }
    #[test]
    fn in_range_nonzero_slot_uses_mesh_even_when_null() {
        assert_eq!(
            select_lod_material(1, &[Some(1), None], Some(&[Some(8)]), |_| None),
            None
        );
    }
    #[test]
    fn out_of_range_prefers_actor_first_including_null() {
        assert_eq!(
            select_lod_material(55, &[Some(1)], Some(&[None]), |_| None),
            None
        );
    }
    #[test]
    fn out_of_range_without_actor_skin_falls_back_to_mesh_first() {
        assert_eq!(
            select_lod_material(99, &[Some(1)], Some(&[]), |_| None),
            Some(1)
        );
    }
    #[test]
    fn empty_mesh_returns_null_or_actor_skin() {
        assert_eq!(select_lod_material::<i32>(0, &[], None, |_| None), None);
        assert_eq!(
            select_lod_material(0, &[], Some(&[Some(8)]), |_| None),
            Some(8)
        );
    }
}
