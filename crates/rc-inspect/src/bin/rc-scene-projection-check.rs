use rc_package::{
    d3d_dynamic_draw::Runtime,
    scene_projection::{self, Request},
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
    math: Vec<MathCase>,
}
#[derive(Deserialize)]
struct MathCase {
    matrix: [[u32; 4]; 4],
    inverse: [[u32; 4]; 4],
    position: [u32; 3],
    plane: [u32; 4],
    project_ok: bool,
    deproject_ok: bool,
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
                let result = rt.submit_projected_mesh(&mut q);
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
    let math: Vec<_> = input
        .math
        .into_iter()
        .map(|m| {
            let projection = scene_projection::project(m.matrix, m.position);
            let deprojection = scene_projection::deproject(m.inverse, m.plane);
            assert_eq!(projection.is_ok(), m.project_ok);
            assert_eq!(deprojection.is_ok(), m.deproject_ok);
            serde_json::json!({"projection":projection,"deprojection":deprojection})
        })
        .collect();
    fs::write(
        &args[2],
        serde_json::to_vec(
            &serde_json::json!({"source_sha256":input.source_sha256,"passes_sha256":input.passes_sha256,"probes":probes,"errors":errors,"math":math,"scope":"Original FSceneNode Project/Deproject SSE bodies and projected actor position feeding transactional camera LOD, geometry, upload and material draws. Supplied forward/inverse matrices; no inverse construction, tangent/GetAbsMax/animation/material/GPU implementation. Finite standard rounding contract; zero/overflow forward W safely rejected. Android deferred."}),
        )?,
    )?;
    Ok(())
}
