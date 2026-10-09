//! Original-layout snapshots and serialized shader inputs through the CPU mesh pass.
use rc_inspect::assets::Assets;
use rc_package::{
    hardware_constants::{Counts, DeviceConstants},
    hardware_stages,
    material_constants::Flicker,
    shader_constants::{scene::portable_seed, Constant, Matrix},
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
    let mut slots: [Option<String>; 8] = Default::default();
    for texture in &shader.textures {
        let slot = usize::try_from(texture.slot)?;
        if slot >= 8 {
            return Err("Invalid hardware texture slot".into());
        }
        slots[slot] = texture.material.clone();
    }
    let mut stages = [[0u32; 28]; 8];
    let mut resolved = Vec::new();
    let bound = hardware_stages::bind(&slots, [[0; 3]; 8], 8, &mut stages, &mut |slot, path| {
        resolved.push((slot, path.clone()));
        Ok(Some([0; 28]))
    })?;
    if bound != 3 {
        return Err("Expected three contiguous hardware textures".into());
    }
    let paths: Vec<_> = (0..bound)
        .map(|slot| {
            shader
                .textures
                .iter()
                .find(|t| t.slot == slot as u32)
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
    let mut vc = [Constant::default(); 96];
    let mut pc = [Constant::default(); 8];
    for (bindings, bank) in [
        (&shader.vertex_constants, &mut vc[..]),
        (&shader.pixel_constants, &mut pc[..]),
    ] {
        for b in bindings {
            bank[b.slot] = Constant {
                kind: b.kind,
                words: b.value.map(f32::to_bits),
            };
        }
    }
    let mut initial = [[0; 4]; 96];
    initial[17] = [0.25f32.to_bits(); 4];
    let mut device = DeviceConstants::new(initial, [[0; 4]; 8], initial);
    let mut counts = Counts::default();
    let mut flicker = Flicker::default();
    let input: Input = serde_json::from_slice(&fs::read(&args[2])?)?;
    let mut frames = Vec::new();
    let mut probes = Vec::new();
    let mut successful = 0;
    let mut rejected = 0;
    for case in input.cases.into_iter().filter(|c| c.id < 256) {
        let (host, _) = capture(
            &Memory::new(case.regions)?,
            case.renderer,
            case.globals,
            Some(portable_seed),
        )?;
        let before = serde_json::json!({"device":device,"counts":counts,"flicker":flicker});
        let mut draws = 0usize;
        let mut inverse_calls = Vec::new();
        let mut rng = Vec::new();
        let mut inverse = |m: Matrix| {
            inverse_calls.push(m);
            let raw = inverse_matrix(std::array::from_fn(|k| m[k / 4][k % 4]));
            Ok(std::array::from_fn(|r| {
                std::array::from_fn(|c| raw[r * 4 + c])
            }))
        };
        let mut random = || {
            let n = ((case.id * 73 + draws * 127) % 32768) as u16;
            draws += 1;
            rng.push(n);
            Ok(n)
        };
        let status = device.update(
            &pc,
            &vc,
            &mut counts,
            host,
            &mut flicker,
            &mut inverse,
            &mut random,
        );
        probes.push(serde_json::json!({"id":case.id,"before":before,"device":device,"counts":counts,"flicker":flicker,"status":status,"rng":rng,"inverse_calls":inverse_calls}));
        if status.is_err() {
            rejected += 1;
            continue;
        }
        successful += 1;
        if ![251, 255].contains(&case.id) {
            continue;
        }
        let vb = &device.vertex;
        let pb = &device.pixel;
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
            let prepared = pass.prepare(inputs, vb, pb)?;
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
    let mut stage_probes = Vec::new();
    for scenario in 0..768usize {
        let mask = scenario % 256;
        let variant = scenario / 256;
        let slots = std::array::from_fn(|i| ((mask >> i) & 1 != 0).then_some(i as u32 + 1));
        let transforms = std::array::from_fn(|i| {
            [
                match (mask + i) % 4 {
                    0 => 0,
                    1 => 0x80000000,
                    2 => 0x7fc12345,
                    _ => 2f32.to_bits(),
                },
                (mask * 31 + i) as u32,
                (mask * 73 + i) as u32,
            ]
        });
        let initial = std::array::from_fn(|i| {
            std::array::from_fn(|w| {
                (mask as u32 * 1009 + i as u32 * 97 + w as u32).wrapping_mul(0x9e3779b9)
            })
        });
        let mut stages = initial;
        let limit = if variant == 1 { mask % 9 } else { 8 };
        let fail = if variant == 2 { mask % 8 } else { 8 };
        let mut calls = Vec::new();
        let bound =
            hardware_stages::bind(&slots, transforms, limit, &mut stages, &mut |index, _| {
                calls.push(index);
                Ok((index != fail).then_some(initial[index]))
            })?;
        stage_probes.push(serde_json::json!({"mask":mask,"variant":variant,"limit":limit,"fail":fail,"initial":initial,"transforms":transforms,"calls":calls,"bound":bound,"stages":stages}));
    }
    fs::write(
        &args[4],
        serde_json::to_vec(
            &serde_json::json!({"shader":name,"inputs":shader,"texture_paths":paths,"source_words":source,"center":center,"radius":radius,"fit_scale":1.5,"frames":frames,"probes":probes,"bound_stages":bound,"resolved":resolved,"stages":stages,"stage_probes":stage_probes,"successful_updates":successful,"rejected_updates":rejected,"scope":"Synthetic captured-layout snapshots -> original serialized programs/constants/textures -> reconstructed CloneCommando LOD0 -> diagnostic CPU raster. PS-first shared scratch with separate retained device uploads; contiguous texture-stage walk. No wrapper texture/tint inference, live scene, native sampler/raster/state parity or Android execution. Explicit c17 .25, mesh fit 1.5 and sampler/raster fixtures."}),
        )?,
    )?;
    Ok(())
}
