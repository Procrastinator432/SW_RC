//! Original dynamic resize lifecycles and automatic resize-to-ring integration.
//! D3DDrv 10029fe0/1002a5f0/1002a880; COM responses are inputs, calls are planned.
use crate::{
    d3d_bindings::Deferred,
    d3d_dynamic as dynamic,
    d3d_upload::{Command, Created, Device},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Context {
    pub device: Device,
    pub stream_capacity: i32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Responses {
    pub creates: Vec<Created>,
    pub evictions: Vec<u32>,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Plan {
    pub usage: u32,
    pub format: Option<u32>,
    pub commands: Vec<Command>,
}
fn command(receiver: u32, offset: u32, arguments: Vec<u32>) -> Command {
    Command {
        receiver,
        vtable_offset: offset,
        arguments,
    }
}
/// Clear matching applied and desired streams independently; never change dirty bits.
/// The native helper also accepts zero and scans signed capacity clamped to [0,16].
pub fn invalidate(
    deferred: &mut Deferred,
    handle: u32,
    capacity: i32,
    device: u32,
) -> Vec<Command> {
    let mut commands = vec![];
    for slot in 0..capacity.clamp(0, 16) as usize {
        if deferred.bindings.applied.streams[slot][0] == handle {
            deferred.bindings.applied.streams[slot] = [0, 0];
            commands.push(command(device, 0x14c, vec![slot as u32, 0, 0]));
        }
        if deferred.bindings.desired.streams[slot][0] == handle {
            deferred.bindings.desired.streams[slot] = [0, 0];
        }
    }
    commands
}
fn validate(address: u32, device: Device) -> Result<(), String> {
    if address == 0 || address.checked_add(0x48).is_none() || device.address == 0 {
        Err("Invalid bounded dynamic resize wrapper/device".into())
    } else {
        Ok(())
    }
}
fn create(
    plan: &mut Plan,
    device: Device,
    slot: u32,
    size: u32,
    pool: u32,
    o: &Responses,
    index: &mut usize,
) -> Result<Created, String> {
    let result = *o
        .creates
        .get(*index)
        .ok_or("Missing dynamic resize creation response")?;
    *index += 1;
    let args = vec![size, plan.usage, plan.format.unwrap_or(0), pool, slot];
    plan.commands.push(command(
        device.address,
        if plan.format.is_some() { 0x60 } else { 0x5c },
        args,
    ));
    if (result.hresult as i32) >= 0 && result.handle == 0 {
        return Err("Successful dynamic creation returned null handle".into());
    }
    Ok(result)
}
fn retry(
    plan: &mut Plan,
    device: Device,
    o: &Responses,
    eviction: &mut usize,
    attempt: &mut usize,
) -> Result<(), String> {
    if device.skip_eviction != 0 {
        *attempt += 2;
    } else {
        plan.commands.push(command(device.address, 0x14, vec![0]));
        let hr = *o
            .evictions
            .get(*eviction)
            .ok_or("Missing resize eviction result")?;
        *eviction += 1;
        if (hr as i32) < 0 {
            return Err("Dynamic resize eviction failed".into());
        }
        *attempt += 1;
    }
    Ok(())
}
/// Two-buffer creation. Failed first creation aborts; failed second retries the pair.
pub fn vertex(
    ring: &mut dynamic::Vertex,
    deferred: &mut Deferred,
    size: u32,
    context: Context,
    o: &Responses,
) -> Result<Plan, String> {
    validate(ring.address, context.device)?;
    let mut next = ring.clone();
    let mut cache = deferred.clone();
    let mut plan = Plan {
        usage: if context.device.hardware_vertices == 0 {
            0x218
        } else {
            0x208
        },
        format: None,
        commands: vec![],
    };
    for handle in next.handles {
        if handle != 0 {
            plan.commands.extend(invalidate(
                &mut cache,
                handle,
                context.stream_capacity,
                context.device.address,
            ));
            plan.commands.push(command(handle, 8, vec![]));
        }
    }
    next.handles = [0, 0];
    next.capacity = size;
    next.cursor = 0;
    let (mut attempt, mut response, mut eviction) = (0, 0, 0);
    while attempt < 3 {
        let pool = if attempt == 2 { 2 } else { 0 };
        let first = create(
            &mut plan,
            context.device,
            next.address + 0x30,
            size,
            pool,
            o,
            &mut response,
        )?;
        next.handles[0] = first.handle;
        if (first.hresult as i32) < 0 {
            return Err("First dynamic vertex creation failed".into());
        }
        let second = create(
            &mut plan,
            context.device,
            next.address + 0x34,
            size,
            pool,
            o,
            &mut response,
        )?;
        next.handles[1] = second.handle;
        if (second.hresult as i32) >= 0 {
            *ring = next;
            *deferred = cache;
            return Ok(plan);
        }
        // Native retry overwrites the first handle; it emits no intermediate Release.
        retry(&mut plan, context.device, o, &mut eviction, &mut attempt)?;
    }
    Err("Dynamic vertex creation attempts exhausted".into())
}
pub fn index(
    ring: &mut dynamic::Index,
    size: u32,
    device: Device,
    o: &Responses,
) -> Result<Plan, String> {
    validate(ring.address, device)?;
    let mut next = ring.clone();
    let mut plan = Plan {
        usage: if device.hardware_vertices == 0 {
            0x218
        } else {
            0x208
        },
        format: Some(if next.width == 4 { 0x66 } else { 0x65 }),
        commands: vec![],
    };
    if next.handle != 0 {
        plan.commands.push(command(next.handle, 8, vec![]));
    }
    next.handle = 0;
    next.capacity = size;
    next.cursor = 0;
    let (mut attempt, mut response, mut eviction) = (0, 0, 0);
    while attempt < 3 {
        let pool = if attempt == 2 { 2 } else { 0 };
        let result = create(
            &mut plan,
            device,
            next.address + 0x30,
            size,
            pool,
            o,
            &mut response,
        )?;
        next.handle = result.handle;
        if (result.hresult as i32) >= 0 {
            *ring = next;
            return Ok(plan);
        }
        retry(&mut plan, device, o, &mut eviction, &mut attempt)?;
    }
    Err("Dynamic index creation attempts exhausted".into())
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Runtime {
    pub vertex: dynamic::Vertex,
    pub index: dynamic::Index,
    pub device: dynamic::Device,
    pub deferred: Deferred,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UploadResponses {
    pub resize: Responses,
    pub lock: dynamic::Responses,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Transfer {
    pub resize: Option<Plan>,
    pub upload: dynamic::Transfer,
}
impl Runtime {
    /// Plan resize only on the original signed growth gate, then execute the CPU mirror.
    pub fn vertex(
        &mut self,
        source: &dynamic::VertexSource,
        context: Context,
        o: &UploadResponses,
        target: &mut [u8],
    ) -> Result<Transfer, String> {
        let mut next = self.clone();
        let mut lock = o.lock.clone();
        lock.resized = None;
        let size = if (source.size_before as i32) < 0 {
            source.size_before.wrapping_neg()
        } else {
            source.size_before
        };
        let resize = if (next.vertex.capacity as i32) < (size as i32) {
            let mut result = next.vertex.clone();
            let plan = vertex(&mut result, &mut next.deferred, size, context, &o.resize)?;
            lock.resized = Some(result.handles);
            Some(plan)
        } else {
            None
        };
        let upload = dynamic::vertex(&mut next.vertex, &mut next.device, source, &lock, target)?;
        *self = next;
        Ok(Transfer { resize, upload })
    }
    pub fn index(
        &mut self,
        source: &dynamic::IndexSource,
        context: Context,
        o: &UploadResponses,
        target: &mut [u8],
    ) -> Result<Transfer, String> {
        let mut next = self.clone();
        let mut lock = o.lock.clone();
        lock.resized = None;
        let resize = if (next.index.capacity as i32) < (source.size as i32) {
            let mut result = next.index.clone();
            let plan = index(&mut result, source.size, context.device, &o.resize)?;
            lock.resized = Some([result.handle, 0]);
            Some(plan)
        } else {
            None
        };
        let upload = dynamic::index(&mut next.index, &next.device, source, &lock, target)?;
        *self = next;
        Ok(Transfer { resize, upload })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Runtime, Context, UploadResponses) {
        let values = crate::d3d_state::Values {
            render: [0; 32],
            stages: [[0; 21]; 8],
            textures: [0; 8],
        };
        let mut desired = crate::d3d_bindings::Values {
            vertex_shader: 7,
            pixel_shader: 9,
            streams: [[0; 2]; 16],
            indices: [77, 88],
        };
        desired.streams[0] = [11, 32];
        desired.streams[1] = [22, 64];
        desired.streams[2] = [33, 96];
        let mut applied = desired.clone();
        applied.streams[0] = [22, 65];
        applied.streams[1] = [11, 33];
        let cache = Deferred {
            states: crate::d3d_state::Cache {
                desired: values.clone(),
                applied: values,
                dirty: 0x89,
            },
            transforms: crate::d3d_transforms::Transforms {
                words: [[0; 16]; 11],
                mask: 3,
            },
            bindings: crate::d3d_bindings::Bindings { desired, applied },
        };
        (
            Runtime {
                vertex: dynamic::Vertex {
                    address: 0x34000000,
                    handles: [11, 22],
                    capacity: 16,
                    cursor: 7,
                    active: 1,
                    source: 99,
                },
                index: dynamic::Index {
                    address: 0x34000100,
                    handle: 33,
                    capacity: 4,
                    cursor: 3,
                    width: 4,
                },
                device: dynamic::Device {
                    vertex_limit: 65535,
                    scratch: 0,
                    discards: u32::MAX,
                    vertex_bytes: 5,
                    index_bytes: 6,
                },
                deferred: cache,
            },
            Context {
                device: Device {
                    address: 0x23000000,
                    hardware_vertices: 1,
                    special_vertices: 1,
                    skip_eviction: 0,
                },
                stream_capacity: 16,
            },
            UploadResponses {
                resize: Responses {
                    creates: vec![
                        Created {
                            hresult: 0,
                            handle: 101,
                        },
                        Created {
                            hresult: 0,
                            handle: 102,
                        },
                    ],
                    evictions: vec![0; 3],
                },
                lock: dynamic::Responses {
                    resized: None,
                    lock_hresult: 0,
                    lock_pointer: 0x40000000,
                    lock_slot: 0x27000000 - 0x14,
                    unlock_hresult: 0,
                },
            },
        )
    }
    fn failed() -> Created {
        Created {
            hresult: 0x80004005,
            handle: 0,
        }
    }
    fn created(h: u32) -> Created {
        Created {
            hresult: 0,
            handle: h,
        }
    }
    fn source() -> dynamic::VertexSource {
        dynamic::VertexSource {
            address: 1,
            size_before: 32,
            size_again: 32,
            stride: 32,
            payload: vec![7; 32],
        }
    }
    #[test]
    fn invalidation_compares_applied_and_desired_independently() {
        let (mut r, c, _) = fixture();
        let commands = invalidate(&mut r.deferred, 11, 16, c.device.address);
        assert_eq!(
            commands,
            vec![command(c.device.address, 0x14c, vec![1, 0, 0])]
        );
        assert_eq!(r.deferred.bindings.desired.streams[0], [0, 0]);
        assert_eq!(r.deferred.bindings.applied.streams[0], [22, 65]);
        assert_eq!(r.deferred.bindings.desired.streams[1], [22, 64]);
        assert_eq!(r.deferred.bindings.applied.streams[1], [0, 0]);
    }
    #[test]
    fn invalidation_preserves_dirty_indices_shaders_and_transform_mask() {
        let (mut r, c, _) = fixture();
        let old = r.deferred.clone();
        invalidate(&mut r.deferred, 11, 16, c.device.address);
        assert_eq!(r.deferred.states, old.states);
        assert_eq!(r.deferred.transforms, old.transforms);
        assert_eq!(r.deferred.bindings.desired.indices, [77, 88]);
        assert_eq!(r.deferred.bindings.desired.vertex_shader, 7);
    }
    #[test]
    fn invalidation_handles_zero_and_signed_capacity_clamp() {
        let (mut r, c, _) = fixture();
        let old = r.deferred.clone();
        assert!(invalidate(&mut r.deferred, 11, -1, c.device.address).is_empty());
        assert_eq!(r.deferred, old);
        assert_eq!(
            invalidate(&mut r.deferred, 0, 99, c.device.address).len(),
            13
        );
        let (mut r, _, _) = fixture();
        assert!(invalidate(&mut r.deferred, 11, 1, c.device.address).is_empty());
        assert_eq!(r.deferred.bindings.applied.streams[1], [11, 33]);
    }
    #[test]
    fn vertex_resize_unbinds_before_each_old_handle_release_and_preserves_active_source() {
        let (mut r, c, o) = fixture();
        let p = vertex(&mut r.vertex, &mut r.deferred, 64, c, &o.resize).unwrap();
        assert_eq!(
            p.commands[..4],
            [
                command(c.device.address, 0x14c, vec![1, 0, 0]),
                command(11, 8, vec![]),
                command(c.device.address, 0x14c, vec![0, 0, 0]),
                command(22, 8, vec![])
            ]
        );
        assert_eq!(
            (
                r.vertex.handles,
                r.vertex.capacity,
                r.vertex.cursor,
                r.vertex.active,
                r.vertex.source
            ),
            ([101, 102], 64, 0, 1, 99)
        );
        assert_eq!(p.usage, 0x208);
    }
    #[test]
    fn failed_second_vertex_creation_retries_pair_without_releasing_first_new_handle() {
        let (mut r, c, mut o) = fixture();
        o.resize.creates = vec![created(101), failed(), created(201), created(202)];
        let p = vertex(&mut r.vertex, &mut r.deferred, 64, c, &o.resize).unwrap();
        assert_eq!(r.vertex.handles, [201, 202]);
        assert!(!p
            .commands
            .iter()
            .any(|c| c.receiver == 101 && c.vtable_offset == 8));
        assert_eq!(
            p.commands
                .iter()
                .filter(|c| c.vtable_offset == 0x5c)
                .count(),
            4
        );
        assert_eq!(
            p.commands
                .iter()
                .filter(|c| c.vtable_offset == 0x14)
                .count(),
            1
        );
    }
    #[test]
    fn vertex_pair_skip_eviction_moves_directly_to_system_pool() {
        let (mut r, mut c, mut o) = fixture();
        c.device.skip_eviction = 1;
        c.device.hardware_vertices = 0;
        o.resize.creates = vec![created(101), failed(), created(201), created(202)];
        o.resize.evictions.clear();
        let p = vertex(&mut r.vertex, &mut r.deferred, 64, c, &o.resize).unwrap();
        let pools: Vec<_> = p
            .commands
            .iter()
            .filter(|c| c.vtable_offset == 0x5c)
            .map(|c| c.arguments[3])
            .collect();
        assert_eq!(pools, [0, 0, 2, 2]);
        assert_eq!(p.usage, 0x218);
    }
    #[test]
    fn failed_first_vertex_creation_does_not_try_later_successful_answers() {
        let (mut r, c, mut o) = fixture();
        o.resize.creates = vec![failed(), created(101), created(102)];
        let old = r.clone();
        assert!(vertex(&mut r.vertex, &mut r.deferred, 64, c, &o.resize).is_err());
        assert_eq!(r, old);
    }
    #[test]
    fn duplicate_old_handles_are_released_twice_but_unbound_once() {
        let (mut r, c, o) = fixture();
        r.vertex.handles = [11, 11];
        let p = vertex(&mut r.vertex, &mut r.deferred, 64, c, &o.resize).unwrap();
        assert_eq!(
            p.commands
                .iter()
                .filter(|c| c.receiver == 11 && c.vtable_offset == 8)
                .count(),
            2
        );
        assert_eq!(
            p.commands
                .iter()
                .filter(|c| c.vtable_offset == 0x14c)
                .count(),
            1
        );
    }
    #[test]
    fn index_retry_uses_stored_format_and_pool_sequence() {
        let (mut r, c, mut o) = fixture();
        o.resize.creates = vec![failed(), failed(), created(201)];
        let p = index(&mut r.index, 32, c.device, &o.resize).unwrap();
        assert_eq!(p.format, Some(0x66));
        assert_eq!(
            p.commands
                .iter()
                .filter(|c| c.vtable_offset == 0x60)
                .map(|c| c.arguments[3])
                .collect::<Vec<_>>(),
            [0, 0, 2]
        );
        assert_eq!(
            (
                r.index.handle,
                r.index.capacity,
                r.index.cursor,
                r.index.width
            ),
            (201, 32, 0, 4)
        );
    }
    #[test]
    fn index_skip_eviction_fallback_preserves_cache_and_uses_nonfour_format() {
        let (mut r, mut c, mut o) = fixture();
        c.device.skip_eviction = 1;
        r.index.width = 3;
        o.resize.creates = vec![failed(), created(201)];
        o.resize.evictions.clear();
        let before = r.deferred.clone();
        let p = index(&mut r.index, 32, c.device, &o.resize).unwrap();
        assert_eq!(p.format, Some(0x65));
        assert_eq!(r.deferred, before);
        assert_eq!(p.commands.last().unwrap().arguments[3], 2);
    }
    #[test]
    fn exhausted_or_failed_eviction_and_null_success_are_atomic() {
        for mode in 0..3 {
            let (mut r, c, mut o) = fixture();
            o.resize.creates = vec![failed(); 3];
            if mode == 1 {
                o.resize.evictions[0] = 0x80004005;
            }
            if mode == 2 {
                o.resize.creates[0] = created(0);
            }
            let before = r.clone();
            assert!(index(&mut r.index, 32, c.device, &o.resize).is_err());
            assert_eq!(r, before);
        }
    }
    #[test]
    fn integrated_vertex_resize_generates_handles_and_keeps_double_discard_count() {
        let (mut r, c, o) = fixture();
        let mut target = vec![0xcc; 48];
        let p = r.vertex(&source(), c, &o, &mut target).unwrap();
        assert!(p.resize.is_some());
        assert!(p.upload.allocation.resized);
        assert_eq!(p.upload.commands[0].receiver, 102);
        assert_eq!(r.device.discards, 1);
        assert_eq!(&target[32..], &[0xcc; 16]);
    }
    #[test]
    fn integrated_reuse_does_not_require_resize_device_or_responses() {
        let (mut r, mut c, mut o) = fixture();
        r.vertex.capacity = 256;
        c.device.address = 0;
        o.resize.creates.clear();
        o.resize.evictions.clear();
        let mut target = vec![0; 48];
        let p = r.vertex(&source(), c, &o, &mut target).unwrap();
        assert!(p.resize.is_none());
        assert_eq!(p.upload.commands[0].receiver, 22);
    }
    #[test]
    fn late_lock_failure_rolls_back_resize_invalidation_and_target() {
        let (mut r, c, mut o) = fixture();
        o.lock.unlock_hresult = 0x80004005;
        let mut target = vec![0xcc; 48];
        let before = (r.clone(), target.clone());
        assert!(r.vertex(&source(), c, &o, &mut target).is_err());
        assert_eq!((r, target), before);
    }
    #[test]
    fn integrated_index_uses_created_handle_but_source_width_for_return_division() {
        let (mut r, c, o) = fixture();
        let src = dynamic::IndexSource {
            address: 1,
            size: 16,
            width: 2,
            payload: vec![9; 16],
        };
        let mut target = vec![0xcc; 24];
        let p = r.index(&src, c, &o, &mut target).unwrap();
        assert_eq!(p.resize.unwrap().format, Some(0x66));
        assert_eq!(p.upload.commands[0].receiver, 101);
        assert_eq!(r.index.width, 4);
        assert_eq!(&target[16..], &[0xcc; 8]);
    }
}
