use rc_package::d3d_bindings::{Bindings, Deferred};
use serde::Deserialize;
use std::{env, fs};
#[derive(Deserialize)]
struct Tail {
    id: usize,
    dirty: u32,
    stream_capacity: i32,
    bindings: Bindings,
}
#[derive(Deserialize)]
struct Case {
    id: usize,
    material_id: Option<usize>,
    deferred: Deferred,
    texture_capacity: usize,
    stream_capacity: i32,
    stencil_gate: bool,
}
#[derive(Deserialize)]
struct Input {
    source_sha256: String,
    cases: Vec<Case>,
    tails: Vec<Tail>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("Expected INPUT OUTPUT".into());
    }
    let input: Input = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut probes = Vec::new();
    let mut tails = Vec::new();
    for c in input.tails {
        let mut b = c.bindings;
        let calls = b.tail(c.dirty, c.stream_capacity);
        let applied = b.clone();
        let repeated = b.tail(c.dirty, c.stream_capacity);
        let expanded = b.tail(12, 16);
        tails.push(serde_json::json!({"id":c.id,"calls":calls,"applied":applied,"repeated":repeated,"expanded":expanded,"final":b}));
    }
    for c in input.cases {
        let mut d = c.deferred;
        let plan = d.flush(c.texture_capacity, c.stream_capacity, c.stencil_gate)?;
        let flushed = d.clone();
        let repeated = d.flush(c.texture_capacity, c.stream_capacity, c.stencil_gate)?;
        // Re-mark all binding groups and expand stream capacity to expose deferred changes.
        d.states.dirty = 12;
        let expanded = d.flush(8, 16, true)?;
        probes.push(serde_json::json!({"id":c.id,"material_id":c.material_id,"plan":plan,"flushed":flushed,"repeated":repeated,"expanded":expanded,"final":d}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec(
            &serde_json::json!({"source_sha256":input.source_sha256,"tails":tails,"probes":probes,"scope":"Native shader/stream/index applied cache tail and ordered combined dirty groups 1/2/4/8/10/20/40. Inputs include previously validated translated hardware-pass snapshots and explicit raw binding fixtures. Excludes light group 80, shader/resource acquisition, buffer contents, actual GPU calls, live game and Android."}),
        )?,
    )?;
    Ok(())
}
