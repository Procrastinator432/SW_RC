use rc_package::{
    d3d_pass::{Device, Pass},
    d3d_state::Cache,
    d3d_transforms::{self, Transforms},
};
use serde::Deserialize;
use std::{env, fs};
#[derive(Deserialize)]
struct Case {
    id: usize,
    material_id: Option<usize>,
    cache: Cache,
    transforms: Transforms,
    pass: Pass,
    resources: [Option<[u32; 2]>; 8],
    device: Device,
    last_pass: u32,
    stencil_gate: bool,
}
#[derive(Deserialize)]
struct Flush {
    id: usize,
    cache: Cache,
    transforms: Transforms,
    capacity: usize,
    stencil_gate: bool,
}
#[derive(Deserialize)]
struct Input {
    source_sha256: String,
    cases: Vec<Case>,
    flush_cases: Vec<Flush>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("Expected INPUT OUTPUT".into());
    }
    let input: Input = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut probes = Vec::new();
    let mut flushes = Vec::new();
    for c in input.cases {
        let mut cache = c.cache;
        let mut matrices = c.transforms;
        let mut pass = c.pass;
        let mut last = c.last_pass;
        let changed = d3d_transforms::apply_hardware_pass(
            &mut cache,
            &mut matrices,
            &mut last,
            &mut pass,
            c.resources,
            c.device,
        )?;
        let translated = cache.clone();
        let translated_matrices = matrices.clone();
        let plan = matrices.flush(&mut cache, c.device.capacity, c.stencil_gate)?;
        let flushed = cache.clone();
        let flushed_matrices = matrices.clone();
        pass.header[9] = 255;
        let skipped = !d3d_transforms::apply_hardware_pass(
            &mut cache,
            &mut matrices,
            &mut last,
            &mut pass,
            [None; 8],
            Device {
                capacity: 99,
                ..c.device
            },
        )?;
        probes.push(serde_json::json!({"id":c.id,"material_id":c.material_id,"changed":changed,"translated":translated,"translated_matrices":translated_matrices,"plan":plan,"flushed":flushed,"flushed_matrices":flushed_matrices,"last_pass":last,"pass":pass,"skipped":skipped,"final":cache,"final_matrices":matrices}));
    }
    for c in input.flush_cases {
        let mut cache = c.cache;
        let mut matrices = c.transforms;
        let plan = matrices.flush(&mut cache, c.capacity, c.stencil_gate)?;
        let flushed = cache.clone();
        let flushed_matrices = matrices.clone();
        let repeated = matrices.flush(&mut cache, c.capacity, c.stencil_gate)?;
        flushes.push(serde_json::json!({"id":c.id,"plan":plan,"flushed":flushed,"flushed_matrices":flushed_matrices,"repeated":repeated,"final":cache,"final_matrices":matrices}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec(
            &serde_json::json!({"source_sha256":input.source_sha256,"probes":probes,"flushes":flushes,"scope":"Original raw texture-stage matrix branch, transform-mask output groups and bounded hardware-pass integration. Eleven raw matrix slots with external initial fixtures; render/stage -> transform -> texture order. No actual GPU calls, matrix arithmetic, resource acquisition, fixed pixelshader selection, live game or Android."}),
        )?,
    )?;
    Ok(())
}
