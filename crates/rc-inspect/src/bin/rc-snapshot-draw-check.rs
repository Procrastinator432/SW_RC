//! Original-layout snapshots and serialized shader inputs through the CPU mesh pass.
use rc_inspect::assets::Assets;
use rc_package::{
    material_constants::Flicker,
    shader_constants::{scene::portable_seed, Bank, Constant, Matrix},
    shader_snapshot::{capture, Globals, Memory, Region},
    skeletal_matrix_inverse::inverse_matrix,
};
use rc_render::{
    hologram_pass::HologramPass,
    shader_raster::{Blend, State, Target},
};
use serde::Deserialize;
use std::{fs, path::Path};
#[derive(Deserialize)]
struct Case {
    id: usize,
    renderer: u32,
    globals: Globals,
    regions: Vec<Region>,
}
#[derive(Deserialize)]
struct Input {
    cases: Vec<Case>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 5 {
        return Err("Expected GAME_DATA SNAPSHOTS DRAW_BIN OUTPUT".into());
    }
    let game = Path::new(&args[1]);
    let dirs = [game.join("Textures"), game.join("System")];
    let mut assets = Assets::new(&dirs.iter().map(|p| p.as_path()).collect::<Vec<_>>())?;
    let name = "HardwareShaders.Hologram.DynamicHologram";
    let shader = assets.hardware_shader_inputs(name)?;
    let pass = HologramPass::new(&shader.vertex_source, &shader.pixel_source)?;
    let paths: Vec<_> = (0..3)
        .map(|slot| {
            shader
                .textures
                .iter()
                .find(|t| t.slot == slot)
                .and_then(|t| t.material.clone())
                .ok_or("Missing explicit texture binding")
        })
        .collect::<Result<_, _>>()?;
    let textures = paths
        .iter()
        .map(|p| assets.diffuse_at_size(p, 256).map(|d| d.texture))
        .collect::<Result<Vec<_>, _>>()?;
    let bytes = fs::read(&args[3])?;
    if !bytes.starts_with(b"RCSKDRAW\x01\0\0\0") || (bytes.len() - 12) % 132 != 0 {
        return Err("Invalid skeletal stream".into());
    }
    let mut source = Vec::new();
    for record in bytes[12..].chunks_exact(132) {
        let word = |i: usize| u32::from_le_bytes(record[i * 4..i * 4 + 4].try_into().unwrap());
        if word(0) == 25 && word(1) == 0 {
            source.push(std::array::from_fn::<_, 24, _>(|i| word(i + 9)));
        }
    }
    if source.len() != 3500 {
        return Err("Expected CloneCommando LOD0 stream".into());
    }
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for tri in &source {
        for v in tri.chunks_exact(8) {
            for i in 0..3 {
                let x = f32::from_bits(v[i]);
                if !x.is_finite() {
                    return Err("Invalid source position".into());
                }
                min[i] = min[i].min(x);
                max[i] = max[i].max(x);
            }
        }
    }
    let center = std::array::from_fn::<_, 3, _>(|i| (min[i] + max[i]) * 0.5);
    let radius = (0..3).map(|i| (max[i] - min[i]) * 0.5).fold(0f32, f32::max);
    if radius <= 0. {
        return Err("Empty source extent".into());
    }
    let mut vc = vec![Constant::default(); 96];
    let mut pc = vec![Constant::default(); 8];
    for (bindings, bank) in [
        (&shader.vertex_constants, &mut vc),
        (&shader.pixel_constants, &mut pc),
    ] {
        for b in bindings {
            bank[b.slot] = Constant {
                kind: b.kind,
                words: b.value.map(f32::to_bits),
            };
        }
    }
    let mut vb = Bank::new(vec![[0; 4]; 96])?;
    vb.words[17] = [0.25f32.to_bits(); 4];
    let mut pb = Bank::new(vec![[0; 4]; 8])?;
    let mut flicker = Flicker::default();
    let input: Input = serde_json::from_slice(&fs::read(&args[2])?)?;
    let mut frames = Vec::new();
    let mut successful = 0;
    let mut rejected = 0;
    for case in input.cases.into_iter().filter(|c| c.id < 256) {
        let (host, _) = capture(
            &Memory::new(case.regions)?,
            case.renderer,
            case.globals,
            Some(portable_seed),
        )?;
        let mut draws = 0usize;
        let mut inverse = |m: Matrix| {
            let raw = inverse_matrix(std::array::from_fn(|k| m[k / 4][k % 4]));
            Ok(std::array::from_fn(|r| {
                std::array::from_fn(|c| raw[r * 4 + c])
            }))
        };
        let mut random = || {
            let n = ((case.id * 73 + draws * 127) % 32768) as u16;
            draws += 1;
            Ok(n)
        };
        let status = vb
            .update(&vc, host, &mut flicker, &mut inverse, &mut random)
            .and_then(|()| pb.update(&pc, host, &mut flicker, &mut inverse, &mut random));
        if status.is_err() {
            rejected += 1;
            continue;
        }
        successful += 1;
        if ![251, 255].contains(&case.id) {
            continue;
        }
        let mut target = Target::new(128, 128, 0xff101820)?;
        let state = State {
            depth_test: true,
            depth_write: false,
            alpha_reference: Some(0),
            blend: Blend::SourceAlphaAdditive,
        };
        let mut projected = Vec::new();
        let mut stats = [0usize; 4];
        for triangle in &source {
            let inputs = std::array::from_fn(|corner| {
                let v = &triangle[corner * 8..corner * 8 + 8];
                let mut registers = [[0.; 4]; 16];
                registers[0] = [
                    (f32::from_bits(v[0]) - center[0]) / radius * 1.5,
                    (f32::from_bits(v[1]) - center[1]) / radius * 1.5,
                    (f32::from_bits(v[2]) - center[2]) / radius * 1.5,
                    1.,
                ];
                registers[1] = [
                    f32::from_bits(v[3]),
                    f32::from_bits(v[4]),
                    f32::from_bits(v[5]),
                    0.,
                ];
                registers[2] = [f32::from_bits(v[6]), f32::from_bits(v[7]), 0., 1.];
                registers
            });
            let prepared = pass.prepare(inputs, &vb, &pb)?;
            projected.push(
                prepared
                    .vertices
                    .map(|v| serde_json::json!({"clip":v.clip,"varying":v.varying})),
            );
            let s = pass.draw(
                &prepared,
                [&textures[0], &textures[1], &textures[2]],
                &mut target,
                state,
            )?;
            for (total, n) in
                stats
                    .iter_mut()
                    .zip([s.covered, s.depth_rejected, s.alpha_rejected, s.written])
            {
                *total += n;
            }
        }
        frames.push(serde_json::json!({"snapshot":case.id,"vertex_bank":vb,"pixel_bank":pb,"flicker":flicker,"projected":projected,"width":128,"height":128,"pixels":target.pixels(),"depth_words":target.depth().iter().map(|v|v.to_bits()).collect::<Vec<_>>(),"stats":stats}));
    }
    if frames.len() != 2 {
        return Err("Expected both diagnostic frames".into());
    }
    fs::write(
        &args[4],
        serde_json::to_vec(
            &serde_json::json!({"shader":name,"inputs":shader,"texture_paths":paths,"source_words":source,"center":center,"radius":radius,"fit_scale":1.5,"frames":frames,"successful_updates":successful,"rejected_updates":rejected,"scope":"Synthetic captured-layout snapshots -> original serialized programs/constants/textures -> reconstructed CloneCommando LOD0 -> diagnostic CPU raster. No post-dispatch register overrides, wrapper texture/tint inference, live scene, native raster/state parity or Android execution. Explicit c17 .25, mesh fit 1.5 and sampler/raster fixtures."}),
        )?,
    )?;
    Ok(())
}
