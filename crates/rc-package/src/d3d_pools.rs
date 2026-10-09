//! Native dynamic wrapper constructors and lazy per-device pools.
//! D3DDrv 1002c120/1002c460/10020540/10020a90; allocations remain supplied images.
use crate::{
    d3d_bindings::Deferred,
    d3d_buffers::State,
    d3d_dynamic as dynamic,
    d3d_resize::{self, Context, Responses, UploadResponses},
    d3d_resource::{Cache, Kind, Node},
    d3d_scratch,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Answers {
    pub allocation: Node,
    pub creates: Responses,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Initialization {
    pub allocation_size: u32,
    pub address: u32,
    pub key: [u32; 2],
    pub resize: d3d_resize::Plan,
}
fn word(n: &Node, offset: usize) -> u32 {
    u32::from_le_bytes(n.bytes[offset..offset + 4].try_into().unwrap())
}
fn put(n: &mut Node, offset: usize, value: u32) {
    n.bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn node(cache: &Cache, address: u32, vertex: bool) -> Result<&Node, String> {
    let n = cache
        .nodes
        .iter()
        .find(|n| n.address == address)
        .ok_or("Missing captured dynamic pool")?;
    if n.bytes.len() != if vertex { 72 } else { 64 }
        || word(n, 0) != if vertex { 0x10072f68 } else { 0x10073018 }
        || word(n, 4) != cache.device
    {
        return Err("Dynamic pool type, extent or owner mismatch".into());
    }
    Ok(n)
}
pub fn vertex_view(cache: &Cache, address: u32) -> Result<dynamic::Vertex, String> {
    let n = node(cache, address, true)?;
    Ok(dynamic::Vertex {
        address,
        handles: [word(n, 0x30), word(n, 0x34)],
        capacity: word(n, 0x38),
        source: word(n, 0x3c),
        cursor: word(n, 0x40),
        active: word(n, 0x44),
    })
}
pub fn index_view(cache: &Cache, address: u32) -> Result<dynamic::Index, String> {
    let n = node(cache, address, false)?;
    Ok(dynamic::Index {
        address,
        handle: word(n, 0x30),
        capacity: word(n, 0x34),
        cursor: word(n, 0x38),
        width: word(n, 0x3c),
    })
}
fn store_vertex(cache: &mut Cache, ring: &dynamic::Vertex) -> Result<(), String> {
    vertex_view(cache, ring.address)?;
    let n = cache
        .nodes
        .iter_mut()
        .find(|n| n.address == ring.address)
        .unwrap();
    for (off, v) in [
        (0x30, ring.handles[0]),
        (0x34, ring.handles[1]),
        (0x38, ring.capacity),
        (0x3c, ring.source),
        (0x40, ring.cursor),
        (0x44, ring.active),
    ] {
        put(n, off, v);
    }
    Ok(())
}
fn store_index(cache: &mut Cache, ring: &dynamic::Index) -> Result<(), String> {
    index_view(cache, ring.address)?;
    let n = cache
        .nodes
        .iter_mut()
        .find(|n| n.address == ring.address)
        .unwrap();
    for (off, v) in [
        (0x30, ring.handle),
        (0x34, ring.capacity),
        (0x38, ring.cursor),
        (0x3c, ring.width),
    ] {
        put(n, off, v);
    }
    Ok(())
}
fn allocation(
    cache: &mut Cache,
    supplied: &Node,
    key: [u32; 2],
    size: usize,
) -> Result<(), String> {
    if supplied.address == 0
        || supplied.bytes.len() != size
        || supplied.address.checked_add(size as u32).is_none()
        || cache.nodes.iter().any(|n| n.address == supplied.address)
    {
        return Err("Invalid, duplicate or missing dynamic wrapper allocation".into());
    }
    cache.nodes.push(supplied.clone());
    cache.insert(supplied.address, key, Kind::Base)
}
/// Base links/key followed by dynamic vtable, zeroed payload and initial 128 KiB pair.
pub fn construct_vertex(
    cache: &mut Cache,
    deferred: &mut Deferred,
    key: [u32; 2],
    context: Context,
    answers: &Answers,
) -> Result<Initialization, String> {
    let mut next = cache.clone();
    let mut d = deferred.clone();
    let address = answers.allocation.address;
    allocation(&mut next, &answers.allocation, key, 72)?;
    let n = next.nodes.last_mut().unwrap();
    put(n, 0, 0x10072f68);
    for off in [0x30, 0x34, 0x38, 0x3c, 0x44] {
        put(n, off, 0);
    }
    let mut ring = vertex_view(&next, address)?;
    let resize = d3d_resize::vertex(&mut ring, &mut d, 0x20000, context, &answers.creates)?;
    store_vertex(&mut next, &ring)?;
    *cache = next;
    *deferred = d;
    Ok(Initialization {
        allocation_size: 72,
        address,
        key,
        resize,
    })
}
/// Base links/key followed by dynamic index vtable and initial 16 KiB allocation.
pub fn construct_index(
    cache: &mut Cache,
    key: [u32; 2],
    width: u32,
    context: Context,
    answers: &Answers,
) -> Result<Initialization, String> {
    let mut next = cache.clone();
    let address = answers.allocation.address;
    allocation(&mut next, &answers.allocation, key, 64)?;
    let n = next.nodes.last_mut().unwrap();
    put(n, 0, 0x10073018);
    put(n, 0x30, 0);
    put(n, 0x34, 0);
    put(n, 0x3c, width);
    let mut ring = index_view(&next, address)?;
    let resize = d3d_resize::index(&mut ring, 0x4000, context.device, &answers.creates)?;
    store_index(&mut next, &ring)?;
    *cache = next;
    Ok(Initialization {
        allocation_size: 64,
        address,
        key,
        resize,
    })
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Pools {
    pub vertex: u32,
    pub indices: [u32; 2],
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Runtime {
    pub cache: Cache,
    pub pools: Pools,
    pub device: dynamic::Device,
    pub deferred: Deferred,
    pub scratch: d3d_scratch::Buffer,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VertexRequest {
    pub source: dynamic::VertexSource,
    pub key: [u32; 2],
    pub pool: Answers,
    pub context: Context,
    pub upload: UploadResponses,
    pub array: d3d_scratch::Answers,
    pub binding: dynamic::StreamBinding,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndexRequest {
    pub source: dynamic::IndexSource,
    pub key: [u32; 2],
    pub pool: Answers,
    pub context: Context,
    pub upload: UploadResponses,
    pub array: d3d_scratch::Answers,
    pub base: u32,
    pub reported_size: u32,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Transfer {
    pub initialization: Option<Initialization>,
    pub transfer: d3d_scratch::Transfer,
    pub offset: u32,
}
impl Runtime {
    /// A populated device slot is reused directly, regardless of a new source key.
    pub fn ensure_vertex(
        &mut self,
        key: [u32; 2],
        context: Context,
        answers: &Answers,
    ) -> Result<Option<Initialization>, String> {
        if self.pools.vertex != 0 {
            vertex_view(&self.cache, self.pools.vertex)?;
            return Ok(None);
        }
        let initialized =
            construct_vertex(&mut self.cache, &mut self.deferred, key, context, answers)?;
        self.pools.vertex = initialized.address;
        Ok(Some(initialized))
    }
    /// Only source width four selects the second slot; other widths create a width-two pool.
    pub fn ensure_index(
        &mut self,
        key: [u32; 2],
        width: u32,
        context: Context,
        answers: &Answers,
    ) -> Result<Option<Initialization>, String> {
        let slot = dynamic::index_pool(width);
        if self.pools.indices[slot] != 0 {
            index_view(&self.cache, self.pools.indices[slot])?;
            return Ok(None);
        }
        let initialized = construct_index(
            &mut self.cache,
            key,
            if slot == 1 { 4 } else { 2 },
            context,
            answers,
        )?;
        self.pools.indices[slot] = initialized.address;
        Ok(Some(initialized))
    }
    fn upload_runtime(
        &self,
        vertex: Option<dynamic::Vertex>,
        index: Option<dynamic::Index>,
    ) -> d3d_scratch::Runtime {
        d3d_scratch::Runtime {
            buffers: d3d_resize::Runtime {
                vertex: vertex.unwrap_or(dynamic::Vertex {
                    address: 0,
                    handles: [0; 2],
                    capacity: 0,
                    cursor: 0,
                    active: 0,
                    source: 0,
                }),
                index: index.unwrap_or(dynamic::Index {
                    address: 0,
                    handle: 0,
                    capacity: 0,
                    cursor: 0,
                    width: 0,
                }),
                device: self.device.clone(),
                deferred: self.deferred.clone(),
            },
            scratch: self.scratch.clone(),
        }
    }
    pub fn vertex(
        &mut self,
        state: &mut State,
        request: &VertexRequest,
        target: &mut [u8],
    ) -> Result<Transfer, String> {
        let mut next = self.clone();
        let mut s = state.clone();
        let mut bytes = target.to_vec();
        let initialization = next.ensure_vertex(request.key, request.context, &request.pool)?;
        let mut r = next.upload_runtime(Some(vertex_view(&next.cache, next.pools.vertex)?), None);
        let transfer = r.vertex(
            &request.source,
            request.context,
            &request.upload,
            request.array,
            &mut bytes,
        )?;
        dynamic::bind_vertex(
            &mut s,
            &mut r.buffers.deferred,
            &mut r.buffers.vertex,
            &mut r.buffers.device,
            request.binding,
        )?;
        store_vertex(&mut next.cache, &r.buffers.vertex)?;
        next.device = r.buffers.device;
        next.deferred = r.buffers.deferred;
        next.scratch = r.scratch;
        let offset = transfer.upload.allocation.element_offset;
        *self = next;
        *state = s;
        target.copy_from_slice(&bytes);
        Ok(Transfer {
            initialization,
            transfer,
            offset,
        })
    }
    pub fn index(
        &mut self,
        state: &mut State,
        request: &IndexRequest,
        target: &mut [u8],
    ) -> Result<Transfer, String> {
        let mut next = self.clone();
        let mut s = state.clone();
        let mut bytes = target.to_vec();
        let initialization = next.ensure_index(
            request.key,
            request.source.width,
            request.context,
            &request.pool,
        )?;
        let address = next.pools.indices[dynamic::index_pool(request.source.width)];
        let mut r = next.upload_runtime(None, Some(index_view(&next.cache, address)?));
        let transfer = r.index(
            &request.source,
            request.context,
            &request.upload,
            request.array,
            &mut bytes,
        )?;
        dynamic::bind_index(
            &mut s,
            &mut r.buffers.deferred,
            &r.buffers.index,
            &mut r.buffers.device,
            request.base,
            request.reported_size,
        )?;
        store_index(&mut next.cache, &r.buffers.index)?;
        next.device = r.buffers.device;
        next.deferred = r.buffers.deferred;
        next.scratch = r.scratch;
        let offset = transfer.upload.allocation.element_offset;
        *self = next;
        *state = s;
        target.copy_from_slice(&bytes);
        Ok(Transfer {
            initialization,
            transfer,
            offset,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Runtime, Context, Answers) {
        let values = crate::d3d_state::Values {
            render: [0; 32],
            stages: [[0; 21]; 8],
            textures: [0; 8],
        };
        let bindings = crate::d3d_bindings::Values {
            vertex_shader: 0,
            pixel_shader: 0,
            streams: [[0; 2]; 16],
            indices: [0; 2],
        };
        (
            Runtime {
                cache: Cache {
                    device: 0x22000000,
                    head: 0,
                    buckets: vec![0; 4096],
                    nodes: vec![],
                },
                pools: Pools {
                    vertex: 0,
                    indices: [0; 2],
                },
                device: dynamic::Device {
                    vertex_limit: 65535,
                    scratch: 1,
                    discards: 0,
                    vertex_bytes: 0,
                    index_bytes: 0,
                },
                deferred: Deferred {
                    states: crate::d3d_state::Cache {
                        desired: values.clone(),
                        applied: values,
                        dirty: 0,
                    },
                    transforms: crate::d3d_transforms::Transforms {
                        words: [[0; 16]; 11],
                        mask: 0,
                    },
                    bindings: crate::d3d_bindings::Bindings {
                        desired: bindings.clone(),
                        applied: bindings,
                    },
                },
                scratch: d3d_scratch::Buffer {
                    address: 0x41000000,
                    packed_count: 32,
                    bytes: vec![0xa5; 32],
                },
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
            Answers {
                allocation: Node {
                    address: 0x34000000,
                    bytes: vec![0xa5; 72],
                },
                creates: Responses {
                    creates: vec![
                        crate::d3d_upload::Created {
                            hresult: 0,
                            handle: 11,
                        },
                        crate::d3d_upload::Created {
                            hresult: 0,
                            handle: 22,
                        },
                    ],
                    evictions: vec![],
                },
            },
        )
    }
    fn index_answers(a: &Answers, address: u32) -> Answers {
        let mut a = a.clone();
        a.allocation.address = address;
        a.allocation.bytes = vec![0xa5; 64];
        a
    }
    fn state() -> State {
        State {
            declaration: 0,
            hardware_vertex: 0,
            declarations: [[0; 5]; 16],
            stream_count: 0,
            wrappers: [0; 16],
            strides: [0; 16],
            previous_stream_count: 0,
            index_wrapper: 0,
            base_vertex: 0,
        }
    }
    fn request(c: Context, a: Answers) -> VertexRequest {
        VertexRequest {
            source: dynamic::VertexSource {
                address: 1,
                size_before: 7,
                size_again: 7,
                stride: 3,
                payload: vec![1, 2],
            },
            key: [0x1234, 9],
            pool: a,
            context: c,
            upload: UploadResponses {
                resize: Responses {
                    creates: vec![],
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
            array: d3d_scratch::Answers {
                reset_pointer: 0,
                growth_pointer: 0x42000000,
            },
            binding: dynamic::StreamBinding {
                source: 1,
                stride: 3,
                declaration: [0; 5],
                shader: crate::d3d_buffers::Shader {
                    address: 0x35000000,
                    handle: 55,
                },
                reported_size: 7,
            },
        }
    }
    #[test]
    fn vertex_constructor_initializes_pair_and_floor() {
        let (mut r, c, a) = fixture();
        let p = r.ensure_vertex([1, 2], c, &a).unwrap().unwrap();
        let v = vertex_view(&r.cache, p.address).unwrap();
        assert_eq!(p.allocation_size, 72);
        assert_eq!(v.handles, [11, 22]);
        assert_eq!(
            (v.capacity, v.cursor, v.active, v.source),
            (0x20000, 0, 0, 0)
        );
        assert_eq!(p.resize.commands.len(), 2);
    }
    #[test]
    fn index_constructor_initializes_format_and_floor() {
        let (mut r, c, a) = fixture();
        let a = index_answers(&a, 0x34000100);
        let p = r.ensure_index([1, 2], 4, c, &a).unwrap().unwrap();
        let i = index_view(&r.cache, p.address).unwrap();
        assert_eq!((i.width, i.capacity, i.cursor), (4, 0x4000, 0));
        assert_eq!(p.resize.format, Some(0x66));
        assert_eq!(p.allocation_size, 64);
    }
    #[test]
    fn constructor_preserves_unwritten_base_bytes() {
        let (mut r, c, a) = fixture();
        r.ensure_vertex([1, 2], c, &a).unwrap();
        let n = &r.cache.nodes[0];
        assert_eq!(&n.bytes[0x1e..0x20], &[0xa5, 0xa5]);
        assert_eq!(word(n, 0x14), 0);
        assert_eq!(word(n, 0), 0x10072f68);
    }
    #[test]
    fn both_constructors_prepend_global_and_colliding_hash_lists() {
        let (mut r, c, a) = fixture();
        r.ensure_vertex([1, 2], c, &a).unwrap();
        r.ensure_index([1, 3], 2, c, &index_answers(&a, 0x34000100))
            .unwrap();
        assert_eq!(r.cache.head, 0x34000100);
        assert_eq!(word(&r.cache.nodes[1], 0x28), a.allocation.address);
        assert_eq!(word(&r.cache.nodes[1], 0x2c), a.allocation.address);
        assert_eq!(r.cache.lookup([1, 2]).unwrap(), Some(a.allocation.address));
    }
    #[test]
    fn vertex_reuse_ignores_new_source_key_and_bad_allocation() {
        let (mut r, c, mut a) = fixture();
        r.ensure_vertex([1, 2], c, &a).unwrap();
        let old = r.clone();
        a.allocation.address = 0;
        a.creates.creates.clear();
        assert!(r.ensure_vertex([7, 8], c, &a).unwrap().is_none());
        assert_eq!(r, old);
    }
    #[test]
    fn non_four_widths_share_a_width_two_pool() {
        let (mut r, c, a) = fixture();
        let a = index_answers(&a, 0x34000100);
        r.ensure_index([1, 2], 8, c, &a).unwrap();
        let old = r.clone();
        assert!(r.ensure_index([9, 8], 3, c, &a).unwrap().is_none());
        assert_eq!(index_view(&r.cache, a.allocation.address).unwrap().width, 2);
        assert_eq!(r, old);
    }
    #[test]
    fn index_pools_have_independent_handles_and_keys() {
        let (mut r, c, a) = fixture();
        r.ensure_index([1, 2], 2, c, &index_answers(&a, 0x34000100))
            .unwrap();
        let mut large = index_answers(&a, 0x34000200);
        large.creates.creates[0].handle = 33;
        r.ensure_index([1, 3], 4, c, &large).unwrap();
        assert_eq!(r.pools.indices, [0x34000100, 0x34000200]);
        assert_eq!(index_view(&r.cache, 0x34000100).unwrap().handle, 11);
        assert_eq!(index_view(&r.cache, 0x34000200).unwrap().handle, 33);
    }
    #[test]
    fn failed_constructor_preserves_cache_deferred_and_pool_slot() {
        let (mut r, c, mut a) = fixture();
        a.creates.creates[0].hresult = 0x80004005;
        let old = r.clone();
        assert!(r.ensure_vertex([1, 2], c, &a).is_err());
        assert_eq!(r, old);
    }
    #[test]
    fn null_allocation_is_atomic() {
        let (mut r, c, mut a) = fixture();
        a.allocation.address = 0;
        let old = r.clone();
        assert!(r.ensure_vertex([1, 2], c, &a).is_err());
        assert_eq!(r, old);
    }
    #[test]
    fn wrong_extent_is_atomic() {
        let (mut r, c, mut a) = fixture();
        a.allocation.bytes.pop();
        let old = r.clone();
        assert!(r.ensure_vertex([1, 2], c, &a).is_err());
        assert_eq!(r, old);
    }
    #[test]
    fn duplicate_allocation_cannot_replace_existing_pool_image() {
        let (mut r, c, a) = fixture();
        r.ensure_vertex([1, 2], c, &a).unwrap();
        let old = r.clone();
        assert!(r
            .ensure_index([1, 3], 2, c, &index_answers(&a, a.allocation.address))
            .is_err());
        assert_eq!(r, old);
    }
    #[test]
    fn typed_view_rejects_wrong_vtable_and_owner() {
        let (mut r, c, a) = fixture();
        r.ensure_vertex([1, 2], c, &a).unwrap();
        assert!(index_view(&r.cache, a.allocation.address).is_err());
        put(&mut r.cache.nodes[0], 4, 0);
        assert!(vertex_view(&r.cache, a.allocation.address).is_err());
    }
    #[test]
    fn vertex_upload_stores_ring_source_cursor_and_cache_links() {
        let (mut r, c, a) = fixture();
        let req = request(c, a);
        let mut s = state();
        let mut target = vec![9; 10];
        let t = r.vertex(&mut s, &req, &mut target).unwrap();
        let v = vertex_view(&r.cache, r.pools.vertex).unwrap();
        assert!(t.initialization.is_some());
        assert_eq!((v.source, v.cursor, t.offset), (1, 7, 0));
        assert_eq!(s.wrappers[0], r.pools.vertex);
        assert_eq!(&target[..7], &[1, 2, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5]);
        assert_eq!(r.cache.lookup(req.key).unwrap(), Some(r.pools.vertex));
    }
    #[test]
    fn late_shader_error_rolls_back_new_pool_and_cpu_bytes() {
        let (mut r, c, a) = fixture();
        let mut req = request(c, a);
        req.binding.shader.address = 0;
        let old = r.clone();
        let mut s = state();
        let old_s = s.clone();
        let mut target = vec![9; 10];
        assert!(r.vertex(&mut s, &req, &mut target).is_err());
        assert_eq!(r, old);
        assert_eq!(s, old_s);
        assert_eq!(target, vec![9; 10]);
    }
    #[test]
    fn index_offset_and_base_stay_separate() {
        let (mut r, c, a) = fixture();
        let v = request(c, a.clone());
        let req = IndexRequest {
            source: dynamic::IndexSource {
                address: 2,
                size: 7,
                width: 3,
                payload: vec![4],
            },
            key: [2, 3],
            pool: index_answers(&a, 0x34000100),
            context: c,
            upload: v.upload,
            array: v.array,
            base: 123,
            reported_size: 7,
        };
        let mut s = state();
        let mut target = vec![9; 10];
        r.index(&mut s, &req, &mut target).unwrap();
        let t = r.index(&mut s, &req, &mut target).unwrap();
        assert!(t.initialization.is_none());
        assert_eq!(t.offset, 2);
        assert_eq!(s.base_vertex, 123);
        assert_eq!(r.deferred.bindings.desired.indices, [11, 123]);
        assert_eq!(index_view(&r.cache, r.pools.indices[0]).unwrap().width, 2);
    }
    #[test]
    fn resource_cache_accepts_dynamic_extent_but_rejects_other_sizes() {
        let (mut r, c, a) = fixture();
        r.ensure_vertex([1, 2], c, &a).unwrap();
        assert!(r.cache.lookup([1, 2]).is_ok());
        r.cache.nodes[0].bytes.push(0);
        assert!(r.cache.lookup([1, 2]).is_err());
    }
}
