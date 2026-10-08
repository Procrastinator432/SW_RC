use rc_render::{Scene, Texture};
use std::{
    env, fs,
    io::{BufWriter, Write},
    path::Path,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err(
            "usage: rc-skeletal-textured-preview DRAW_REPORT MATERIAL_REPORT OUTPUT".into(),
        );
    }
    let draw: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let materials: serde_json::Value = serde_json::from_slice(&fs::read(&args[2])?)?;
    let output = Path::new(&args[3]);
    let mut previews = vec![];
    let mut camera = None;
    for p in draw["previews"].as_array().ok_or("previews")? {
        let original = materials["objects"]
            .as_array()
            .ok_or("material objects")?
            .iter()
            .find(|o| o["source_index"] == p["source_index"])
            .ok_or("mesh material")?;
        let bindings = original["bindings"].as_array().ok_or("bindings")?;
        let count = bindings
            .iter()
            .filter_map(|b| b["slot"].as_u64())
            .max()
            .map_or(0, |n| n as usize + 1);
        let mut slots = vec![None; count];
        for binding in bindings {
            if let Some(i) = binding["texture"].as_u64() {
                let t = &materials["textures"][i as usize];
                slots[binding["slot"].as_u64().ok_or("slot")? as usize] = Some((
                    Texture {
                        width: t["width"].as_u64().ok_or("width")? as usize,
                        height: t["height"].as_u64().ok_or("height")? as usize,
                        pixels: serde_json::from_value(t["pixels"].clone())?,
                    },
                    t["uv_scale"].as_f64().ok_or("scale")? as f32,
                ));
            }
        }
        let mut scene =
            Scene::from_snapshot(&fs::read(p["snapshot"].as_str().ok_or("snapshot")?)?)?;
        let resolved = scene.apply_skeletal_diffuse_slots(&slots)?;
        let (center, radius) = *camera.get_or_insert((scene.center, scene.radius));
        scene.center = center;
        scene.radius = radius;
        let pixels = scene.render(512, 512, 0.65, 0.1, 1.0)?;
        let frame = p["frame"].as_u64().ok_or("frame")?;
        let ppm = output.with_file_name(format!("skeletal-textured-preview-{frame}.ppm"));
        let mut image = BufWriter::new(fs::File::create(&ppm)?);
        image.write_all(b"P6\n512 512\n255\n")?;
        for color in pixels {
            image.write_all(&[(color >> 16) as u8, (color >> 8) as u8, color as u8])?;
        }
        image.flush()?;
        let snapshot = ppm.with_extension("rcscene");
        fs::write(&snapshot, scene.snapshot()?)?;
        previews.push(serde_json::json!({"frame":frame,"source_index":p["source_index"],"input_snapshot":p["snapshot"],"triangles":scene.triangles.len(),"diffuse_triangles":resolved,"textures":scene.textures.len(),"ppm":ppm,"snapshot":snapshot}));
    }
    fs::write(
        output,
        serde_json::to_vec_pretty(
            &serde_json::json!({"previews":previews,"scope":"Original mesh material selection without actor overrides and original base diffuse textures on the prior verified mesh-local animation geometry. Missing chains remain flat diagnostic surfaces; no original shader/effect or Android equivalence claim."}),
        )?,
    )?;
    println!("{} original-diffuse preview frames", previews.len());
    Ok(())
}
