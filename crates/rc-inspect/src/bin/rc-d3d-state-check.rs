//! Native pass/stage translation and supported deferred D3D8 call plans.
use rc_package::{d3d_state::Cache, hardware_stages::Stage};
use serde::Deserialize;
use std::{env, fs};
#[derive(Deserialize)]
struct Case {
    id: usize,
    material_id: Option<usize>,
    cache: Cache,
    pass: [u8; 28],
    stage: Stage,
    index: usize,
    resource: Option<[u32; 2]>,
    hardware: bool,
    lod_bias: u32,
    cull_mode: u32,
    capacity: usize,
    stencil_gate: bool,
}
#[derive(Deserialize)]
struct Input {
    cases: Vec<Case>,
    source_material_report_sha256: String,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("Expected INPUT OUTPUT".into());
    }
    let input: Input = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut probes = Vec::new();
    for c in input.cases {
        let mut cache = c.cache;
        cache.render_prefix(&c.pass, c.cull_mode);
        let render = cache.clone();
        let mut stage = c.stage;
        cache.stage(c.index, &mut stage, c.resource, c.hardware, c.lod_bias)?;
        let translated = cache.clone();
        let calls = cache.flush(c.capacity, c.stencil_gate)?;
        let flushed = cache.clone();
        cache.dirty = 0x13;
        let repeated = cache.flush(c.capacity, c.stencil_gate)?;
        cache.dirty = 0x13;
        let expanded = cache.flush(8, true)?;
        probes.push(serde_json::json!({"id":c.id,"material_id":c.material_id,"render":render,"translated":translated,"stage":stage,"calls":calls,"flushed":flushed,"repeated":repeated,"expanded":expanded,"final":cache}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec(
            &serde_json::json!({"probes":probes,"source_material_report_sha256":input.source_material_report_sha256,"scope":"Original common pass prefix and non-matrix per-stage translation -> native desired/applied render/stage/texture cache -> ordered numeric D3D8 call plans. Explicit initial cache/capacity/stencil_gate/LOD/resource fixtures. Does not invoke GPU, translate full pass/shader selection/unused-stage tail, transform matrices, vertex/index/stream state, live scene or Android."}),
        )?,
    )?;
    Ok(())
}
