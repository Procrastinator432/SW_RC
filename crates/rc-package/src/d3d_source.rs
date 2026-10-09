//! Engine raw-index and skin-stream source callbacks over captured x86 memory.
//! CPU byte copies only; owner callbacks and device responses remain explicit inputs.
use crate::{d3d_upload as upload, shader_snapshot::Memory};
use serde::{Deserialize, Serialize};
pub const MAX_PAYLOAD: usize = 16 * 1024 * 1024;
fn word(memory: &Memory, address: u32, offset: u32) -> Result<u32, String> {
    let at = address
        .checked_add(offset)
        .filter(|_| address != 0)
        .ok_or("Source field address overflow/null")?;
    Ok(u32::from_le_bytes(memory.read(at, 4)?.try_into().unwrap()))
}
/// Native SHL 3 / SAR 3: preserve the sign of the packed 29-bit count.
pub fn count(word: u32) -> i32 {
    (word.wrapping_shl(3) as i32) >> 3
}
fn bytes(memory: &Memory, address: u32, size: u32) -> Result<Vec<u8>, String> {
    if size as u64 > MAX_PAYLOAD as u64 {
        return Err("Source payload exceeds bounded CPU limit".into());
    }
    if size == 0 {
        return Ok(vec![]);
    }
    memory.read(address, size as usize)
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum IndexWidth {
    U16,
    U32,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RawIndex {
    pub address: u32,
    pub data: u32,
    pub count_word: u32,
    pub width: IndexWidth,
}
impl RawIndex {
    pub fn capture(memory: &Memory, address: u32, width: IndexWidth) -> Result<Self, String> {
        Ok(Self {
            address,
            data: word(memory, address, 0x10)?,
            count_word: word(memory, address, 0x14)?,
            width,
        })
    }
    pub fn index_size(self) -> u32 {
        match self.width {
            IndexWidth::U16 => 2,
            IndexWidth::U32 => 4,
        }
    }
    pub fn size(self) -> u32 {
        (count(self.count_word) as u32).wrapping_mul(self.index_size())
    }
    pub fn payload(self, memory: &Memory) -> Result<Vec<u8>, String> {
        bytes(memory, self.data, self.size())
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Skin {
    pub address: u32,
    pub owner: u32,
    pub mode: u32,
    pub data: u32,
    pub count_word: u32,
}
impl Skin {
    pub fn capture(memory: &Memory, address: u32) -> Result<Self, String> {
        Ok(Self {
            address,
            owner: word(memory, address, 0x10)?,
            mode: word(memory, address, 0x18)?,
            data: word(memory, address, 0x1c)?,
            count_word: word(memory, address, 0x20)?,
        })
    }
    /// Size gate does not contain the fill callback's additional owner-null guard.
    pub fn size(self, owner_count: Option<u32>) -> Result<u32, String> {
        let n = if self.mode != 0 {
            if self.owner == 0 {
                return Err("Skin size requires nonnull owner".into());
            }
            owner_count.ok_or("Missing owner vertex-count result")?
        } else {
            count(self.count_word) as u32
        };
        Ok(n.wrapping_mul(32))
    }
    pub fn payload(self, memory: &Memory, delegate: Option<&[u8]>) -> Result<Vec<u8>, String> {
        if self.mode != 0 && self.owner != 0 {
            let data = delegate.ok_or("Missing owner fill callback bytes")?;
            if data.len() > MAX_PAYLOAD {
                return Err("Owner payload exceeds bounded CPU limit".into());
            }
            Ok(data.to_vec())
        } else {
            bytes(
                memory,
                self.data,
                (count(self.count_word) as u32).wrapping_mul(32),
            )
        }
    }
    /// Original pointer result; no bounds check or dereference, including negative offsets.
    pub fn raw_pointer(self, vertex: i32) -> u32 {
        if self.mode != 0 {
            0
        } else {
            self.data.wrapping_add((vertex as u32).wrapping_mul(32))
        }
    }
}
pub fn stride() -> u32 {
    32
}
/// GetComponents changes six bytes only. Caller-supplied trailing bytes stay intact.
pub fn components(mut initial: [u8; 16]) -> [u32; 5] {
    initial[..6].copy_from_slice(&[1, 0, 1, 1, 2, 4]);
    let mut result = [0; 5];
    for i in 0..4 {
        result[i] = u32::from_le_bytes(initial[i * 4..i * 4 + 4].try_into().unwrap());
    }
    result[4] = 3;
    result
}
/// Apply one planned source callback to a CPU lock mirror; validate before any writes.
pub fn fill_upload(
    plan: &upload::Plan,
    source: u32,
    callback: u32,
    locked_address: u32,
    payload: &[u8],
    destination: &mut [u8],
) -> Result<usize, String> {
    if !plan.uploaded {
        return Ok(0);
    }
    let tail = plan
        .commands
        .get(plan.commands.len().saturating_sub(3)..)
        .filter(|s| s.len() == 3)
        .ok_or("Missing upload callback sequence")?;
    if tail[0].vtable_offset != 0x2c
        || tail[0].arguments.len() != 4
        || tail[0].arguments[0] != 0
        || tail[0].arguments[1] != plan.size
        || tail[0].arguments[3] != plan.lock_flags
        || locked_address == 0
        || tail[1].receiver != source
        || tail[1].vtable_offset != callback
        || tail[1].arguments != [locked_address]
        || tail[2].receiver != tail[0].receiver
        || tail[2].vtable_offset != 0x30
        || !tail[2].arguments.is_empty()
        || payload.len() as u64 > u64::from(plan.size)
        || (destination.len() as u64) < u64::from(plan.size)
    {
        return Err("Inconsistent upload callback/range".into());
    }
    destination[..payload.len()].copy_from_slice(payload);
    Ok(payload.len())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VertexData {
    pub skin: Skin,
    pub owner_count: Option<u32>,
    pub delegate: Option<Vec<u8>>,
    pub dynamic: u32,
    pub special: u32,
    pub revision_after: u32,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct IndexData {
    pub index: RawIndex,
    pub revision_before: u32,
    pub revision_after: u32,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Transfer {
    pub plan: upload::Plan,
    pub copied: usize,
}
pub fn vertex(
    resource: &mut upload::Resource,
    data: &VertexData,
    memory: &Memory,
    device: upload::Device,
    responses: &upload::Responses,
    destination: &mut [u8],
) -> Result<Transfer, String> {
    let source = upload::VertexSource {
        address: data.skin.address,
        size: data.skin.size(data.owner_count)?,
        dynamic: data.dynamic,
        special: data.special,
        revision_after: data.revision_after,
    };
    let mut next = resource.clone();
    let plan = upload::vertex(&mut next, source, device, responses)?;
    let payload = data.skin.payload(memory, data.delegate.as_deref())?;
    let copied = fill_upload(
        &plan,
        source.address,
        0x24,
        responses.lock_pointer,
        &payload,
        destination,
    )?;
    *resource = next;
    Ok(Transfer { plan, copied })
}
pub fn index(
    resource: &mut upload::Resource,
    data: IndexData,
    memory: &Memory,
    device: upload::Device,
    responses: &upload::Responses,
    destination: &mut [u8],
) -> Result<Transfer, String> {
    let source = upload::IndexSource {
        address: data.index.address,
        size: data.index.size(),
        width: data.index.index_size(),
        revision_before: data.revision_before,
        revision_after: data.revision_after,
    };
    let mut next = resource.clone();
    let plan = upload::index(&mut next, source, device, responses)?;
    let payload = if plan.uploaded {
        data.index.payload(memory)?
    } else {
        vec![]
    };
    let copied = fill_upload(
        &plan,
        source.address,
        0x14,
        responses.lock_pointer,
        &payload,
        destination,
    )?;
    *resource = next;
    Ok(Transfer { plan, copied })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::shader_snapshot::Region;
    fn memory() -> Memory {
        Memory::new(vec![Region {
            address: 0x1000,
            bytes: (0..96).collect(),
        }])
        .unwrap()
    }
    #[test]
    fn packed_counts_preserve_sign_and_ignore_top_flags() {
        assert_eq!(count(0xe0000003), 3);
        assert_eq!(count(0x10000000), -268435456);
        assert_eq!(count(u32::MAX), -1);
    }
    #[test]
    fn both_index_widths_copy_raw_little_endian_bytes() {
        for (width, n) in [(IndexWidth::U16, 6), (IndexWidth::U32, 12)] {
            let s = RawIndex {
                address: 1,
                data: 0x1000,
                count_word: 0xa0000003,
                width,
            };
            assert_eq!(s.payload(&memory()).unwrap(), (0..n).collect::<Vec<_>>());
        }
    }
    #[test]
    fn zero_size_skips_null_pointer_read_and_negative_counts_fail_bounded() {
        let mut s = RawIndex {
            address: 1,
            data: 0,
            count_word: 0,
            width: IndexWidth::U16,
        };
        assert!(s.payload(&memory()).unwrap().is_empty());
        s.count_word = u32::MAX;
        assert!(s.payload(&memory()).is_err());
    }
    #[test]
    fn components_preserve_unwritten_bytes() {
        let c = components([0xff; 16]);
        assert_eq!(c, [0x01010001, 0xffff0402, u32::MAX, u32::MAX, 3]);
        assert_eq!(stride(), 32);
    }
    #[test]
    fn skin_size_and_fill_have_distinct_owner_gates() {
        let s = Skin {
            address: 1,
            owner: 0,
            mode: 1,
            data: 0x1000,
            count_word: 1,
        };
        assert!(s.size(Some(9)).is_err());
        assert_eq!(s.payload(&memory(), None).unwrap().len(), 32);
        assert_eq!(s.raw_pointer(0), 0);
    }
    #[test]
    fn delegate_overrides_stored_data_and_size_uses_owner_result() {
        let s = Skin {
            address: 1,
            owner: 2,
            mode: 255,
            data: 0,
            count_word: 99,
        };
        assert_eq!(s.size(Some(2)).unwrap(), 64);
        assert_eq!(s.payload(&memory(), Some(&[7, 8])).unwrap(), vec![7, 8]);
        assert!(s.payload(&memory(), None).is_err());
    }
    #[test]
    fn raw_pointer_arithmetic_wraps() {
        let s = Skin {
            address: 1,
            owner: 0,
            mode: 0,
            data: 8,
            count_word: 1,
        };
        assert_eq!(s.raw_pointer(-1), 0xffffffe8);
        assert_eq!(s.raw_pointer(i32::MIN), 8);
    }
    #[test]
    fn overflowing_skin_byte_size_can_wrap_to_zero() {
        let s = Skin {
            address: 1,
            owner: 0,
            mode: 0,
            data: 0,
            count_word: 0x08000000,
        };
        assert_eq!(s.size(None).unwrap(), 0);
        assert!(s.payload(&memory(), None).unwrap().is_empty());
    }
    #[test]
    fn sparse_capture_handles_contiguous_fragments_and_rejects_gaps() {
        let mut b = [0; 36];
        b[0x1c..0x20].copy_from_slice(&0x1000u32.to_le_bytes());
        let m = Memory::new(vec![
            Region {
                address: 0x2000,
                bytes: b[..30].to_vec(),
            },
            Region {
                address: 0x201e,
                bytes: b[30..].to_vec(),
            },
        ])
        .unwrap();
        assert_eq!(Skin::capture(&m, 0x2000).unwrap().data, 0x1000);
        assert!(RawIndex::capture(&m, 0xfffffff0, IndexWidth::U16).is_err());
    }
    #[test]
    fn invalid_fill_preserves_destination_and_no_upload_skips_reads() {
        let p = upload::Plan {
            reused: true,
            usage: None,
            lock_flags: 0,
            size: 4,
            format: None,
            uploaded: true,
            commands: vec![],
        };
        let mut out = [9; 8];
        assert!(fill_upload(&p, 1, 0x14, 2, &[1, 2], &mut out).is_err());
        assert_eq!(out, [9; 8]);
        let p = upload::Plan {
            uploaded: false,
            ..p
        };
        assert_eq!(fill_upload(&p, 0, 0, 0, &[1; 32], &mut out).unwrap(), 0);
    }
    fn fixture() -> (upload::Resource, upload::Device, upload::Responses) {
        (
            upload::Resource {
                address: 0x2000,
                handle: 0x2200,
                capacity: 32,
                revision: 7,
                source: 0,
            },
            upload::Device {
                address: 0x2300,
                hardware_vertices: 1,
                special_vertices: 0,
                skip_eviction: 0,
            },
            upload::Responses {
                creates: vec![upload::Created {
                    hresult: 0,
                    handle: 0x2400,
                }],
                evictions: vec![],
                lock_hresult: 0,
                lock_pointer: 0x5000,
                unlock_hresult: 0,
                lock_slot: 0x7000,
            },
        )
    }
    #[test]
    fn vertex_copy_preserves_tail_and_missing_payload_rolls_back() {
        let (mut r, d, o) = fixture();
        let mut data = VertexData {
            skin: Skin {
                address: 0x3000,
                owner: 0,
                mode: 0,
                data: 0x1000,
                count_word: 1,
            },
            owner_count: None,
            delegate: None,
            dynamic: 0,
            special: 0,
            revision_after: 8,
        };
        let mut out = [255; 40];
        let t = vertex(&mut r, &data, &memory(), d, &o, &mut out).unwrap();
        assert_eq!(t.copied, 32);
        assert_eq!(out[32..], [255; 8]);
        let before = r.clone();
        data.skin.data = 0x9000;
        assert!(vertex(&mut r, &data, &memory(), d, &o, &mut out).is_err());
        assert_eq!(r, before);
        assert_eq!(out[..32], (0..32).collect::<Vec<_>>());
    }
    #[test]
    fn index_revision_hit_skips_uncaptured_payload() {
        let (mut r, d, o) = fixture();
        let data = IndexData {
            index: RawIndex {
                address: 0x3000,
                data: 0x9000,
                count_word: 1,
                width: IndexWidth::U16,
            },
            revision_before: 7,
            revision_after: 9,
        };
        let mut out = [];
        let t = index(&mut r, data, &memory(), d, &o, &mut out).unwrap();
        assert_eq!(t.copied, 0);
        assert!(!t.plan.uploaded);
        assert_eq!(r.revision, 9);
    }
}
