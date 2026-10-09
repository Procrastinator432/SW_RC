//! Bounded multi-stage hardware-pass and native pass-tail verification.
use rc_package::{
    d3d_pass::{self, Device, Pass},
    d3d_state::Cache,
};
use serde::Deserialize;
use std::{env, fs};
#[derive(Deserialize)]
struct Case {
    id: usize,
    material_id: Option<usize>,
    cache: Cache,
    pass: Pass,
    resources: [Option<[u32; 2]>; 8],
    device: Device,
    hardware: bool,
    last_pass: u32,
    stencil_gate: bool,
}
#[derive(Deserialize)]
struct Input {
    source_sha256: String,
    cases: Vec<Case>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("Expected INPUT OUTPUT".into());
    }
    let input: Input = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut probes = Vec::new();
    for c in input.cases {
        let mut tail = c.cache.clone();
        let mut tail_pass = c.pass.clone();
        let mut tail_last = c.last_pass;
        tail.disable_unused(
            &mut tail_pass.stages,
            usize::from(tail_pass.header[9]),
            c.device.capacity,
            c.hardware,
        )?;
        tail.finish_pass(tail_pass.color_write, tail_pass.address, &mut tail_last);
        let tail_state = tail.clone();
        let tail_calls = tail.flush(c.device.capacity, c.stencil_gate)?;
        let mut cache = c.cache;
        let mut pass = c.pass;
        let mut last = c.last_pass;
        let changed =
            d3d_pass::apply_hardware_pass(&mut cache, &mut last, &mut pass, c.resources, c.device)?;
        let translated = cache.clone();
        let calls = cache.flush(c.device.capacity, c.stencil_gate)?;
        let flushed = cache.clone();
        // Same pointer must skip before validating the changed contents or device.
        pass.header[9] = 255;
        let skipped = !d3d_pass::apply_hardware_pass(
            &mut cache,
            &mut last,
            &mut pass,
            [None; 8],
            Device {
                capacity: 99,
                ..c.device
            },
        )?;
        probes.push(serde_json::json!({"id":c.id,"material_id":c.material_id,"tail_pass":tail_pass,"tail_state":tail_state,"tail_last":tail_last,"tail_calls":tail_calls,"tail_flushed":tail,"changed":changed,"translated":translated,"calls":calls,"flushed":flushed,"pass":pass,"last_pass":last,"skipped":skipped,"final":cache}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec(
            &serde_json::json!({"source_sha256":input.source_sha256,"probes":probes,"scope":"Original unused-stage tail in hardware/fixed branches, raw ColorWrite and pass identity; multi-stage hardware pass with resolved resources and no matrix stages. Explicit initial cache/device fixtures. No shader acquisition, matrix path, GPU calls, live scene or Android."}),
        )?,
    )?;
    Ok(())
}
