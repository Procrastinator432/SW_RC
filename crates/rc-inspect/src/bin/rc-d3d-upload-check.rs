use rc_package::d3d_upload::{self, Device, IndexSource, Resource, Responses, VertexSource};
use serde::Deserialize;
use std::{env, fs};
#[derive(Deserialize)]
struct Case<S> {
    id: usize,
    resource: Resource,
    source: S,
    device: Device,
    responses: Responses,
}
#[derive(Deserialize)]
struct Action {
    source: serde_json::Value,
    device: Device,
    responses: Responses,
}
#[derive(Deserialize)]
struct Sequence {
    id: usize,
    kind: String,
    resource: Resource,
    actions: Vec<Action>,
}
#[derive(Deserialize)]
struct Input {
    source_sha256: String,
    vertices: Vec<Case<VertexSource>>,
    indices: Vec<Case<IndexSource>>,
    sequences: Vec<Sequence>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("Expected INPUT OUTPUT".into());
    }
    let input: Input = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut vertices = vec![];
    let mut indices = vec![];
    let mut sequences = vec![];
    for c in input.vertices {
        let mut resource = c.resource;
        let result = d3d_upload::vertex(&mut resource, c.source, c.device, &c.responses);
        vertices.push(serde_json::json!({"id":c.id,"result":result,"resource":resource}));
    }
    for c in input.indices {
        let mut resource = c.resource;
        let result = d3d_upload::index(&mut resource, c.source, c.device, &c.responses);
        indices.push(serde_json::json!({"id":c.id,"result":result,"resource":resource}));
    }
    for c in input.sequences {
        let mut resource = c.resource;
        let mut steps = vec![];
        for a in c.actions {
            let result = match c.kind.as_str() {
                "vertex" => d3d_upload::vertex(
                    &mut resource,
                    serde_json::from_value(a.source)?,
                    a.device,
                    &a.responses,
                ),
                "index" => d3d_upload::index(
                    &mut resource,
                    serde_json::from_value(a.source)?,
                    a.device,
                    &a.responses,
                ),
                _ => return Err("Unknown upload kind".into()),
            }?;
            steps.push(serde_json::json!({"plan":result,"resource":resource}));
        }
        sequences
            .push(serde_json::json!({"id":c.id,"kind":c.kind,"steps":steps,"resource":resource}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec(
            &serde_json::json!({"source_sha256":input.source_sha256,"vertices":vertices,"indices":indices,"sequences":sequences,"scope":"Static buffer lifecycle call planning: vertex flags/reuse, bounded creation retries/eviction, lock/source callback/unlock/finalization, signed index size/format/reuse and revision-gated index upload. Explicit device/source results, no GPU invocation or source payload execution. Safe atomic rejection excludes native failure logging/continuation. Android deferred."}),
        )?,
    )?;
    Ok(())
}
