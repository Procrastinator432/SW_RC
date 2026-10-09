//! Static buffer lifecycle planning, D3DDrv 1002a380 and 1002c270.
//! Device results and source callbacks are explicit inputs, not real GPU execution.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Resource {
    pub address: u32,
    pub handle: u32,
    pub capacity: u32,
    pub revision: u32,
    /// Vertex wrapper+3c. Index upload preserves this field.
    pub source: u32,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Device {
    pub address: u32,
    /// Raw Device+40dc: zero adds usage bit 10.
    pub hardware_vertices: u32,
    /// Raw Device+4118: gates requested usage bit 100 for vertex buffers.
    pub special_vertices: u32,
    /// Raw Device+4120: nonzero skips eviction and advances retry index by two.
    pub skip_eviction: u32,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct VertexSource {
    pub address: u32,
    pub size: u32,
    pub dynamic: u32,
    pub special: u32,
    pub revision_after: u32,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct IndexSource {
    pub address: u32,
    pub size: u32,
    pub width: u32,
    /// The original calls the revision getter before and after the upload gate.
    pub revision_before: u32,
    pub revision_after: u32,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Created {
    pub hresult: u32,
    pub handle: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Responses {
    pub creates: Vec<Created>,
    pub evictions: Vec<u32>,
    pub lock_hresult: u32,
    pub lock_pointer: u32,
    pub unlock_hresult: u32,
    /// Address used for the native lock out-parameter, explicit in diagnostic fixtures.
    pub lock_slot: u32,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Command {
    pub receiver: u32,
    pub vtable_offset: u32,
    pub arguments: Vec<u32>,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Plan {
    pub reused: bool,
    /// Index creation policy is not evaluated on the native reuse path.
    pub usage: Option<u32>,
    pub lock_flags: u32,
    pub size: u32,
    pub format: Option<u32>,
    pub uploaded: bool,
    pub commands: Vec<Command>,
}
fn success(hr: u32) -> bool {
    (hr as i32) >= 0
}
fn command(plan: &mut Plan, receiver: u32, offset: u32, arguments: Vec<u32>) {
    plan.commands.push(Command {
        receiver,
        vtable_offset: offset,
        arguments,
    });
}
pub fn vertex_flags(source: VertexSource, device: Device) -> (u32, u32) {
    let mut usage = if source.dynamic != 0 { 0x208 } else { 8 };
    if source.special != 0 && device.special_vertices != 0 {
        usage |= 0x100;
    }
    if device.hardware_vertices == 0 {
        usage |= 0x10;
    }
    (usage, if source.dynamic != 0 { 0x2000 } else { 0 })
}
pub fn index_size(size: u32) -> u32 {
    if (size as i32) > 2 {
        size
    } else {
        2
    }
}
fn validate(resource: &Resource, source: u32, device: Device) -> Result<(), String> {
    if resource.address == 0 || source == 0 || device.address == 0 {
        return Err("Missing resolved upload resource/source/device".into());
    }
    Ok(())
}
fn lock_fill(
    resource: &Resource,
    source: u32,
    callback: u32,
    responses: &Responses,
    plan: &mut Plan,
) -> Result<(), String> {
    if !success(responses.lock_hresult) || responses.lock_slot == 0 || responses.lock_pointer == 0 {
        return Err("Static buffer lock failed or missing output pointer".into());
    }
    command(
        plan,
        resource.handle,
        0x2c,
        vec![0, plan.size, responses.lock_slot, plan.lock_flags],
    );
    command(plan, source, callback, vec![responses.lock_pointer]);
    command(plan, resource.handle, 0x30, vec![]);
    if !success(responses.unlock_hresult) {
        return Err("Static buffer unlock failed".into());
    }
    plan.uploaded = true;
    Ok(())
}
/// Successful native lifecycle. Failures stop safely and preserve resource state;
/// native diagnostic logging/continuation after failure is outside this API.
pub fn vertex(
    resource: &mut Resource,
    source: VertexSource,
    device: Device,
    responses: &Responses,
) -> Result<Plan, String> {
    validate(resource, source.address, device)?;
    let mut next = resource.clone();
    next.source = source.address;
    let (usage, lock_flags) = vertex_flags(source, device);
    let mut plan = Plan {
        reused: next.handle != 0 && next.capacity == source.size,
        usage: Some(usage),
        lock_flags,
        size: source.size,
        format: None,
        uploaded: false,
        commands: vec![],
    };
    if !plan.reused {
        if next.handle != 0 {
            command(&mut plan, next.handle, 8, vec![]);
        }
        let mut attempt = 0;
        let mut response = 0;
        let mut eviction = 0;
        let mut created = false;
        while attempt < 3 {
            let pool = if attempt == 2 { 2 } else { 0 };
            command(
                &mut plan,
                device.address,
                0x5c,
                vec![source.size, usage, 0, pool, next.address.wrapping_add(0x30)],
            );
            let result = responses
                .creates
                .get(response)
                .ok_or("Missing vertex creation response")?;
            response += 1;
            next.handle = result.handle;
            if success(result.hresult) {
                if next.handle == 0 {
                    return Err("Successful vertex creation returned null handle".into());
                }
                created = true;
                break;
            }
            if device.skip_eviction != 0 {
                attempt += 2;
            } else {
                command(&mut plan, device.address, 0x14, vec![0]);
                let hr = *responses
                    .evictions
                    .get(eviction)
                    .ok_or("Missing eviction response")?;
                eviction += 1;
                if !success(hr) {
                    return Err("Vertex eviction failed".into());
                }
                attempt += 1;
            }
        }
        if !created {
            return Err("Vertex creation attempts exhausted".into());
        }
    }
    lock_fill(&next, source.address, 0x24, responses, &mut plan)?;
    next.capacity = source.size;
    next.revision = source.revision_after;
    *resource = next;
    Ok(plan)
}
pub fn index(
    resource: &mut Resource,
    source: IndexSource,
    device: Device,
    responses: &Responses,
) -> Result<Plan, String> {
    validate(resource, source.address, device)?;
    let mut next = resource.clone();
    let size = index_size(source.size);
    let usage = if device.hardware_vertices == 0 {
        0x18
    } else {
        8
    };
    let format = if source.width == 4 { 0x66 } else { 0x65 };
    let mut plan = Plan {
        reused: next.handle != 0 && next.capacity == size,
        usage: None,
        lock_flags: 0,
        size,
        format: None,
        uploaded: false,
        commands: vec![],
    };
    if !plan.reused {
        plan.usage = Some(usage);
        plan.format = Some(format);
        if next.handle != 0 {
            command(&mut plan, next.handle, 8, vec![]);
        }
        command(
            &mut plan,
            device.address,
            0x60,
            vec![size, usage, format, 1, next.address.wrapping_add(0x30)],
        );
        let result = responses
            .creates
            .first()
            .ok_or("Missing index creation response")?;
        if !success(result.hresult) || result.handle == 0 {
            return Err("Index creation failed or null handle".into());
        }
        next.handle = result.handle;
    }
    if next.revision != source.revision_before {
        lock_fill(&next, source.address, 0x14, responses, &mut plan)?;
    }
    next.capacity = size;
    next.revision = source.revision_after;
    *resource = next;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Resource, VertexSource, IndexSource, Device, Responses) {
        (
            Resource {
                address: 0x1000,
                handle: 0x2000,
                capacity: 16,
                revision: 7,
                source: 99,
            },
            VertexSource {
                address: 0x3000,
                size: 16,
                dynamic: 0,
                special: 0,
                revision_after: 8,
            },
            IndexSource {
                address: 0x3000,
                size: 16,
                width: 2,
                revision_before: 8,
                revision_after: 9,
            },
            Device {
                address: 0x4000,
                hardware_vertices: 1,
                special_vertices: 1,
                skip_eviction: 0,
            },
            Responses {
                creates: vec![Created {
                    hresult: 0,
                    handle: 0x5000,
                }],
                evictions: vec![0; 3],
                lock_hresult: 0,
                lock_pointer: 0x6000,
                unlock_hresult: 0,
                lock_slot: 0x7000,
            },
        )
    }
    #[test]
    fn flags_cover_raw_nonzero_gates() {
        let (_, mut s, _, mut d, _) = fixture();
        s.dynamic = u32::MAX;
        s.special = 128;
        d.hardware_vertices = 0;
        assert_eq!(vertex_flags(s, d), (0x318, 0x2000));
        d.special_vertices = 0;
        assert_eq!(vertex_flags(s, d), (0x218, 0x2000));
    }
    #[test]
    fn vertex_reuse_always_uploads_even_with_matching_revision() {
        let (mut r, mut s, _, d, o) = fixture();
        s.revision_after = r.revision;
        let p = vertex(&mut r, s, d, &o).unwrap();
        assert!(p.reused && p.uploaded);
        assert_eq!(
            p.commands
                .iter()
                .map(|c| c.vtable_offset)
                .collect::<Vec<_>>(),
            vec![0x2c, 0x24, 0x30]
        );
        assert_eq!(r.source, s.address);
    }
    #[test]
    fn vertex_resize_releases_then_creates_then_locks() {
        let (mut r, mut s, _, d, o) = fixture();
        s.size = 0;
        let p = vertex(&mut r, s, d, &o).unwrap();
        assert_eq!(p.commands[0].receiver, 0x2000);
        assert_eq!(p.commands[1].arguments, vec![0, 8, 0, 0, 0x1030]);
        assert_eq!(r.handle, 0x5000);
        assert_eq!(r.capacity, 0);
    }
    #[test]
    fn retries_use_default_default_system_and_evict_between() {
        let (mut r, mut s, _, d, mut o) = fixture();
        s.size = 17;
        o.creates = vec![
            Created {
                hresult: 0x80004005,
                handle: 0,
            },
            Created {
                hresult: 0xffffffff,
                handle: 0,
            },
            Created {
                hresult: 1,
                handle: 0x5000,
            },
        ];
        let p = vertex(&mut r, s, d, &o).unwrap();
        assert_eq!(
            p.commands
                .iter()
                .filter(|c| c.vtable_offset == 0x5c)
                .map(|c| c.arguments[3])
                .collect::<Vec<_>>(),
            vec![0, 0, 2]
        );
        assert_eq!(
            p.commands
                .iter()
                .filter(|c| c.vtable_offset == 0x14 && c.receiver == d.address)
                .count(),
            2
        );
    }
    #[test]
    fn skip_eviction_retries_default_then_system() {
        let (mut r, mut s, _, mut d, mut o) = fixture();
        s.size = 17;
        d.skip_eviction = 255;
        o.creates = vec![
            Created {
                hresult: 0x80004005,
                handle: 0,
            },
            Created {
                hresult: 0,
                handle: 0x5000,
            },
        ];
        let p = vertex(&mut r, s, d, &o).unwrap();
        assert_eq!(
            p.commands
                .iter()
                .filter(|c| c.vtable_offset == 0x5c)
                .map(|c| c.arguments[3])
                .collect::<Vec<_>>(),
            vec![0, 2]
        );
    }
    #[test]
    fn index_minimum_is_signed() {
        for n in [0, 1, 2, 0x80000000, u32::MAX] {
            assert_eq!(index_size(n), 2);
        }
        assert_eq!(index_size(0x7fffffff), 0x7fffffff);
    }
    #[test]
    fn index_reuse_ignores_width_and_usage_changes() {
        let (mut r, _, mut s, mut d, o) = fixture();
        s.width = 4;
        d.hardware_vertices = 0;
        let p = index(&mut r, s, d, &o).unwrap();
        assert!(p.reused);
        assert_eq!(p.format, None);
        assert_eq!(p.usage, None);
        assert_eq!(r.source, 99);
    }
    #[test]
    fn index_matching_revision_skips_upload_even_after_new_creation() {
        let (mut r, _, mut s, d, mut o) = fixture();
        r.handle = 0;
        s.revision_before = r.revision;
        s.revision_after = 123;
        o.lock_hresult = 0x80004005;
        let p = index(&mut r, s, d, &o).unwrap();
        assert!(!p.uploaded);
        assert_eq!(p.commands.len(), 1);
        assert_eq!(r.revision, 123);
    }
    #[test]
    fn index_callback_and_lock_length_use_clamped_size() {
        let (mut r, _, mut s, d, o) = fixture();
        s.size = u32::MAX;
        s.width = 3;
        let p = index(&mut r, s, d, &o).unwrap();
        assert_eq!(p.commands[1].arguments, vec![2, 8, 0x65, 1, 0x1030]);
        assert_eq!(p.commands[2].arguments, vec![0, 2, 0x7000, 0]);
        assert_eq!(p.commands[3].vtable_offset, 0x14);
    }
    #[test]
    fn failed_operations_preserve_resource() {
        let (r, mut s, i, d, mut o) = fixture();
        s.size = 17;
        o.creates.clear();
        let mut n = r.clone();
        assert!(vertex(&mut n, s, d, &o).is_err());
        assert_eq!(n, r);
        o.creates = vec![Created {
            hresult: 0,
            handle: 0x5000,
        }];
        o.unlock_hresult = 0x80004005;
        assert!(index(&mut n, i, d, &o).is_err());
        assert_eq!(n, r);
    }
}
