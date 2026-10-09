use rc_package::{
    d3d_bindings::Bindings,
    d3d_complete::{self, Complete, Lights, ShaderChoice},
    d3d_pass::{Device, Pass},
};
use serde::Deserialize;
use std::{env, fs};
#[derive(Deserialize)]
struct Light {
    id: usize,
    lights: Lights,
    dirty: u32,
    capacity: i32,
}
#[derive(Deserialize)]
struct Selector {
    id: usize,
    bindings: Bindings,
    dirty: u32,
    kind: u32,
    choice: ShaderChoice,
}
#[derive(Deserialize)]
struct Case {
    id: usize,
    material_id: Option<usize>,
    complete: Complete,
    pass: Pass,
    resources: [Option<[u32; 2]>; 8],
    device: Device,
    last_pass: u32,
    stencil_gate: bool,
    choice: ShaderChoice,
    stream_capacity: i32,
    light_capacity: i32,
}
#[derive(Deserialize)]
struct Flush {
    id: usize,
    complete: Complete,
    texture_capacity: usize,
    stream_capacity: i32,
    light_capacity: i32,
    stencil_gate: bool,
}
#[derive(Deserialize)]
struct Input {
    source_bindings_sha256: String,
    source_passes_sha256: String,
    lights: Vec<Light>,
    selectors: Vec<Selector>,
    cases: Vec<Case>,
    flushes: Vec<Flush>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("Expected INPUT OUTPUT".into());
    }
    let input: Input = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut lights = Vec::new();
    let mut selectors = Vec::new();
    let mut probes = Vec::new();
    let mut flushes = Vec::new();
    for c in input.lights {
        let mut l = c.lights;
        let calls = l.tail(c.dirty, c.capacity);
        let applied = l.clone();
        let repeated = l.tail(c.dirty, c.capacity);
        let expanded = l.tail(0x80, 8);
        lights.push(serde_json::json!({"id":c.id,"calls":calls,"applied":applied,"repeated":repeated,"expanded":expanded,"final":l}));
    }
    for c in input.selectors {
        let mut b = c.bindings;
        let mut dirty = c.dirty;
        let result = d3d_complete::select_pixel(&mut b, &mut dirty, c.kind, c.choice);
        selectors.push(serde_json::json!({"id":c.id,"result":result,"bindings":b,"dirty":dirty}));
    }
    for c in input.cases {
        let mut complete = c.complete;
        let mut pass = c.pass;
        let mut last = c.last_pass;
        let pass_plan = d3d_complete::apply_pass(
            &mut complete,
            &mut last,
            &mut pass,
            c.resources,
            c.device,
            c.choice,
        )?;
        let translated = complete.clone();
        let translated_pass = pass.clone();
        let plan = complete.flush(
            c.device.capacity,
            c.stream_capacity,
            c.light_capacity,
            c.stencil_gate,
        )?;
        let flushed = complete.clone();
        let repeated = complete.flush(
            c.device.capacity,
            c.stream_capacity,
            c.light_capacity,
            c.stencil_gate,
        )?;
        pass.header[9] = 255;
        let skipped = d3d_complete::apply_pass(
            &mut complete,
            &mut last,
            &mut pass,
            [None; 8],
            Device {
                capacity: 99,
                ..c.device
            },
            ShaderChoice {
                hardware: false,
                resolved: None,
            },
        )?;
        complete.deferred.states.dirty = 0xff;
        let expanded = complete.flush(8, 16, 8, true)?;
        probes.push(serde_json::json!({"id":c.id,"material_id":c.material_id,"pass_plan":pass_plan,"translated":translated,"translated_pass":translated_pass,"plan":plan,"flushed":flushed,"repeated":repeated,"skipped":skipped,"last_pass":last,"expanded":expanded,"final":complete}));
    }
    for c in input.flushes {
        let mut complete = c.complete;
        let plan = complete.flush(
            c.texture_capacity,
            c.stream_capacity,
            c.light_capacity,
            c.stencil_gate,
        )?;
        let flushed = complete.clone();
        let repeated = complete.flush(
            c.texture_capacity,
            c.stream_capacity,
            c.light_capacity,
            c.stencil_gate,
        )?;
        flushes.push(serde_json::json!({"id":c.id,"plan":plan,"flushed":flushed,"repeated":repeated,"final":complete}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec(
            &serde_json::json!({"source_bindings_sha256":input.source_bindings_sha256,"source_passes_sha256":input.source_passes_sha256,"lights":lights,"selectors":selectors,"probes":probes,"flushes":flushes,"scope":"All low-byte deferred state groups including raw light payload/enable cache; resolved fixed pixelshader selection and kind-1 immediate identity-plane upload; fixed/hardware pass integration with matrices. Explicit source/cache/resource/handle/light fixtures. Shader lookup and static container allocation external; no actual GPU calls, live scene, full game or Android."}),
        )?,
    )?;
    Ok(())
}
