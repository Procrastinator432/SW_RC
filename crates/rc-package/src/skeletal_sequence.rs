//! ULodMeshInstance::GetSequence, engine 1044faf0; opaque runtime sequence identities.
use crate::mesh_animation::AnimationChannel;
pub trait SequenceLookupHost {
    fn find_sequence(&mut self, name_handle: u32, load: bool) -> Result<u32, String>;
}
/// Supplied resolved bindings; does not invent native dynamic name registration or asset loading.
pub struct SnapshotSequenceLookup<'a> {
    pub bindings: &'a [(u32, u32)],
}
impl SequenceLookupHost for SnapshotSequenceLookup<'_> {
    fn find_sequence(&mut self, name: u32, _: bool) -> Result<u32, String> {
        Ok(self
            .bindings
            .iter()
            .find(|(n, _)| *n == name)
            .map_or(0, |(_, s)| *s))
    }
}
/// Null sequence is identity 0. Noneditor returns cached identity without validation/lookup.
pub fn get_sequence(
    channels: &mut [AnimationChannel],
    index: i32,
    editor: bool,
    host: &mut impl SequenceLookupHost,
) -> Result<u32, String> {
    let index = usize::try_from(index).map_err(|_| "negative sequence channel index")?;
    let c = channels
        .get_mut(index)
        .ok_or("sequence channel index out of range")?;
    if editor {
        c.words[17] = host.find_sequence(c.words[0], false)?;
    }
    Ok(c.words[17])
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Host {
        calls: Vec<(u32, bool)>,
        result: Result<u32, String>,
    }
    impl SequenceLookupHost for Host {
        fn find_sequence(&mut self, n: u32, l: bool) -> Result<u32, String> {
            self.calls.push((n, l));
            self.result.clone()
        }
    }
    fn channel() -> AnimationChannel {
        let mut c = AnimationChannel::default();
        c.words[0] = 11;
        c.words[17] = 99;
        c
    }
    #[test]
    fn noneditor_reads_raw_cached_identity_without_host() {
        let mut c = [channel()];
        let mut h = Host {
            calls: vec![],
            result: Err("unavailable".into()),
        };
        assert_eq!(get_sequence(&mut c, 0, false, &mut h).unwrap(), 99);
        assert!(h.calls.is_empty());
    }
    #[test]
    fn editor_refreshes_even_cached_identity_and_can_store_null() {
        let mut c = [channel()];
        let mut h = Host {
            calls: vec![],
            result: Ok(0),
        };
        assert_eq!(get_sequence(&mut c, 0, true, &mut h).unwrap(), 0);
        assert_eq!(c[0].words[17], 0);
        assert_eq!(h.calls, [(11, false)]);
    }
    #[test]
    fn lookup_failure_keeps_cached_value_and_invalid_indices_skip_host() {
        let mut c = [channel()];
        let mut h = Host {
            calls: vec![],
            result: Err("lookup boundary".into()),
        };
        assert!(get_sequence(&mut c, 0, true, &mut h).is_err());
        assert_eq!(c[0].words[17], 99);
        assert!(get_sequence(&mut c, -1, true, &mut h).is_err());
        assert!(get_sequence(&mut c, 1, true, &mut h).is_err());
        assert_eq!(h.calls, [(11, false)]);
    }
    #[test]
    fn resolved_bindings_use_first_exact_identity_match() {
        let mut h = SnapshotSequenceLookup {
            bindings: &[(11, 77), (11, 88), (12, 99)],
        };
        let mut c = [channel()];
        assert_eq!(get_sequence(&mut c, 0, true, &mut h).unwrap(), 77);
        c[0].words[0] = 13;
        assert_eq!(get_sequence(&mut c, 0, true, &mut h).unwrap(), 0);
    }
}
