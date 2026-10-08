use rc_inspect::assets::{Assets, MaterialState};
use rc_package::{
    geometry::Triangle,
    material_jitter::{JitterState, TexJitter},
};
use rc_render::{FaceTexture, Scene};
use std::{env, fs, io::Write, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-jitter-check MATERIAL_REPORT GAME_DATA OUTPUT".into());
    }
    let old: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let game = Path::new(&args[2]);
    let dirs = [
        game.join("Textures"),
        game.join("StaticMeshes"),
        game.join("Animations"),
        game.join("System"),
    ];
    let mut assets = Assets::new(&dirs.iter().map(|p| p.as_path()).collect::<Vec<_>>())?;
    let mut paths = std::collections::BTreeSet::new();
    let mut slots = 0;
    for o in old["objects"].as_array().ok_or("objects")? {
        for b in o["bindings"].as_array().ok_or("bindings")? {
            if b["error"] == "Unsupported material class Engine.TexOscillator" {
                paths.insert(b["material"].as_str().ok_or("material")?.to_owned());
                slots += 1;
            }
        }
    }
    let mut materials = vec![];
    for path in paths {
        if assets.diffuse_at_time(&path, 256, 0.).is_ok() {
            return Err("stateless sampling unexpectedly accepted jitter".into());
        }
        let sequence: Vec<u16> = (0..32)
            .map(|i| ((i * 7919 + 1234) % 32768) as u16)
            .collect();
        let mut cursor = 0;
        let mut runtime = MaterialState::default();
        let mut samples = vec![];
        let mut description = None;
        for (frame, time) in [0., 0.05, 0.075, 0.1].into_iter().enumerate() {
            let input = if frame == 1 {
                path.to_uppercase()
            } else {
                path.clone()
            };
            let before = runtime.oscillators.clone();
            let start = cursor;
            let d = assets.diffuse_with_jitter(&input, 256, time, &mut runtime, &mut || {
                let v = *sequence.get(cursor).ok_or("random fixture exhausted")?;
                cursor += 1;
                Ok(v)
            })?;
            if runtime.oscillators.len() != 1 {
                return Err("jitter runtime did not reuse material state".into());
            }
            let mut scene = Scene::new(vec![
                Triangle {
                    points: [[0., -1., -1.], [0., 1., -1.], [0., -1., 1.]],
                    surface: 0,
                },
                Triangle {
                    points: [[0., 1., -1.], [0., 1., 1.], [0., -1., 1.]],
                    surface: 0,
                },
            ])?;
            scene.appearance = vec![
                Some(FaceTexture {
                    uv: [[0., 0.], [1., 0.], [0., 1.]],
                    texture: 0,
                }),
                Some(FaceTexture {
                    uv: [[1., 0.], [1., 1.], [0., 1.]],
                    texture: 0,
                }),
            ];
            scene.apply_skeletal_diffuse_slots(&[Some((d.texture.clone(), 1.))])?;
            scene.apply_skeletal_uv_transforms(&[d.uv_transform])?;
            let snapshot = Path::new(&args[3]).with_file_name(format!(
                "jitter-material-{}-{frame}.rcscene",
                materials.len()
            ));
            fs::write(&snapshot, scene.snapshot()?)?;
            let ppm = snapshot.with_extension("ppm");
            let mut image = std::io::BufWriter::new(fs::File::create(&ppm)?);
            image.write_all(b"P6\n256 256\n255\n")?;
            for c in scene.render(256, 256, 0., 0., 1.)? {
                image.write_all(&[(c >> 16) as u8, (c >> 8) as u8, c as u8])?;
            }
            image.flush()?;
            samples.push(serde_json::json!({"time":time,"before":before,"after":runtime.oscillators,"random_start":start,"random_end":cursor,"transform":d.uv_transform,"snapshot":snapshot,"ppm":ppm}));
            description.get_or_insert(serde_json::json!({"material":path,"source":d.source,"chain":d.chain,"parameters":d.jitter,"width":d.texture.width,"height":d.texture.height,"pixels":d.texture.pixels,"format":d.format,"source_mip":d.source_mip}));
        }
        let mut d = description.unwrap();
        d["samples"] = serde_json::json!(samples);
        d["random_fixture"] = serde_json::json!(sequence);
        materials.push(d);
    }
    let mut probes = vec![];
    for i in 0..128 {
        let p = TexJitter {
            rate: [(i as f32 - 40.) / 3., (i % 17) as f32 / 5.],
            phase: [(i % 5) as f32 / 2., (i % 7) as f32 / 3.],
            amplitude: [(i % 11) as f32 / 7., if i % 3 == 0 { 0. } else { 0.75 }],
            kind: [3; 2],
            pivot: [0.; 2],
        };
        let initial = JitterState {
            last: [(i % 13) as f32, (i % 9) as f32],
            current: [0.25, -0.125],
        };
        let mut state = initial;
        let sequence: Vec<u16> = (0..96)
            .map(|j| ((i * 541 + j * 7919) % 32768) as u16)
            .collect();
        let mut cursor = 0;
        let mut steps = vec![];
        for step in 0..24 {
            let time = if step % 7 == 0 {
                -(step as f32) / 4.
            } else {
                step as f32 / 3.
            };
            let before = state;
            let start = cursor;
            let transform = p.step(time, &mut state, &mut || {
                let v = *sequence.get(cursor).ok_or("random probe exhausted")?;
                cursor += 1;
                Ok(v)
            })?;
            steps.push(serde_json::json!({"time":time,"before":before,"after":state,"random_start":start,"random_end":cursor,"transform":transform}));
        }
        probes.push(serde_json::json!({"parameters":p,"initial":initial,"random_fixture":sequence,"steps":steps}));
    }
    fs::write(
        &args[3],
        serde_json::to_vec(
            &serde_json::json!({"resolved_slots":slots,"materials":materials,"probes":probes,"scope":"Dual-axis OT_Jitter with zero pivots. Original serialized parameters/state, persistent material state and caller-supplied shared CRT rand samples. Fixture random values demonstrate consumption/state transitions, not the original game's RNG sequence. Diagnostic diffuse quads, no original shader/effects or Android verification."}),
        )?,
    )?;
    println!(
        "{slots} formerly omitted slots, {} material paths, 3072 state transitions",
        materials.len()
    );
    Ok(())
}
