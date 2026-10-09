//! Shared diagnostic scratch path, D3DDrv 1002d7fb..1002d8dd/1002da75..1002db46.
//! TArray reallocation pointers and source callback bytes remain explicit ABI answers.
use crate::{
    d3d_dynamic as dynamic,
    d3d_resize::{self, Context, UploadResponses},
    d3d_source::MAX_PAYLOAD,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Buffer {
    pub address: u32,
    /// Upper three array flags are preserved; low 29 bits encode a signed count.
    pub packed_count: u32,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Answers {
    /// Pointer returned by the first array-helper call with count zero.
    pub reset_pointer: u32,
    /// Pointer returned by the second array-helper call with the growth count.
    pub growth_pointer: u32,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ArrayCall {
    pub count: u32,
    pub extra: u32,
    pub pointer: u32,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Preparation {
    pub calls: Vec<ArrayCall>,
    pub guard_offset: u32,
    pub callback_pointer: u32,
}
impl Buffer {
    pub fn count(&self) -> i32 {
        (self.packed_count.wrapping_shl(3) as i32) >> 3
    }
    /// Native signed growth gate and doubled max(size,65535), followed by guard write.
    /// The guard ends at size+5 although the original growth gate compares size+4.
    pub fn prepare(&mut self, size: u32, answers: Answers) -> Result<Preparation, String> {
        if size as usize > MAX_PAYLOAD || self.bytes.len() > MAX_PAYLOAD {
            return Err("Scratch size exceeds bounded CPU storage".into());
        }
        let mut next = self.clone();
        let mut calls = vec![];
        if next.count() < size.wrapping_add(4) as i32 {
            let growth = size.max(0xffff).wrapping_mul(2);
            if growth as usize > MAX_PAYLOAD {
                return Err("Scratch growth exceeds bounded CPU storage".into());
            }
            calls.push(ArrayCall {
                count: 0,
                extra: 0,
                pointer: answers.reset_pointer,
            });
            calls.push(ArrayCall {
                count: growth,
                extra: 0,
                pointer: answers.growth_pointer,
            });
            next.address = answers.growth_pointer;
            next.packed_count = (next.packed_count & 0xe0000000) | growth;
            next.bytes = vec![0; growth as usize];
        }
        let end = size as usize + 5;
        if next.address == 0
            || next.address.checked_add(next.bytes.len() as u32).is_none()
            || next.count() < 0
            || next.count() as usize > next.bytes.len()
            || end > next.bytes.len()
        {
            return Err("Invalid scratch pointer/count or guard extent".into());
        }
        next.bytes[size as usize + 1..end].copy_from_slice(&0x03221977u32.to_le_bytes());
        let result = Preparation {
            calls,
            guard_offset: size + 1,
            callback_pointer: next.address,
        };
        *self = next;
        Ok(result)
    }
    /// Callback overwrites only its supplied prefix; the entire lock size is copied.
    pub fn fill_copy(
        &mut self,
        size: u32,
        payload: &[u8],
        target: &mut [u8],
    ) -> Result<(), String> {
        let size = size as usize;
        if size > MAX_PAYLOAD
            || payload.len() > size
            || self.bytes.len() < size
            || target.len() < size
        {
            return Err("Invalid scratch callback/copy extent".into());
        }
        self.bytes[..payload.len()].copy_from_slice(payload);
        target[..size].copy_from_slice(&self.bytes[..size]);
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Runtime {
    pub buffers: d3d_resize::Runtime,
    pub scratch: Buffer,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Transfer {
    pub resize: Option<d3d_resize::Plan>,
    pub scratch: Option<Preparation>,
    pub upload: dynamic::Transfer,
}
impl Runtime {
    pub fn vertex(
        &mut self,
        source: &dynamic::VertexSource,
        context: Context,
        answers: &UploadResponses,
        array: Answers,
        target: &mut [u8],
    ) -> Result<Transfer, String> {
        if self.buffers.device.scratch == 0 {
            let t = self.buffers.vertex(source, context, answers, target)?;
            return Ok(Transfer {
                resize: t.resize,
                scratch: None,
                upload: t.upload,
            });
        }
        let mut next = self.clone();
        let mut lock = answers.lock.clone();
        lock.resized = None;
        let size = if (source.size_before as i32) < 0 {
            source.size_before.wrapping_neg()
        } else {
            source.size_before
        };
        let resize = if (next.buffers.vertex.capacity as i32) < size as i32 {
            let mut ring = next.buffers.vertex.clone();
            let plan = d3d_resize::vertex(
                &mut ring,
                &mut next.buffers.deferred,
                size,
                context,
                &answers.resize,
            )?;
            lock.resized = Some(ring.handles);
            Some(plan)
        } else {
            None
        };
        let scratch = next.scratch.prepare(size, array)?;
        // Use the existing direct lock validation, then replace its callback destination.
        // No caller memory is changed until every upload and scratch check succeeds.
        let mut mirror = vec![0; size as usize];
        let enabled = next.buffers.device.scratch;
        next.buffers.device.scratch = 0;
        let mut upload = dynamic::vertex(
            &mut next.buffers.vertex,
            &mut next.buffers.device,
            source,
            &lock,
            &mut mirror,
        )?;
        next.buffers.device.scratch = enabled;
        next.scratch.fill_copy(size, &source.payload, target)?;
        upload.commands[1].arguments[0] = scratch.callback_pointer;
        *self = next;
        Ok(Transfer {
            resize,
            scratch: Some(scratch),
            upload,
        })
    }
    pub fn index(
        &mut self,
        source: &dynamic::IndexSource,
        context: Context,
        answers: &UploadResponses,
        array: Answers,
        target: &mut [u8],
    ) -> Result<Transfer, String> {
        if self.buffers.device.scratch == 0 {
            let t = self.buffers.index(source, context, answers, target)?;
            return Ok(Transfer {
                resize: t.resize,
                scratch: None,
                upload: t.upload,
            });
        }
        let mut next = self.clone();
        let mut lock = answers.lock.clone();
        lock.resized = None;
        let resize = if (next.buffers.index.capacity as i32) < source.size as i32 {
            let mut ring = next.buffers.index.clone();
            let plan = d3d_resize::index(&mut ring, source.size, context.device, &answers.resize)?;
            lock.resized = Some([ring.handle, 0]);
            Some(plan)
        } else {
            None
        };
        let scratch = next.scratch.prepare(source.size, array)?;
        let mut mirror = vec![0; source.size as usize];
        let enabled = next.buffers.device.scratch;
        next.buffers.device.scratch = 0;
        let mut upload = dynamic::index(
            &mut next.buffers.index,
            &next.buffers.device,
            source,
            &lock,
            &mut mirror,
        )?;
        next.buffers.device.scratch = enabled;
        next.scratch
            .fill_copy(source.size, &source.payload, target)?;
        upload.commands[1].arguments[0] = scratch.callback_pointer;
        *self = next;
        Ok(Transfer {
            resize,
            scratch: Some(scratch),
            upload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn answers() -> Answers {
        Answers {
            reset_pointer: 0,
            growth_pointer: 0x42000000,
        }
    }
    fn buffer(count: u32, extent: usize) -> Buffer {
        Buffer {
            address: 0x41000000,
            packed_count: count,
            bytes: vec![0xa5; extent],
        }
    }
    fn fixture() -> (Runtime, Context, UploadResponses) {
        let state = crate::d3d_state::Values {
            render: [0; 32],
            stages: [[0; 21]; 8],
            textures: [0; 8],
        };
        let bind = crate::d3d_bindings::Values {
            vertex_shader: 0,
            pixel_shader: 0,
            streams: [[0; 2]; 16],
            indices: [0; 2],
        };
        (
            Runtime {
                buffers: d3d_resize::Runtime {
                    vertex: dynamic::Vertex {
                        address: 0x34000000,
                        handles: [11, 22],
                        capacity: 16,
                        cursor: 0,
                        active: 1,
                        source: 99,
                    },
                    index: dynamic::Index {
                        address: 0x34000100,
                        handle: 33,
                        capacity: 16,
                        cursor: 0,
                        width: 4,
                    },
                    device: dynamic::Device {
                        vertex_limit: 65535,
                        scratch: 7,
                        discards: 0,
                        vertex_bytes: 0,
                        index_bytes: 0,
                    },
                    deferred: crate::d3d_bindings::Deferred {
                        states: crate::d3d_state::Cache {
                            desired: state.clone(),
                            applied: state,
                            dirty: 0,
                        },
                        transforms: crate::d3d_transforms::Transforms {
                            words: [[0; 16]; 11],
                            mask: 0,
                        },
                        bindings: crate::d3d_bindings::Bindings {
                            desired: bind.clone(),
                            applied: bind,
                        },
                    },
                },
                scratch: buffer(32, 32),
            },
            Context {
                device: crate::d3d_upload::Device {
                    address: 0x23000000,
                    hardware_vertices: 1,
                    special_vertices: 0,
                    skip_eviction: 0,
                },
                stream_capacity: 16,
            },
            UploadResponses {
                resize: d3d_resize::Responses {
                    creates: vec![
                        crate::d3d_upload::Created {
                            hresult: 0,
                            handle: 101,
                        },
                        crate::d3d_upload::Created {
                            hresult: 0,
                            handle: 102,
                        },
                    ],
                    evictions: vec![],
                },
                lock: dynamic::Responses {
                    resized: None,
                    lock_hresult: 0,
                    lock_pointer: 0x40000000,
                    lock_slot: 0x27000000,
                    unlock_hresult: 0,
                },
            },
        )
    }
    fn vertex() -> dynamic::VertexSource {
        dynamic::VertexSource {
            address: 1,
            size_before: 7,
            size_again: 7,
            stride: 3,
            payload: vec![1, 2, 3],
        }
    }
    fn index() -> dynamic::IndexSource {
        dynamic::IndexSource {
            address: 2,
            size: 7,
            width: 2,
            payload: vec![4, 5],
        }
    }
    #[test]
    fn small_growth_zeroes_full_minimum_and_preserves_flags() {
        let mut b = buffer(0xe0000002, 2);
        let p = b.prepare(3, answers()).unwrap();
        assert_eq!(b.packed_count, 0xe001fffe);
        assert_eq!(b.bytes.len(), 131070);
        assert_eq!(
            p.calls.iter().map(|c| c.count).collect::<Vec<_>>(),
            vec![0, 131070]
        );
        assert_eq!(&b.bytes[4..8], &[0x77, 0x19, 0x22, 3]);
        assert!(b.bytes[8..].iter().all(|b| *b == 0));
    }
    #[test]
    fn large_growth_doubles_requested_size() {
        let mut b = buffer(0, 0);
        b.prepare(70001, answers()).unwrap();
        assert_eq!(b.count(), 140002);
        assert_eq!(b.bytes.len(), 140002);
    }
    #[test]
    fn negative_packed_count_forces_growth() {
        let mut b = buffer(0x1fffffff, 0);
        b.prepare(0, answers()).unwrap();
        assert_eq!(b.count(), 131070);
    }
    #[test]
    fn reuse_preserves_flags_pointer_and_unwritten_bytes() {
        let mut b = buffer(0xa0000020, 32);
        let p = b.prepare(7, answers()).unwrap();
        assert!(p.calls.is_empty());
        assert_eq!(b.address, 0x41000000);
        assert_eq!(b.packed_count, 0xa0000020);
        assert_eq!(b.bytes[7], 0xa5);
        assert_eq!(b.bytes[12], 0xa5);
    }
    #[test]
    fn native_gate_can_reuse_physical_padding() {
        let mut b = buffer(11, 12);
        assert!(b.prepare(7, answers()).unwrap().calls.is_empty());
        assert_eq!(b.bytes[11], 3);
    }
    #[test]
    fn native_guard_off_by_one_is_safely_rejected_without_padding() {
        let mut b = buffer(11, 11);
        let old = b.clone();
        assert!(b.prepare(7, answers()).is_err());
        assert_eq!(b, old);
    }
    #[test]
    fn zero_size_guard_keeps_first_byte() {
        let mut b = buffer(5, 5);
        b.prepare(0, answers()).unwrap();
        assert_eq!(b.bytes, vec![0xa5, 0x77, 0x19, 0x22, 3]);
    }
    #[test]
    fn full_copy_uses_stale_scratch_tail_and_keeps_target_tail() {
        let mut b = buffer(32, 32);
        b.prepare(7, answers()).unwrap();
        let mut t = vec![9; 10];
        b.fill_copy(7, &[1, 2], &mut t).unwrap();
        assert_eq!(t, vec![1, 2, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 9, 9, 9]);
    }
    #[test]
    fn failed_copy_preserves_both_buffers() {
        let mut b = buffer(32, 32);
        let old = b.clone();
        let mut t = vec![9; 3];
        assert!(b.fill_copy(7, &[1, 2], &mut t).is_err());
        assert_eq!(b, old);
        assert_eq!(t, vec![9; 3]);
    }
    #[test]
    fn oversized_growth_preserves_buffer() {
        let mut b = buffer(0, 0);
        let old = b.clone();
        assert!(b.prepare(MAX_PAYLOAD as u32, answers()).is_err());
        assert_eq!(b, old);
    }
    #[test]
    fn invalid_growth_pointer_preserves_buffer() {
        let mut b = buffer(0, 0);
        let old = b.clone();
        assert!(b
            .prepare(
                1,
                Answers {
                    growth_pointer: 0,
                    ..answers()
                }
            )
            .is_err());
        assert_eq!(b, old);
    }
    #[test]
    fn vertex_callback_uses_scratch_and_copy_uses_full_lock_size() {
        let (mut r, c, a) = fixture();
        let mut t = vec![9; 10];
        let p = r.vertex(&vertex(), c, &a, answers(), &mut t).unwrap();
        assert_eq!(p.upload.commands[1].arguments, vec![0x41000000]);
        assert_eq!(p.upload.commands[0].receiver, 22);
        assert_eq!(&t[..7], &[1, 2, 3, 0xa5, 0xa5, 0xa5, 0xa5]);
        assert_eq!(r.buffers.device.scratch, 7);
    }
    #[test]
    fn index_callback_preserves_stored_width_and_exact_cursor() {
        let (mut r, c, a) = fixture();
        let mut t = vec![9; 10];
        let p = r.index(&index(), c, &a, answers(), &mut t).unwrap();
        assert_eq!(p.upload.commands[1].vtable_offset, 0x14);
        assert_eq!(r.buffers.index.width, 4);
        assert_eq!(r.buffers.index.cursor, 7);
        assert_eq!(&t[..7], &[4, 5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5]);
    }
    #[test]
    fn vertex_growth_composes_resize_with_scratch() {
        let (mut r, c, a) = fixture();
        r.buffers.vertex.capacity = 1;
        r.scratch = buffer(0, 0);
        let mut t = vec![9; 10];
        let p = r.vertex(&vertex(), c, &a, answers(), &mut t).unwrap();
        assert!(p.resize.is_some());
        assert_eq!(p.upload.commands[0].receiver, 102);
        assert_eq!(&t[..7], &[1, 2, 3, 0, 0, 0, 0]);
    }
    #[test]
    fn index_growth_composes_resize_with_scratch() {
        let (mut r, c, a) = fixture();
        r.buffers.index.capacity = 1;
        let mut t = vec![9; 10];
        let p = r.index(&index(), c, &a, answers(), &mut t).unwrap();
        assert_eq!(p.resize.unwrap().format, Some(0x66));
        assert_eq!(p.upload.commands[0].receiver, 101);
    }
    #[test]
    fn unlock_failure_rolls_back_growth_and_target() {
        let (mut r, c, mut a) = fixture();
        r.scratch = buffer(0, 0);
        a.lock.unlock_hresult = 0x80004005;
        let old = r.clone();
        let mut t = vec![9; 10];
        assert!(r.vertex(&vertex(), c, &a, answers(), &mut t).is_err());
        assert_eq!(r, old);
        assert_eq!(t, vec![9; 10]);
    }
    #[test]
    fn short_target_rolls_back_scratch_and_ring() {
        let (mut r, c, a) = fixture();
        let old = r.clone();
        let mut t = vec![9; 3];
        assert!(r.index(&index(), c, &a, answers(), &mut t).is_err());
        assert_eq!(r, old);
        assert_eq!(t, vec![9; 3]);
    }
    #[test]
    fn disabled_scratch_ignores_invalid_array_and_retains_direct_tail() {
        let (mut r, c, a) = fixture();
        r.buffers.device.scratch = 0;
        r.scratch.address = 0;
        let old = r.scratch.clone();
        let mut t = vec![9; 10];
        let p = r
            .vertex(
                &vertex(),
                c,
                &a,
                Answers {
                    reset_pointer: 0,
                    growth_pointer: 0,
                },
                &mut t,
            )
            .unwrap();
        assert!(p.scratch.is_none());
        assert_eq!(p.upload.commands[1].arguments, vec![0x40000000]);
        assert_eq!(t, vec![1, 2, 3, 9, 9, 9, 9, 9, 9, 9]);
        assert_eq!(r.scratch, old);
    }
}
