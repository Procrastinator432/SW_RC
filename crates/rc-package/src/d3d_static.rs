//! Captured static sources through resource lookup, CPU upload mirrors and draw planning.
//! Allocation images, source revision/owner callbacks, COM and shader results stay explicit.
use crate::{
    d3d_bindings::Deferred,
    d3d_buffers as buffers,
    d3d_resource::{Cache, Kind},
    d3d_source as source, d3d_upload as upload,
    shader_snapshot::Memory,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Target {
    pub address: u32,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Resources {
    pub cache: Cache,
    pub targets: Vec<Target>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Vertex {
    pub address: u32,
    pub allocation: Option<u32>,
    pub revision: u32,
    pub revision_after: u32,
    pub initial_components: [u8; 16],
    pub owner_count: Option<u32>,
    pub delegate: Option<Vec<u8>>,
    pub dynamic: u32,
    pub special: u32,
    pub responses: upload::Responses,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Index {
    pub address: u32,
    pub allocation: Option<u32>,
    pub width: source::IndexWidth,
    /// Revision seen by SetIndexBuffer, before entering the upload function.
    pub revision: u32,
    /// The upload function reads this getter again before deciding to fill.
    pub revision_before: u32,
    pub revision_after: u32,
    pub responses: upload::Responses,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub streams: Vec<Vertex>,
    pub index: Option<Index>,
    pub shader_kind: u32,
    pub shader: Option<buffers::Shader>,
    pub base_vertex: u32,
    pub frame: u32,
    /// Actual COM device; cache.device is the owning UD3DRenderDevice instead.
    pub device: upload::Device,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Resolution {
    pub wrapper: u32,
    pub created: bool,
    pub cached_revision: u32,
    /// None when the outer binding revision gate skipped the upload function.
    pub transfer: Option<source::Transfer>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Resolved {
    pub request: buffers::Request,
    pub streams: Vec<Resolution>,
    pub index: Option<Resolution>,
}
fn key(memory: &Memory, address: u32) -> Result<[u32; 2], String> {
    let at = address
        .checked_add(4)
        .filter(|_| address != 0)
        .ok_or("Invalid source key address")?;
    let b = memory.read(at, 8)?;
    Ok([
        u32::from_le_bytes(b[..4].try_into().unwrap()),
        u32::from_le_bytes(b[4..].try_into().unwrap()),
    ])
}
impl Resources {
    fn validate(&self) -> Result<(), String> {
        if self.targets.len() > 32 {
            return Err("Too many captured CPU lock targets".into());
        }
        let mut seen = std::collections::HashSet::new();
        let mut total = 0usize;
        for t in &self.targets {
            total = total
                .checked_add(t.bytes.len())
                .ok_or("CPU target size overflow")?;
            if t.address == 0
                || !seen.insert(t.address)
                || t.bytes.len() > source::MAX_PAYLOAD
                || u64::from(t.address) + t.bytes.len() as u64 > 1u64 << 32
            {
                return Err("Invalid or duplicate CPU lock target".into());
            }
        }
        if total > 4 * source::MAX_PAYLOAD {
            return Err("Total CPU lock mirrors exceed bounded limit".into());
        }
        Ok(())
    }
    fn target(&mut self, address: u32) -> Result<&mut [u8], String> {
        self.targets
            .iter_mut()
            .find(|t| t.address == address)
            .map(|t| t.bytes.as_mut_slice())
            .ok_or("Uncaptured CPU lock target".into())
    }
    fn resolve(
        &mut self,
        key: [u32; 2],
        allocation: Option<u32>,
        kind: Kind,
    ) -> Result<(upload::Resource, bool), String> {
        let (address, created) = match self.cache.lookup(key)? {
            Some(a) => (a, false),
            None => {
                let a = allocation.ok_or("Cache miss requires supplied wrapper allocation")?;
                self.cache.insert(a, key, kind)?;
                (a, true)
            }
        };
        Ok((self.cache.upload_resource(address, kind)?, created))
    }
    fn resolve_inner(&mut self, memory: &Memory, r: &Request) -> Result<Resolved, String> {
        self.validate()?;
        if r.streams.len() > 16 {
            return Err("Static source count exceeds sixteen slots".into());
        }
        let mut streams = vec![];
        let mut bindings = vec![];
        for s in &r.streams {
            let (mut resource, created) =
                self.resolve(key(memory, s.address)?, s.allocation, Kind::Vertex)?;
            let cached_revision = resource.revision;
            let changed = cached_revision != s.revision;
            let skin = if changed {
                Some(source::Skin::capture(memory, s.address)?)
            } else {
                None
            };
            let size = if let Some(skin) = skin {
                skin.size(s.owner_count)?
            } else {
                0
            };
            let transfer = if changed {
                Some(source::vertex(
                    &mut resource,
                    &source::VertexData {
                        skin: skin.unwrap(),
                        owner_count: s.owner_count,
                        delegate: s.delegate.clone(),
                        dynamic: s.dynamic,
                        special: s.special,
                        revision_after: s.revision_after,
                    },
                    memory,
                    r.device,
                    &s.responses,
                    self.target(s.responses.lock_pointer)?,
                )?)
            } else {
                None
            };
            self.cache.store_upload(&resource, Kind::Vertex, r.frame)?;
            bindings.push(buffers::Stream {
                declaration: source::components(s.initial_components),
                source_revision: s.revision,
                cached_revision,
                upload_size: size,
                wrapper: resource.address,
                handle: resource.handle,
                stride: source::stride(),
            });
            streams.push(Resolution {
                wrapper: resource.address,
                created,
                cached_revision,
                transfer,
            });
        }
        let mut index_binding = buffers::Index {
            source: 0,
            size: 0,
            source_revision: 0,
            cached_revision: 0,
            wrapper: 0,
            handle: 0,
        };
        let mut index = None;
        if let Some(s) = &r.index {
            if s.address != 0 {
                let data = source::RawIndex::capture(memory, s.address, s.width)?;
                let size = data.size();
                index_binding.source = s.address;
                index_binding.size = size;
                // Original zero-size path precedes key lookup and allocation.
                if size != 0 {
                    let (mut resource, created) =
                        self.resolve(key(memory, s.address)?, s.allocation, Kind::Index)?;
                    let cached_revision = resource.revision;
                    let transfer = if cached_revision != s.revision {
                        let data = source::IndexData {
                            index: data,
                            revision_before: s.revision_before,
                            revision_after: s.revision_after,
                        };
                        // A matching inner revision skips Lock and all source/target reads.
                        let target = if cached_revision != s.revision_before {
                            self.target(s.responses.lock_pointer)?
                        } else {
                            &mut []
                        };
                        Some(source::index(
                            &mut resource,
                            data,
                            memory,
                            r.device,
                            &s.responses,
                            target,
                        )?)
                    } else {
                        None
                    };
                    self.cache.store_upload(&resource, Kind::Index, r.frame)?;
                    index_binding = buffers::Index {
                        source: s.address,
                        size,
                        source_revision: s.revision,
                        cached_revision,
                        wrapper: resource.address,
                        handle: resource.handle,
                    };
                    index = Some(Resolution {
                        wrapper: resource.address,
                        created,
                        cached_revision,
                        transfer,
                    });
                }
            }
        }
        Ok(Resolved {
            request: buffers::Request {
                streams: bindings,
                index: index_binding,
                shader_kind: r.shader_kind,
                shader: r.shader,
                base_vertex: r.base_vertex,
                frame: r.frame,
            },
            streams,
            index,
        })
    }
    /// Resolve sequentially, so aliased sources observe revisions written by earlier slots.
    pub fn resolve_sources(
        &mut self,
        memory: &Memory,
        request: &Request,
    ) -> Result<Resolved, String> {
        let mut next = self.clone();
        let result = next.resolve_inner(memory, request)?;
        *self = next;
        Ok(result)
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Preparation {
    pub resolved: Resolved,
    pub bindings: buffers::Preparation,
}
/// Port transaction: failures preserve all resource images, CPU targets and bind state.
pub fn prepare(
    resources: &mut Resources,
    state: &mut buffers::State,
    deferred: &mut Deferred,
    memory: &Memory,
    request: &Request,
) -> Result<Preparation, String> {
    let mut next = resources.clone();
    let mut s = state.clone();
    let mut d = deferred.clone();
    let resolved = next.resolve_inner(memory, request)?;
    let bindings = buffers::prepare(&mut s, &mut d, &resolved.request)?;
    *resources = next;
    *state = s;
    *deferred = d;
    Ok(Preparation { resolved, bindings })
}
#[derive(Clone, Debug, Serialize)]
pub struct SubmissionPlan {
    pub resolved: Resolved,
    pub submission: buffers::SubmissionPlan,
}
pub fn submit(
    resources: &mut Resources,
    submission: &mut buffers::Submission,
    memory: &Memory,
    request: &Request,
    context: crate::d3d_draw::Context,
    draw: crate::d3d_draw::Draw,
) -> Result<SubmissionPlan, String> {
    let mut next = resources.clone();
    let mut s = submission.clone();
    let resolved = next.resolve_inner(memory, request)?;
    let plan = buffers::submit(&mut s, &resolved.request, context, draw)?;
    *resources = next;
    *submission = s;
    Ok(SubmissionPlan {
        resolved,
        submission: plan,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{d3d_resource::Node, shader_snapshot::Region};
    fn memory(vc: u32, ic: u32) -> Memory {
        let mut v = vec![0; 36];
        let mut i = vec![0; 24];
        for (b, fields) in [
            (
                &mut v,
                vec![(4, 11u32), (8, 22), (0x1c, 0x2000), (0x20, vc)],
            ),
            (&mut i, vec![(4, 12), (8, 22), (0x10, 0x2100), (0x14, ic)]),
        ] {
            for (off, value) in fields {
                b[off..off + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        Memory::new(vec![
            Region {
                address: 0x1000,
                bytes: v,
            },
            Region {
                address: 0x1100,
                bytes: i,
            },
            Region {
                address: 0x2000,
                bytes: (0..128).collect(),
            },
            Region {
                address: 0x2100,
                bytes: (128..160).collect(),
            },
        ])
        .unwrap()
    }
    fn response(handle: u32, target: u32) -> upload::Responses {
        upload::Responses {
            creates: vec![upload::Created { hresult: 0, handle }],
            evictions: vec![],
            lock_hresult: 0,
            lock_pointer: target,
            unlock_hresult: 0,
            lock_slot: 0x7000,
        }
    }
    fn fixture() -> (Resources, Request) {
        (
            Resources {
                cache: Cache {
                    device: 0x22000000,
                    head: 0,
                    buckets: vec![0; 4096],
                    nodes: (0..8)
                        .map(|n| Node {
                            address: 0x3000 + n * 0x100,
                            bytes: vec![0xcc; 64],
                        })
                        .collect(),
                },
                targets: vec![
                    Target {
                        address: 0x4000,
                        bytes: vec![0xaa; 128],
                    },
                    Target {
                        address: 0x4100,
                        bytes: vec![0xbb; 128],
                    },
                ],
            },
            Request {
                streams: vec![Vertex {
                    address: 0x1000,
                    allocation: Some(0x3000),
                    revision: 1,
                    revision_after: 1,
                    initial_components: [0xff; 16],
                    owner_count: None,
                    delegate: None,
                    dynamic: 0,
                    special: 0,
                    responses: response(0x5000, 0x4000),
                }],
                index: Some(Index {
                    address: 0x1100,
                    allocation: Some(0x3100),
                    width: source::IndexWidth::U16,
                    revision: 1,
                    revision_before: 1,
                    revision_after: 1,
                    responses: response(0x5100, 0x4100),
                }),
                shader_kind: 1,
                shader: None,
                base_vertex: 7,
                frame: 9,
                device: upload::Device {
                    address: 0x23000000,
                    hardware_vertices: 1,
                    special_vertices: 0,
                    skip_eviction: 0,
                },
            },
        )
    }
    #[test]
    fn misses_create_typed_wrappers_copy_bytes_and_keep_target_tails() {
        let (mut r, q) = fixture();
        let p = r.resolve_sources(&memory(2, 3), &q).unwrap();
        assert!(p.streams[0].created);
        assert!(p.index.unwrap().created);
        assert_eq!(&r.targets[0].bytes[..64], &(0..64).collect::<Vec<_>>());
        assert_eq!(&r.targets[0].bytes[64..], &[0xaa; 64]);
        assert_eq!(&r.targets[1].bytes[..6], &[128, 129, 130, 131, 132, 133]);
        assert_eq!(
            p.request.streams[0].declaration,
            [0x01010001, 0xffff0402, u32::MAX, u32::MAX, 3]
        );
        assert_eq!(
            r.cache
                .upload_resource(0x3000, Kind::Vertex)
                .unwrap()
                .source,
            0x1000
        );
        assert_eq!(r.cache.nodes[0].bytes[0x14], 9);
    }
    #[test]
    fn revision_hit_skips_skin_metadata_payload_target_and_allocation() {
        let (mut r, mut q) = fixture();
        q.index = None;
        r.resolve_sources(&memory(2, 3), &q).unwrap();
        q.streams[0].allocation = None;
        q.streams[0].responses = response(0, 0);
        r.targets.clear();
        let m = Memory::new(vec![Region {
            address: 0x1004,
            bytes: [11u32.to_le_bytes(), 22u32.to_le_bytes()].concat(),
        }])
        .unwrap();
        let p = r.resolve_sources(&m, &q).unwrap();
        assert!(!p.streams[0].created);
        assert!(p.streams[0].transfer.is_none());
    }
    #[test]
    fn initial_zero_revision_keeps_new_wrappers_without_gpu_handles() {
        let (mut r, mut q) = fixture();
        q.streams[0].revision = 0;
        q.index.as_mut().unwrap().revision = 0;
        q.streams[0].responses.creates.clear();
        q.index.as_mut().unwrap().responses.creates.clear();
        r.targets.clear();
        let p = r.resolve_sources(&memory(2, 3), &q).unwrap();
        assert!(p.streams[0].created);
        assert!(p.streams[0].transfer.is_none());
        assert!(p.index.unwrap().transfer.is_none());
        assert_eq!(p.request.streams[0].handle, 0);
        assert_eq!(p.request.index.handle, 0);
        assert_ne!(p.request.index.wrapper, 0);
    }
    #[test]
    fn aliased_slots_observe_first_slots_new_revision() {
        let (mut r, mut q) = fixture();
        q.index = None;
        let mut second = q.streams[0].clone();
        second.allocation = Some(0x3200);
        second.responses = response(0, 0);
        q.streams.push(second);
        let p = r.resolve_sources(&memory(2, 3), &q).unwrap();
        assert!(p.streams[0].transfer.is_some());
        assert!(p.streams[1].transfer.is_none());
        assert_eq!(p.request.streams[0].wrapper, p.request.streams[1].wrapper);
        assert_eq!(r.cache.head, 0x3000);
    }
    #[test]
    fn outer_index_upload_gate_can_call_inner_revision_skip_without_target() {
        let (mut r, mut q) = fixture();
        q.streams.clear();
        q.index.as_mut().unwrap().revision_before = 0;
        q.index.as_mut().unwrap().responses.lock_pointer = 0;
        r.targets.clear();
        let p = r.resolve_sources(&memory(2, 3), &q).unwrap();
        let t = p.index.unwrap().transfer.unwrap();
        assert!(!t.plan.uploaded);
        assert_eq!(t.copied, 0);
        assert_eq!(t.plan.commands.len(), 1);
        assert_eq!(
            r.cache
                .upload_resource(0x3100, Kind::Index)
                .unwrap()
                .revision,
            1
        );
    }
    #[test]
    fn zero_index_size_does_not_read_key_or_require_allocation() {
        let (mut r, mut q) = fixture();
        q.streams.clear();
        q.index.as_mut().unwrap().allocation = None;
        let m = Memory::new(vec![Region {
            address: 0x1110,
            bytes: vec![0; 8],
        }])
        .unwrap();
        let p = r.resolve_sources(&m, &q).unwrap();
        assert!(p.index.is_none());
        assert_eq!(r.cache.head, 0);
    }
    #[test]
    fn null_index_does_not_read_source() {
        let (mut r, mut q) = fixture();
        q.streams.clear();
        q.index.as_mut().unwrap().address = 0;
        let p = r
            .resolve_sources(&Memory::new(vec![]).unwrap(), &q)
            .unwrap();
        assert!(p.index.is_none());
        assert_eq!(p.request.index.source, 0);
    }
    #[test]
    fn failed_index_after_vertex_rolls_back_both_copies_and_cache_insertion() {
        let (mut r, mut q) = fixture();
        q.index.as_mut().unwrap().responses.unlock_hresult = 0x80004005;
        let before = r.clone();
        assert!(r.resolve_sources(&memory(2, 3), &q).is_err());
        assert_eq!(r, before);
    }
    #[test]
    fn missing_payload_after_create_is_atomic() {
        let (mut r, mut q) = fixture();
        q.index = None;
        let before = r.clone();
        assert!(r.resolve_sources(&memory(5, 3), &q).is_err());
        assert_eq!(r, before);
    }
    #[test]
    fn cache_miss_requires_allocation_and_type_collision_is_rejected() {
        let (mut r, mut q) = fixture();
        q.streams[0].allocation = None;
        let before = r.clone();
        assert!(r.resolve_sources(&memory(2, 3), &q).is_err());
        assert_eq!(r, before);
        r.cache.insert(0x3000, [11, 22], Kind::Index).unwrap();
        let before = r.clone();
        assert!(r.resolve_sources(&memory(2, 3), &q).is_err());
        assert_eq!(r, before);
    }
    #[test]
    fn duplicate_targets_and_excess_streams_are_safe_errors() {
        let (mut r, mut q) = fixture();
        r.targets.push(r.targets[0].clone());
        let before = r.clone();
        assert!(r.resolve_sources(&memory(2, 3), &q).is_err());
        assert_eq!(r, before);
        r.targets.pop();
        q.streams = vec![q.streams[0].clone(); 17];
        assert!(r.resolve_sources(&memory(2, 3), &q).is_err());
    }
    fn state() -> (buffers::State, Deferred) {
        let s = buffers::State {
            declaration: 0,
            hardware_vertex: 0,
            declarations: [[0; 5]; 16],
            stream_count: 0,
            wrappers: [0; 16],
            strides: [0; 16],
            previous_stream_count: 0,
            index_wrapper: 0,
            base_vertex: 0,
        };
        let v = crate::d3d_state::Values {
            render: [0; 32],
            stages: [[0; 21]; 8],
            textures: [0; 8],
        };
        let b = crate::d3d_bindings::Values {
            vertex_shader: 0,
            pixel_shader: 0,
            streams: [[0; 2]; 16],
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
    fn prepared_bindings_use_cache_handles_and_preupload_revisions() {
        let (mut r, q) = fixture();
        let (mut s, mut d) = state();
        let p = prepare(&mut r, &mut s, &mut d, &memory(2, 3), &q).unwrap();
        assert_eq!(p.bindings.streams.upload_slots, vec![0]);
        assert_eq!(p.bindings.streams.upload_size, 64);
        assert!(p.bindings.index.upload);
        assert_eq!(d.bindings.desired.streams[0], [0x5000, 32]);
        assert_eq!(d.bindings.desired.indices, [0x5100, 7]);
        let p = prepare(&mut r, &mut s, &mut d, &memory(2, 3), &q).unwrap();
        assert!(p.bindings.streams.upload_slots.is_empty());
        assert!(!p.bindings.index.upload);
    }
    #[test]
    fn missing_fixed_shader_rolls_back_completed_transfers_and_bindings() {
        let (mut r, mut q) = fixture();
        q.shader_kind = 0;
        let (mut s, mut d) = state();
        let before = (r.clone(), s.clone(), d.clone());
        assert!(prepare(&mut r, &mut s, &mut d, &memory(2, 3), &q).is_err());
        assert_eq!((r, s, d), before);
    }
}
