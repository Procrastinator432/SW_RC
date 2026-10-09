use rc_package::{
    d3d_source::{self, IndexData, IndexWidth, RawIndex, Skin, VertexData},
    d3d_upload::{Device, Resource, Responses},
    shader_snapshot::{Memory, Region},
};
use serde::Deserialize;
use std::{env, fs};
#[derive(Deserialize)]
struct Index {
    id: usize,
    address: u32,
    regions: Vec<Region>,
    width: IndexWidth,
}
#[derive(Deserialize)]
struct Vertex {
    id: usize,
    address: u32,
    regions: Vec<Region>,
    owner_count: u32,
    delegate: Vec<u8>,
    initial: [u8; 16],
    vertex: i32,
}
#[derive(Deserialize)]
struct Error {
    id: usize,
    address: u32,
    regions: Vec<Region>,
}
#[derive(Deserialize)]
struct Case {
    id: usize,
    kind: String,
    address: u32,
    regions: Vec<Region>,
    width: IndexWidth,
    owner_count: u32,
    delegate: Vec<u8>,
    dynamic: u32,
    special: u32,
    revision_before: u32,
    revision_after: u32,
    resource: Resource,
    device: Device,
    responses: Responses,
    destination: Vec<u8>,
}
#[derive(Deserialize)]
struct Input {
    source_sha256: String,
    indices: Vec<Index>,
    skins: Vec<Vertex>,
    errors: Vec<Error>,
    transfers: Vec<Case>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("Expected INPUT OUTPUT".into());
    }
    let input: Input = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut indices = vec![];
    let mut skins = vec![];
    let mut errors = vec![];
    let mut transfers = vec![];
    for c in input.indices {
        let m = Memory::new(c.regions)?;
        let s = RawIndex::capture(&m, c.address, c.width)?;
        indices.push(serde_json::json!({"id":c.id,"capture":s,"size":s.size(),"width":s.index_size(),"payload":s.payload(&m)}));
    }
    for c in input.skins {
        let m = Memory::new(c.regions)?;
        let s = Skin::capture(&m, c.address)?;
        skins.push(serde_json::json!({"id":c.id,"capture":s,"size":s.size(Some(c.owner_count)),"payload":s.payload(&m,Some(&c.delegate)),"pointer":s.raw_pointer(c.vertex),"components":d3d_source::components(c.initial),"stride":d3d_source::stride()}));
    }
    for c in input.errors {
        let result = Memory::new(c.regions).and_then(|m| Skin::capture(&m, c.address));
        errors.push(serde_json::json!({"id":c.id,"result":result}));
    }
    for c in input.transfers {
        let m = Memory::new(c.regions)?;
        let mut resource = c.resource;
        let mut destination = c.destination;
        let result = match c.kind.as_str() {
            "vertex" => d3d_source::vertex(
                &mut resource,
                &VertexData {
                    skin: Skin::capture(&m, c.address)?,
                    owner_count: Some(c.owner_count),
                    delegate: Some(c.delegate),
                    dynamic: c.dynamic,
                    special: c.special,
                    revision_after: c.revision_after,
                },
                &m,
                c.device,
                &c.responses,
                &mut destination,
            ),
            "index" => d3d_source::index(
                &mut resource,
                IndexData {
                    index: RawIndex::capture(&m, c.address, c.width)?,
                    revision_before: c.revision_before,
                    revision_after: c.revision_after,
                },
                &m,
                c.device,
                &c.responses,
                &mut destination,
            ),
            _ => return Err("Unknown kind".into()),
        }?;
        transfers.push(serde_json::json!({"id":c.id,"result":result,"resource":resource,"destination":destination}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec(
            &serde_json::json!({"source_sha256":input.source_sha256,"indices":indices,"skins":skins,"errors":errors,"transfers":transfers,"scope":"Original raw 16/32-bit index and stored skin-stream bytes, packed counts, component partial writes, size/owner gates, raw pointer arithmetic and captured-memory CPU lock mirrors integrated with static upload planning. Owner callback bytes/count and device answers external; no GPU or live process."}),
        )?,
    )?;
    Ok(())
}
