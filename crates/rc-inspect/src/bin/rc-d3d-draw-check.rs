use rc_package::{
    d3d_complete::Complete,
    d3d_draw::{self, Context, Counters, Draw, PreparedPass},
    d3d_state::Cache,
    skeletal_draw::DrawSection,
};
use serde::Deserialize;
use std::{env, fs};
#[derive(Deserialize)]
struct DrawCase {
    id: usize,
    draw: Draw,
    counters: Counters,
}
#[derive(Deserialize)]
struct Fog {
    id: usize,
    cache: Cache,
    pass: PreparedPass,
    enabled: u8,
    restore: u32,
}
#[derive(Deserialize)]
struct Case {
    id: usize,
    complete: Complete,
    last_pass: u32,
    counters: Counters,
    passes: Vec<PreparedPass>,
    context: Context,
    draw: Draw,
    section: Option<usize>,
}
#[derive(Deserialize)]
struct Input {
    source_sha256: String,
    mesh_sha256: String,
    mesh_sections: Vec<DrawSection>,
    draws: Vec<DrawCase>,
    fog: Vec<Fog>,
    cases: Vec<Case>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("Expected INPUT OUTPUT".into());
    }
    let input: Input = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut draws = Vec::new();
    let mut fog = Vec::new();
    let mut probes = Vec::new();
    let section_draws: Vec<_> = input
        .mesh_sections
        .iter()
        .map(|s| Draw::from_section(s, true))
        .collect::<Result<_, _>>()?;
    for c in input.draws {
        let mut counters = c.counters;
        let call = counters.submit(c.draw);
        draws.push(serde_json::json!({"id":c.id,"call":call,"counters":counters}));
    }
    for c in input.fog {
        let mut cache = c.cache;
        let overridden = d3d_draw::fog_begin(&mut cache, c.enabled, &c.pass);
        let applied = cache.clone();
        d3d_draw::fog_end(&mut cache, overridden, c.restore);
        fog.push(
            serde_json::json!({"id":c.id,"overridden":overridden,"applied":applied,"final":cache}),
        );
    }
    for c in input.cases {
        let mut complete = c.complete;
        let mut last = c.last_pass;
        let mut counters = c.counters;
        let mut passes = c.passes;
        let result = d3d_draw::render(
            &mut complete,
            &mut last,
            &mut counters,
            &mut passes,
            c.context,
            c.draw,
        )?;
        probes.push(serde_json::json!({"id":c.id,"section":c.section,"result":result,"complete":complete,"last_pass":last,"counters":counters,"passes":passes}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec(
            &serde_json::json!({"source_sha256":input.source_sha256,"mesh_sha256":input.mesh_sha256,"section_draws":section_draws,"draws":draws,"fog":fog,"probes":probes,"scope":"Original primitive dispatch/index arguments, wrapping draw statistics, per-pass raw fog override/restoration and bounded multipass integration of resolved pass/state/draw plans. Aliased pass slots share mutation. Three validated CloneCommando LOD0 section requests included. No GPU invocation, buffer/material acquisition, live scene or Android."}),
        )?,
    )?;
    Ok(())
}
