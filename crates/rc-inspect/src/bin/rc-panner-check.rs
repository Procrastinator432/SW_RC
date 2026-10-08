use rc_inspect::assets::Assets;
use rc_package::{
    geometry::Triangle,
    material_panner::{rotator_direction, TexPanner},
};
use rc_render::{FaceTexture, Scene};
use std::{env, fs, io::Write, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-panner-check MATERIAL_REPORT GAME_DATA OUTPUT".into());
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
            if b["error"] == "Unsupported material class Engine.TexPanner" {
                paths.insert(b["material"].as_str().ok_or("material")?.to_owned());
                slots += 1;
            }
        }
    }
    let mut materials = vec![];
    for path in paths {
        if assets.diffuse_at_size(&path, 256).is_ok() {
            return Err("scalar-only resolver accepted TexPanner".into());
        }
        let d = assets.diffuse_at_time(&path, 256, 0.)?;
        let p = d.directional_panner.ok_or("panner")?;
        let mut samples = vec![];
        for (frame, time) in [0., 2., 4., 6.].into_iter().enumerate() {
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
            let timed = assets.diffuse_at_time(&path, 256, time)?;
            scene.apply_skeletal_diffuse_slots(&[Some((timed.texture, 1.))])?;
            scene.apply_skeletal_uv_transforms(&[timed.uv_transform])?;
            let snapshot = Path::new(&args[3]).with_file_name(format!(
                "panner-material-{}-{frame}.rcscene",
                materials.len()
            ));
            fs::write(&snapshot, scene.snapshot()?)?;
            let ppm = snapshot.with_extension("ppm");
            let mut image = std::io::BufWriter::new(fs::File::create(&ppm)?);
            image.write_all(b"P6\n256 256\n255\n")?;
            for color in scene.render(256, 256, 0., 0., 1.)? {
                image.write_all(&[(color >> 16) as u8, (color >> 8) as u8, color as u8])?;
            }
            image.flush()?;
            samples.push(serde_json::json!({"time":time,"transform":p.at(time)?,"snapshot":snapshot,"ppm":ppm}));
        }
        materials.push(serde_json::json!({"material":path,"source":d.source,"chain":d.chain,"parameters":p,"direction":rotator_direction(p.direction),"samples":samples,"width":d.texture.width,"height":d.texture.height,"pixels":d.texture.pixels,"format":d.format,"source_mip":d.source_mip}));
    }
    let mut probes = vec![];
    for i in 0i32..512 {
        let p = TexPanner {
            direction: [
                i.wrapping_mul(125621) - 900000,
                i.wrapping_mul(-263193) + i32::MAX,
                i.wrapping_mul(173),
            ],
            rate: (i as f32 - 200.) / 19.,
        };
        let time = (i as f32 - 300.) * 13.25;
        probes.push(serde_json::json!({"parameters":p,"time":time,"direction":rotator_direction(p.direction),"transform":p.at(time)?}));
    }
    for time in [
        -1024.0001, -1024., -0.00001, -0.0, 0., 1023.9999, 1024., 1024.0001,
    ] {
        let p = TexPanner {
            direction: [0; 3],
            rate: 1.,
        };
        probes.push(serde_json::json!({"parameters":p,"time":time,"direction":rotator_direction(p.direction),"transform":p.at(time)?}));
    }
    fs::write(
        &args[3],
        serde_json::to_vec(
            &serde_json::json!({"resolved_slots":slots,"materials":materials,"probes":probes,"scope":"Original TexPanner base diffuse and time sampled UV transform on diagnostic material quads. Reconstructed native quantized direction table. No full shader, mesh/world rendering, script parameter updates or Android verification."}),
        )?,
    )?;
    println!(
        "{slots} formerly omitted slots, {} material paths, {} direction/phase probes",
        materials.len(),
        probes.len()
    );
    Ok(())
}
