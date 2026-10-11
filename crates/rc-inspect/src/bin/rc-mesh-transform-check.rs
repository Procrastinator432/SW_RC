use rc_package::{
    d3d_dynamic_draw::Runtime,
    mesh_transform::{Input as Transform, Request},
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
    math: Vec<Transform>,
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
    let math: Vec<_> = input.math.iter().map(Transform::build).collect();
    let (mut probes, mut errors) = (vec![], vec![]);
    for (safe, cases) in [(false, input.cases), (true, input.errors)] {
        for c in cases {
            let mut rt = c.runtime;
            let mut steps = vec![];
            for mut q in c.requests {
                let result = rt.submit_transformed_vertmesh(&mut q);
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
            &serde_json::json!({"source_sha256":input.source_sha256,"passes_sha256":input.passes_sha256,"math":math,"probes":probes,"errors":errors,"scope":"Core rotator matrix/IsZero and Engine MeshToWorld joined to the existing render CPU path; copied matrix recorded at renderer +48 slot zero. Stable actor snapshot, finite default IEEE contract. Supplied camera/tangent/asset/allocator/material/COM answers; renderer +48 body and GPU/Android excluded."}),
        )?,
    )?;
    Ok(())
}
