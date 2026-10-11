use rc_package::{
    d3d_dynamic_draw::Runtime,
    vertmesh_animation::{Clock, Input as FrameInput, Request},
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
    frames: Vec<FrameInput>,
    frame_errors: Vec<FrameInput>,
    clocks: Vec<Clock>,
    clock_errors: Vec<Clock>,
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
                let result = rt.submit_animated_frame(&mut q);
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
    let clocks: Vec<_> = input.clocks.iter().map(Clock::select).collect();
    let clock_errors: Vec<_> = input.clock_errors.iter().map(Clock::select).collect();
    assert!(clocks.iter().all(Result::is_ok));
    assert!(clock_errors.iter().all(Result::is_err));
    fs::write(
        &args[2],
        serde_json::to_vec(
            &serde_json::json!({"source_sha256":input.source_sha256,"passes_sha256":input.passes_sha256,"probes":probes,"errors":errors,"frames":frames,"frame_errors":frame_errors,"clocks":clocks,"clock_errors":clock_errors,"scope":"Original GetFrame time clamp, float32 product, MXCSR floor conversion, cyclic frame offsets and fractional interpolation; flattened packed animation banks feed decode, indices, upload and material passes transactionally. Supplied GetSequence result/cache storage; no lookup/allocation, full camera integration, live assets/GPU or Android run."}),
        )?,
    )?;
    Ok(())
}
