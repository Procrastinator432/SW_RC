use rc_package::{
    d3d_buffers::Submission,
    d3d_draw::{Context, Draw},
    d3d_static::{self, Request, Resources},
    shader_snapshot::{Memory, Region},
};
use serde::Deserialize;
use std::{env, fs};
#[derive(Deserialize)]
struct Step {
    regions: Vec<Region>,
    request: Request,
    context: Context,
    draw: Draw,
}
#[derive(Deserialize)]
struct Case {
    id: usize,
    resources: Resources,
    submission: Submission,
    steps: Vec<Step>,
}
#[derive(Deserialize)]
struct Input {
    source_sha256: String,
    cases: Vec<Case>,
    errors: Vec<Case>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("Expected INPUT OUTPUT".into());
    }
    let input: Input = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut probes = vec![];
    let mut errors = vec![];
    for (safe, cases) in [(false, input.cases), (true, input.errors)] {
        for c in cases {
            let mut resources = c.resources;
            let mut submission = c.submission;
            let mut steps = vec![];
            for s in c.steps {
                let result = Memory::new(s.regions).and_then(|m| {
                    d3d_static::submit(
                        &mut resources,
                        &mut submission,
                        &m,
                        &s.request,
                        s.context,
                        s.draw,
                    )
                });
                if !safe {
                    if let Err(error) = &result {
                        return Err(error.clone().into());
                    }
                }
                steps.push(serde_json::json!({"result":result,"resources":resources,"submission":submission}));
            }
            let value = serde_json::json!({"id":c.id,"steps":steps,"resources":resources,"submission":submission});
            if safe {
                errors.push(value);
            } else {
                probes.push(value);
            }
        }
    }
    fs::write(
        &args[2],
        serde_json::to_vec(
            &serde_json::json!({"source_sha256":input.source_sha256,"probes":probes,"errors":errors,"scope":"Captured static source keys and data through typed resource lookup/initialization, sequential revision gates, CPU upload mirrors, native wrapper field persistence, stream/index handoff and bounded multipass draw planning. Synthetic snapshots, explicit allocator images/COM/revision/owner/shader answers; no real allocator, GPU, skinning implementation, live game or Android execution."}),
        )?,
    )?;
    Ok(())
}
