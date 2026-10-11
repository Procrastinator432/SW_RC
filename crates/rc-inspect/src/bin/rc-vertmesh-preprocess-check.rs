use rc_package::{
    d3d_dynamic_draw::Runtime,
    vertmesh_preprocess::{self, Input as Preparation, Request},
};
use serde::Deserialize;
use std::{env, fs};
#[derive(Deserialize)]
struct Case {
    id: usize,
    runtime: Runtime,
    requests: Vec<Request>,
}
#[derive(Deserialize)]
struct Input {
    source_sha256: String,
    passes_sha256: String,
    cases: Vec<Case>,
    errors: Vec<Case>,
    preparations: Vec<Preparation>,
    abs_probes: Vec<[u32; 3]>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("Expected INPUT OUTPUT".into());
    }
    let input: Input = serde_json::from_slice(&fs::read(&args[1])?)?;
    let (mut probes, mut errors) = (vec![], vec![]);
    for (safe, cases) in [(false, input.cases), (true, input.errors)] {
        for c in cases {
            let mut rt = c.runtime;
            let mut steps = vec![];
            for mut q in c.requests {
                let result = rt.submit_preprocessed_mesh(&mut q);
                if !safe {
                    if let Err(e) = &result {
                        return Err(e.clone().into());
                    }
                }
                steps.push(serde_json::json!({"result":result,"runtime":rt,"request":q}));
            }
            let value = serde_json::json!({"id":c.id,"steps":steps,"runtime":rt});
            if safe {
                errors.push(value);
            } else {
                probes.push(value);
            }
        }
    }
    let preparations: Vec<_> = input
        .preparations
        .iter()
        .map(Preparation::prepare)
        .collect();
    let abs_probes: Vec<_> = input
        .abs_probes
        .into_iter()
        .map(vertmesh_preprocess::abs_max)
        .collect();
    fs::write(
        &args[2],
        serde_json::to_vec(
            &serde_json::json!({"source_sha256":input.source_sha256,"passes_sha256":input.passes_sha256,"probes":probes,"errors":errors,"preparations":preparations,"abs_probes":abs_probes,"scope":"Original RenderPreProcess stream reset/UV copy/revision, consecutive material sections and vertex/index constructors with cache IDs; Core GetAbsMax with ordered signed-zero behavior; connected to original Project/LOD/index/upload/material replay. Allocators and 64-bit multiply helper are explicit boundaries, supplied section allocation bytes retained. Animated vertex positions/GetFrame filling, tangent, matrix construction/material/GPU remain external. Safe bounded inputs, Android deferred."}),
        )?,
    )?;
    Ok(())
}
