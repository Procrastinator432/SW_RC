//! Sparse original-layout material/sampler state capture and native state transitions.
use rc_package::{
    hardware_material, hardware_sampler, hardware_state_snapshot,
    shader_snapshot::{Memory, Region},
};
use serde::Deserialize;
use std::{env, fs};
#[derive(Deserialize)]
struct Material {
    id: usize,
    renderer: u32,
    shader: u32,
    regions: Vec<Region>,
    setup_return: i32,
    mutate: bool,
    diagnostic_present: bool,
    fallback_present: bool,
}
#[derive(Deserialize)]
struct Sampler {
    id: usize,
    renderer: u32,
    texture: u32,
    resource: u32,
    regions: Vec<Region>,
    stage: [u32; 28],
    renderer_flags: u32,
}
#[derive(Deserialize)]
struct Input {
    materials: Vec<Material>,
    samplers: Vec<Sampler>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("Expected INPUT OUTPUT".into());
    }
    let input: Input = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut materials = Vec::new();
    let mut samplers = Vec::new();
    for case in input.materials {
        let memory = Memory::new(case.regions)?;
        let (mut host, shader, capabilities) =
            match hardware_state_snapshot::material(&memory, case.renderer, case.shader) {
                Ok(v) => v,
                Err(e) => {
                    materials.push(serde_json::json!({"id":case.id,"capture_error":e}));
                    continue;
                }
            };
        let before = host.clone();
        let mut text = String::from("previous");
        let mut fallback = 0xdeadbeefu32;
        let mut calls = 0;
        let status = hardware_material::set(
            &mut host,
            shader,
            capabilities,
            case.diagnostic_present.then_some(&mut text),
            case.fallback_present.then_some(&mut fallback),
            &mut |host| {
                calls += 1;
                if case.mutate {
                    host.renderer_flags ^= 0x80000000;
                    host.pass[0] ^= 0x5a;
                    host.active_passes ^= 0x80;
                }
                Ok(case.setup_return)
            },
        )?;
        materials.push(serde_json::json!({"id":case.id,"before":before,"shader":shader,"capabilities":capabilities,"host":host,"status":status,"diagnostic":text,"fallback":fallback,"setup_calls":calls}));
    }
    for case in input.samplers {
        let memory = Memory::new(case.regions)?;
        let (texture, resource, filter) = match hardware_state_snapshot::sampler(
            &memory,
            case.renderer,
            case.texture,
            case.resource,
        ) {
            Ok(v) => v,
            Err(e) => {
                samplers.push(serde_json::json!({"id":case.id,"capture_error":e}));
                continue;
            }
        };
        let mut stage = case.stage;
        let mut flags = case.renderer_flags;
        hardware_sampler::bind(&mut stage, &mut flags, texture, resource, filter);
        samplers.push(serde_json::json!({"id":case.id,"texture":texture,"resource":resource,"filter":filter,"stage":stage,"renderer_flags":flags}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec(
            &serde_json::json!({"materials":materials,"samplers":samplers,"scope":"Synthetic sparse original x86 layouts -> eager material and post-cache-lookup sampler capture -> reconstructed raw pass/resource/address/filter state transitions. Native capability/fallback and diagnostic paths. External setup/resource creation remain host inputs; no enum/GPU-state application, class-default mapping, live scene, image parity or Android execution."}),
        )?,
    )?;
    Ok(())
}
