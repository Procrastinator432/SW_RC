use rc_package::{d3d_dynamic_draw::Runtime, vertmesh_lod::Request};
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
                let result = rt.submit_lod(&mut q);
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
    fs::write(
        &args[2],
        serde_json::to_vec(
            &serde_json::json!({"source_sha256":input.source_sha256,"passes_sha256":input.passes_sha256,"probes":probes,"errors":errors,"scope":"Original camera-dependent vertex-mesh LOD arithmetic, nearest-even conversion, signed clamp, section/actor gates and vertex callback argument routing; connected to generated index geometry and complete mesh material draws. Matrix setup, projected W, stored tangent/GetAbsMax answers, vertex animation, material/allocator/COM answers remain explicit. Default MXCSR rounding; nonfinite inputs and zero/nonfinite denominator/candidate safely rejected. No actual GPU/live game/Android execution."}),
        )?,
    )?;
    Ok(())
}
