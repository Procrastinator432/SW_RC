//! Dynamic ring scheduling, direct lock mirrors and resolved renderer handoff.
//! D3DDrv 1002d610/1002d990/10020540/10020a90; resize results remain explicit.
use crate::{
    d3d_bindings::Deferred,
    d3d_buffers::{Shader, State},
    d3d_source::MAX_PAYLOAD,
    d3d_upload::Command,
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Vertex {
    pub address: u32,
    pub handles: [u32; 2],
    pub capacity: u32,
    pub cursor: u32,
    pub active: u32,
    pub source: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Index {
    pub address: u32,
    pub handle: u32,
    pub capacity: u32,
    pub cursor: u32,
    /// Stored creation width at +3c, distinct from the source getter below.
    pub width: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Device {
    pub vertex_limit: u32,
    pub scratch: u32,
    pub discards: u32,
    pub vertex_bytes: u32,
    pub index_bytes: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VertexSource {
    pub address: u32,
    pub size_before: u32,
    /// The native getter is called again; a negative result forces DISCARD.
    pub size_again: u32,
    pub stride: u32,
    pub payload: Vec<u8>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndexSource {
    pub address: u32,
    pub size: u32,
    pub width: u32,
    pub payload: Vec<u8>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Responses {
    /// Handles produced by the external resize helper, only required on growth.
    pub resized: Option<[u32; 2]>,
    pub lock_hresult: u32,
    pub lock_pointer: u32,
    pub lock_slot: u32,
    pub unlock_hresult: u32,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Allocation {
    pub size: u32,
    pub byte_offset: u32,
    pub element_offset: u32,
    pub flags: u32,
    pub slot: u32,
    pub resized: bool,
    pub discard_events: u32,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Transfer {
    pub allocation: Allocation,
    pub copied: usize,
    pub commands: Vec<Command>,
}
fn signed_div(n: u32, d: u32) -> Result<u32, String> {
    (n as i32)
        .checked_div(d as i32)
        .map(|q| q as u32)
        .ok_or("Native IDIV zero divisor or quotient overflow".into())
}
/// Arithmetic only: signed capacity tests, unsigned limit division, signed alignment.
pub fn vertex_schedule(
    ring: &Vertex,
    source: &VertexSource,
    limit: u32,
    resized: Option<[u32; 2]>,
) -> Result<(Vertex, Allocation), String> {
    if ring.address == 0 || source.address == 0 || ring.active > 1 || source.stride == 0 {
        return Err("Invalid dynamic vertex wrapper/source/slot/stride".into());
    }
    let mut next = ring.clone();
    let size = if (source.size_before as i32) < 0 {
        source.size_before.wrapping_neg()
    } else {
        source.size_before
    };
    let grow = (next.capacity as i32) < (size as i32);
    let mut flags = 0x1000;
    let mut discards = 0;
    if grow {
        next.handles = resized
            .filter(|h| h.iter().all(|v| *v != 0))
            .ok_or("Missing successful vertex resize handles")?;
        next.capacity = size;
        next.cursor = 0;
        flags = 0x2000;
        discards += 1;
    }
    let end = next.cursor.wrapping_add(source.stride).wrapping_add(size);
    if (source.size_again as i32) < 0
        || (end as i32) > (next.capacity as i32)
        || end / source.stride > limit
    {
        next.cursor = 0;
        flags = 0x2000;
        discards += 1;
    }
    let aligned = signed_div(
        next.cursor.wrapping_add(source.stride).wrapping_sub(1),
        source.stride,
    )?
    .wrapping_mul(source.stride);
    let element_offset = signed_div(aligned, source.stride)?;
    next.cursor = aligned.wrapping_add(size);
    Ok((
        next,
        Allocation {
            size,
            byte_offset: aligned,
            element_offset,
            flags,
            slot: ring.active,
            resized: grow,
            discard_events: discards,
        },
    ))
}
pub fn index_schedule(
    ring: &Index,
    source: &IndexSource,
    resized: Option<[u32; 2]>,
) -> Result<(Index, Allocation), String> {
    if ring.address == 0 || source.address == 0 {
        return Err("Invalid dynamic index wrapper/source".into());
    }
    let mut next = ring.clone();
    let mut flags = 0x1000;
    let grow = (next.capacity as i32) < (source.size as i32);
    if grow {
        next.handle = resized
            .map(|h| h[0])
            .filter(|h| *h != 0)
            .ok_or("Missing successful index resize handle")?;
        next.capacity = source.size;
        next.cursor = 0;
        flags = 0x2000;
    }
    if (next.cursor.wrapping_add(source.size) as i32) > (next.capacity as i32) {
        next.cursor = 0;
        flags = 0x2000;
    }
    let offset = next.cursor;
    let element_offset = signed_div(offset, source.width)?;
    next.cursor = offset.wrapping_add(source.size);
    Ok((
        next,
        Allocation {
            size: source.size,
            byte_offset: offset,
            element_offset,
            flags,
            slot: 0,
            resized: grow,
            discard_events: 0,
        },
    ))
}
fn direct(
    a: Allocation,
    handle: u32,
    callback: (u32, u32),
    payload: &[u8],
    device: &Device,
    o: &Responses,
    target: &mut [u8],
) -> Result<Transfer, String> {
    if device.scratch != 0 {
        return Err("Dynamic scratch-buffer path is not yet reconstructed".into());
    }
    if handle == 0
        || o.lock_pointer == 0
        || o.lock_slot == 0
        || (o.lock_hresult as i32) < 0
        || (o.unlock_hresult as i32) < 0
        || a.size as usize > MAX_PAYLOAD
        || payload.len() > a.size as usize
        || target.len() < a.size as usize
    {
        return Err("Invalid/failed bounded dynamic direct lock transfer".into());
    }
    let commands = vec![
        Command {
            receiver: handle,
            vtable_offset: 0x2c,
            arguments: vec![a.byte_offset, a.size, o.lock_slot, a.flags],
        },
        Command {
            receiver: callback.0,
            vtable_offset: callback.1,
            arguments: vec![o.lock_pointer],
        },
        Command {
            receiver: handle,
            vtable_offset: 0x30,
            arguments: vec![],
        },
    ];
    target[..payload.len()].copy_from_slice(payload);
    Ok(Transfer {
        allocation: a,
        copied: payload.len(),
        commands,
    })
}
/// Direct CPU mirror of a successful lock; safe failure preserves all caller state.
pub fn vertex(
    ring: &mut Vertex,
    device: &mut Device,
    source: &VertexSource,
    o: &Responses,
    target: &mut [u8],
) -> Result<Transfer, String> {
    let (next, a) = vertex_schedule(ring, source, device.vertex_limit, o.resized)?;
    let handle = next.handles[next.active as usize];
    let count = a.discard_events;
    let transfer = direct(
        a,
        handle,
        (source.address, 0x24),
        &source.payload,
        device,
        o,
        target,
    )?;
    *ring = next;
    device.discards = device.discards.wrapping_add(count);
    Ok(transfer)
}
pub fn index(
    ring: &mut Index,
    device: &Device,
    source: &IndexSource,
    o: &Responses,
    target: &mut [u8],
) -> Result<Transfer, String> {
    let (next, a) = index_schedule(ring, source, o.resized)?;
    let transfer = direct(
        a,
        next.handle,
        (source.address, 0x14),
        &source.payload,
        device,
        o,
        target,
    )?;
    *ring = next;
    Ok(transfer)
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct StreamBinding {
    pub source: u32,
    pub stride: u32,
    pub declaration: [u32; 5],
    pub shader: Shader,
    /// Final GetSize answer used by the native byte statistic, without abs().
    pub reported_size: u32,
}
/// Dynamic stream handoff always clears old tail slots and installs a resolved shader.
pub fn bind_vertex(
    state: &mut State,
    deferred: &mut Deferred,
    ring: &mut Vertex,
    device: &mut Device,
    b: StreamBinding,
) -> Result<(), String> {
    if state.previous_stream_count > 16
        || ring.address == 0
        || ring.active > 1
        || b.source == 0
        || b.shader.address == 0
    {
        return Err("Invalid resolved dynamic stream binding".into());
    }
    deferred.bindings.desired.streams[0] = [ring.handles[ring.active as usize], b.stride];
    state.wrappers[0] = ring.address;
    state.strides[0] = b.stride as u8;
    for slot in 1..usize::from(state.previous_stream_count) {
        state.wrappers[slot] = 0;
        deferred.bindings.desired.streams[slot] = [0, 0];
    }
    deferred.states.dirty |= 12;
    state.previous_stream_count = 1;
    state.stream_count = 1;
    state.declarations[0] = b.declaration;
    state.declaration = b.shader.address;
    state.hardware_vertex = 0;
    ring.source = b.source;
    deferred.bindings.desired.vertex_shader = b.shader.handle;
    device.vertex_bytes = device.vertex_bytes.wrapping_add(b.reported_size);
    Ok(())
}
/// Width four selects Device+40c4; every other getter value selects +40c0.
pub fn index_pool(width: u32) -> usize {
    usize::from(width == 4)
}
pub fn bind_index(
    state: &mut State,
    deferred: &mut Deferred,
    ring: &Index,
    device: &mut Device,
    base: u32,
    reported_size: u32,
) -> Result<(), String> {
    if ring.address == 0 {
        return Err("Missing resolved dynamic index wrapper".into());
    }
    state.index_wrapper = ring.address;
    state.base_vertex = base;
    deferred.bindings.desired.indices = [ring.handle, base];
    deferred.states.dirty |= 8;
    device.index_bytes = device.index_bytes.wrapping_add(reported_size);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Vertex, Index, Device, VertexSource, IndexSource, Responses) {
        (
            Vertex {
                address: 0x34000000,
                handles: [0x33000000, 0x33001000],
                capacity: 256,
                cursor: 33,
                active: 1,
                source: 99,
            },
            Index {
                address: 0x34000100,
                handle: 0x33002000,
                capacity: 32,
                cursor: 8,
                width: 2,
            },
            Device {
                vertex_limit: 65535,
                scratch: 0,
                discards: u32::MAX,
                vertex_bytes: u32::MAX,
                index_bytes: u32::MAX,
            },
            VertexSource {
                address: 0x35000000,
                size_before: 32,
                size_again: 32,
                stride: 32,
                payload: vec![7; 32],
            },
            IndexSource {
                address: 0x35000100,
                size: 6,
                width: 2,
                payload: vec![8; 6],
            },
            Responses {
                resized: Some([0x33100000, 0x33101000]),
                lock_hresult: 0,
                lock_pointer: 0x40000000,
                lock_slot: 0x27000000 - 0x14,
                unlock_hresult: 0,
            },
        )
    }
    #[test]
    fn vertex_aligns_cursor_and_returns_element_offset() {
        let (v, _, d, s, _, o) = fixture();
        let (n, a) = vertex_schedule(&v, &s, d.vertex_limit, o.resized).unwrap();
        assert_eq!(
            (a.byte_offset, a.element_offset, a.flags, n.cursor),
            (64, 2, 0x1000, 96)
        );
        assert_eq!(n.source, 99);
    }
    #[test]
    fn vertex_reserves_one_extra_stride_in_capacity_gate() {
        let (mut v, _, d, s, _, o) = fixture();
        v.capacity = 64;
        v.cursor = 32;
        let (n, a) = vertex_schedule(&v, &s, d.vertex_limit, o.resized).unwrap();
        assert_eq!((a.byte_offset, a.flags, n.cursor), (0, 0x2000, 32));
        v.cursor = 0;
        let (_, a) = vertex_schedule(&v, &s, d.vertex_limit, o.resized).unwrap();
        assert_eq!(a.flags, 0x1000);
    }
    #[test]
    fn second_negative_size_forces_discard_independent_of_first_size() {
        let (v, _, d, mut s, _, o) = fixture();
        s.size_again = u32::MAX;
        let (_, a) = vertex_schedule(&v, &s, d.vertex_limit, o.resized).unwrap();
        assert_eq!((a.size, a.byte_offset, a.discard_events), (32, 0, 1));
    }
    #[test]
    fn growth_can_increment_discard_stat_twice() {
        let (mut v, _, d, mut s, _, o) = fixture();
        v.capacity = 16;
        s.size_before = (-64i32) as u32;
        let (n, a) = vertex_schedule(&v, &s, d.vertex_limit, o.resized).unwrap();
        assert_eq!(n.handles, o.resized.unwrap());
        assert_eq!((n.capacity, a.size, a.discard_events), (64, 64, 2));
        assert_eq!(n.active, 1);
    }
    #[test]
    fn vertex_limit_uses_unsigned_end_division() {
        let (v, _, _, s, _, o) = fixture();
        let (_, a) = vertex_schedule(&v, &s, 2, o.resized).unwrap();
        assert_eq!(a.byte_offset, 0);
        let (_, a) = vertex_schedule(&v, &s, 3, o.resized).unwrap();
        assert_eq!(a.byte_offset, 64);
    }
    #[test]
    fn zero_stride_and_division_overflow_fail_safely() {
        let (mut v, _, d, mut s, _, o) = fixture();
        s.stride = 0;
        assert!(vertex_schedule(&v, &s, d.vertex_limit, o.resized).is_err());
        s.stride = u32::MAX;
        s.size_before = 0;
        s.size_again = 0;
        v.capacity = 0x7fffffff;
        v.cursor = 0x80000002;
        assert!(vertex_schedule(&v, &s, u32::MAX, o.resized).is_err());
    }
    #[test]
    fn vertex_copy_uses_active_handle_preserves_source_and_target_tail() {
        let (mut v, _, mut d, s, _, o) = fixture();
        let mut target = vec![0xcc; 64];
        let p = vertex(&mut v, &mut d, &s, &o, &mut target).unwrap();
        assert_eq!(p.commands[0].receiver, 0x33001000);
        assert_eq!(p.commands[0].arguments[0], 64);
        assert_eq!(&target[32..], &[0xcc; 32]);
        assert_eq!(v.source, 99);
    }
    #[test]
    fn failed_unlock_preserves_ring_counter_and_target_even_after_growth() {
        let (mut v, _, mut d, s, _, mut o) = fixture();
        v.capacity = 1;
        o.unlock_hresult = 0x80004005;
        let mut target = vec![0xcc; 64];
        let old = (v.clone(), d.clone(), target.clone());
        assert!(vertex(&mut v, &mut d, &s, &o, &mut target).is_err());
        assert_eq!((v, d, target), old);
    }
    #[test]
    fn scratch_mode_is_an_explicit_safe_boundary() {
        let (mut v, _, mut d, s, _, o) = fixture();
        d.scratch = 1;
        let mut target = vec![0xcc; 64];
        let old = (v.clone(), d.clone(), target.clone());
        assert!(vertex(&mut v, &mut d, &s, &o, &mut target).is_err());
        assert_eq!((v, d, target), old);
    }
    #[test]
    fn index_exact_capacity_is_usable_then_wraps() {
        let (_, mut i, _, _, mut s, o) = fixture();
        i.cursor = 26;
        let (n, a) = index_schedule(&i, &s, o.resized).unwrap();
        assert_eq!((n.cursor, a.flags, a.element_offset), (32, 0x1000, 13));
        s.size = 2;
        let (n, a) = index_schedule(&n, &s, o.resized).unwrap();
        assert_eq!((n.cursor, a.flags, a.element_offset), (2, 0x2000, 0));
    }
    #[test]
    fn index_return_divisor_is_source_width_and_zero_size_is_not_clamped() {
        let (_, i, _, _, mut s, o) = fixture();
        s.width = 4;
        s.size = 0;
        let (n, a) = index_schedule(&i, &s, o.resized).unwrap();
        assert_eq!((a.size, a.element_offset, n.width, n.cursor), (0, 2, 2, 8));
        s.width = 0;
        assert!(index_schedule(&i, &s, o.resized).is_err());
    }
    #[test]
    fn index_copy_grows_and_preserves_stored_width() {
        let (_, mut i, d, _, mut s, o) = fixture();
        s.size = 40;
        s.payload = vec![9; 40];
        s.width = 4;
        let mut target = vec![0xcc; 48];
        let p = index(&mut i, &d, &s, &o, &mut target).unwrap();
        assert!(p.allocation.resized);
        assert_eq!((i.width, i.cursor, i.handle), (2, 40, 0x33100000));
        assert_eq!(&target[40..], &[0xcc; 8]);
    }
    fn state() -> (State, Deferred) {
        let s = State {
            declaration: 9,
            hardware_vertex: 8,
            declarations: [[99; 5]; 16],
            stream_count: 3,
            wrappers: [0; 16],
            strides: [255; 16],
            previous_stream_count: 3,
            index_wrapper: 0,
            base_vertex: 7,
        };
        let v = crate::d3d_state::Values {
            render: [0; 32],
            stages: [[0; 21]; 8],
            textures: [0; 8],
        };
        let b = crate::d3d_bindings::Values {
            vertex_shader: 0,
            pixel_shader: 0,
            streams: [[99; 2]; 16],
            indices: [0; 2],
        };
        (
            s,
            Deferred {
                states: crate::d3d_state::Cache {
                    desired: v.clone(),
                    applied: v,
                    dirty: 0,
                },
                transforms: crate::d3d_transforms::Transforms {
                    words: [[0; 16]; 11],
                    mask: 0,
                },
                bindings: crate::d3d_bindings::Bindings {
                    desired: b.clone(),
                    applied: b,
                },
            },
        )
    }
    #[test]
    fn dynamic_binding_clears_tail_even_when_renderer_wrappers_are_zero() {
        let (mut v, _, mut d, _, _, _) = fixture();
        let (mut s, mut cache) = state();
        bind_vertex(
            &mut s,
            &mut cache,
            &mut v,
            &mut d,
            StreamBinding {
                source: 3,
                stride: 0x123,
                declaration: [1; 5],
                shader: Shader {
                    address: 4,
                    handle: 5,
                },
                reported_size: 2,
            },
        )
        .unwrap();
        assert_eq!(cache.bindings.desired.streams[1], [0, 0]);
        assert_eq!(cache.bindings.desired.streams[2], [0, 0]);
        assert_eq!(cache.bindings.desired.streams[3], [99, 99]);
        assert_eq!(
            (
                s.strides[0],
                s.strides[1],
                s.stream_count,
                s.hardware_vertex,
                v.source,
                d.vertex_bytes
            ),
            (0x23, 255, 1, 0, 3, 1)
        );
        assert_eq!(cache.states.dirty, 12);
        assert_eq!(s.declarations[1], [99; 5]);
        assert_eq!(cache.bindings.desired.vertex_shader, 5);
    }
    #[test]
    fn invalid_shader_preserves_all_dynamic_binding_state() {
        let (mut v, _, mut d, _, _, _) = fixture();
        let (mut s, mut cache) = state();
        let old = (v.clone(), d.clone(), s.clone(), cache.clone());
        assert!(bind_vertex(
            &mut s,
            &mut cache,
            &mut v,
            &mut d,
            StreamBinding {
                source: 3,
                stride: 32,
                declaration: [1; 5],
                shader: Shader {
                    address: 0,
                    handle: 5
                },
                reported_size: 2
            }
        )
        .is_err());
        assert_eq!((v, d, s, cache), old);
    }
    #[test]
    fn dynamic_index_binding_keeps_base_separate_from_ring_return_offset() {
        let (_, i, mut d, _, _, _) = fixture();
        let (mut s, mut cache) = state();
        bind_index(&mut s, &mut cache, &i, &mut d, 99, 6).unwrap();
        assert_eq!(cache.bindings.desired.indices, [i.handle, 99]);
        assert_eq!(
            (s.base_vertex, s.index_wrapper, d.index_bytes),
            (99, i.address, 5)
        );
        assert_eq!(
            [
                index_pool(0),
                index_pool(2),
                index_pool(4),
                index_pool(u32::MAX)
            ],
            [0, 0, 1, 0]
        );
    }
}
