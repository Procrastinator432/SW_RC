use rc_package::{
    d3d_dynamic_draw::Runtime,
    vertmesh_instance::{Cache, Input as FrameInput, Request},
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
struct CacheProbe {
    cache: Cache,
    mesh: u32,
    vertex_count: u32,
}
#[derive(Deserialize)]
struct Input {
    source_sha256: String,
    passes_sha256: String,
    cases: Vec<Case>,
    errors: Vec<Case>,
    frames: Vec<FrameInput>,
    frame_errors: Vec<FrameInput>,
    caches: Vec<CacheProbe>,
    cache_errors: Vec<CacheProbe>,
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
                let result = rt.submit_vertmesh_instance(&mut q);
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
    let frames: Vec<_> = input.frames.iter().map(FrameInput::build).collect();
    assert!(frames.iter().all(Result::is_ok));
    let frame_errors: Vec<_> = input.frame_errors.iter().map(FrameInput::build).collect();
    assert!(frame_errors.iter().all(Result::is_err));
    let caches: Vec<_> = input
        .caches
        .iter()
        .map(|q| q.cache.prepare(q.mesh, q.vertex_count))
        .collect();
    let cache_errors: Vec<_> = input
        .cache_errors
        .iter()
        .map(|q| q.cache.prepare(q.mesh, q.vertex_count))
        .collect();
    assert!(caches.iter().all(Result::is_ok));
    assert!(cache_errors.iter().all(Result::is_err));
    fs::write(
        &args[2],
        serde_json::to_vec(
            &serde_json::json!({"source_sha256":input.source_sha256,"passes_sha256":input.passes_sha256,"probes":probes,"errors":errors,"frames":frames,"frame_errors":frame_errors,"caches":caches,"cache_errors":cache_errors,"scope":"Original GetFrame cache gate, mesh-change reset, logical array resize and original zeroing/metadata; shared GetSequence channel cache/editor refresh; selected record/time drives packed animation, geometry, upload and draw transactionally. Logical array allocator and virtual name bindings supplied. No full camera chain, live animation/GPU or Android run."}),
        )?,
    )?;
    Ok(())
}
