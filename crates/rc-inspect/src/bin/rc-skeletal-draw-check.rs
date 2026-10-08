use rc_package::skeletal_render_product::rigid_render_product;
use rc_package::{
    read_package,
    skeletal_draw::build_skeletal_draw,
    skeletal_lod::{inspect_skin_program, read_skeletal_lods},
    skeletal_mesh::read_skeletal_mesh_prefix,
    skeletal_reference_product::compose_reference_matrix,
    skeletal_root_pose::{quaternion_translation_matrix, RootTransform},
    skeletal_skin::{build_skin_palette, skin_lod_stream, SkinStreamVertex},
};
use rc_render::Scene;
use std::{
    env, fs,
    io::{BufWriter, Write},
    path::Path,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 5 {
        return Err(
            "usage: rc-skeletal-draw-check LINKUPS FULL_TICKS REFERENCE_CACHE OUTPUT".into(),
        );
    }
    let links: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let poses: serde_json::Value = serde_json::from_slice(&fs::read(&args[2])?)?;
    let inverses: serde_json::Value = serde_json::from_slice(&fs::read(&args[3])?)?;
    let output = Path::new(&args[4]);
    let binary = output.with_extension("bin");
    let mut writer = BufWriter::new(fs::File::create(&binary)?);
    writer.write_all(b"RCSKDRAW\x01\0\0\0")?;
    let mut objects = vec![];
    let mut triangle_counts = [0usize; 2];
    let mut previews = vec![];
    for prior in poses["objects"].as_array().ok_or("objects")? {
        let index = prior["source_index"].as_u64().ok_or("source index")? as usize;
        let original = &links["objects"][index];
        let data = fs::read(original["file"].as_str().ok_or("file")?)?;
        let pkg = read_package(&data)?;
        let e = &pkg.exports[original["export_index"].as_u64().ok_or("export")? as usize - 1];
        let mesh = read_skeletal_lods(&pkg, &data, e)?;
        let inv = inverses["objects"]
            .as_array()
            .ok_or("inverse objects")?
            .iter()
            .find(|o| o["source_index"] == index)
            .ok_or("inverse")?;
        let inv: Vec<[u32; 16]> = serde_json::from_value(inv["inverse"].clone())?;
        let animated = !prior["runs"].as_array().ok_or("runs")?.is_empty();
        let first = &prior["runs"][0]["cases"][0];
        let matrices: Vec<[u32; 16]> = if animated {
            serde_json::from_value(first["full"]["matrices"].clone())?
        } else {
            let prefix = read_skeletal_mesh_prefix(&pkg, &data, e)?;
            let mut reference = vec![];
            for (i, b) in prefix.bones.iter().enumerate() {
                let mut m = quaternion_translation_matrix(RootTransform {
                    rotation: b.rotation,
                    position: b.position,
                });
                if i > 0 {
                    m = compose_reference_matrix(m, reference[b.word_34 as usize]);
                }
                reference.push(m);
            }
            reference
        };
        let mut palette = vec![];
        build_skin_palette(&matrices, &inv, &mut palette)?;
        let mut lods = vec![];
        for (li, lod) in mesh.lods.iter().enumerate() {
            let mut soft = vec![];
            if !lod.commands.is_empty() {
                let p = inspect_skin_program(&lod.commands, lod.bind_vertices.len())?;
                soft.resize(p.outputs, SkinStreamVertex::default());
                skin_lod_stream(
                    &lod.commands,
                    &lod.bind_vertices,
                    &palette,
                    &mut soft,
                    &mut vec![],
                )?;
            }
            let draw = build_skeletal_draw(lod, &soft, &matrices, &inv)
                .map_err(|e| format!("{} LOD {li}: {e}", original["object"]))?;
            for t in &draw.triangles {
                let s = &draw.sections[t.section];
                triangle_counts[s.bank] += 1;
                let header = [
                    index as u32,
                    li as u32,
                    s.bank as u32,
                    s.section_index as u32,
                    s.material as u32,
                    s.bone.map_or(u32::MAX, u32::from),
                ];
                for w in header.into_iter().chain(t.indices.map(u32::from)).chain(
                    t.vertices.into_iter().flat_map(|v| {
                        v.vertex
                            .position
                            .into_iter()
                            .chain(v.vertex.normal)
                            .chain(v.uv)
                    }),
                ) {
                    writer.write_all(&w.to_le_bytes())?;
                }
            }
            let products: Vec<_> = draw
                .sections
                .iter()
                .map(|s| {
                    s.bone
                        .filter(|_| s.triangle_count > 0)
                        .map(|b| rigid_render_product(inv[b as usize], matrices[b as usize]))
                })
                .collect();
            lods.push(serde_json::json!({"sections":draw.sections,"rigid_products":products,"triangles":draw.triangles.len(),"unused_indices":draw.unused_indices}));
        }
        objects.push(serde_json::json!({"source_index":index,"object":original["object"],"pose_source":if animated {"AnimatedFullPose"} else {"OriginalReferencePose"},"lods":lods}));
        if original["object"] == "CloneCommando" && animated {
            let lod = &mesh.lods[0];
            let p = inspect_skin_program(&lod.commands, lod.bind_vertices.len())?;
            let mut camera = None;
            for (frame, case) in prior["runs"][0]["cases"]
                .as_array()
                .ok_or("preview cases")?
                .iter()
                .enumerate()
            {
                let matrices: Vec<[u32; 16]> =
                    serde_json::from_value(case["full"]["matrices"].clone())?;
                build_skin_palette(&matrices, &inv, &mut palette)?;
                let mut soft = vec![SkinStreamVertex::default(); p.outputs];
                skin_lod_stream(
                    &lod.commands,
                    &lod.bind_vertices,
                    &palette,
                    &mut soft,
                    &mut vec![],
                )?;
                let draw = build_skeletal_draw(lod, &soft, &matrices, &inv)?;
                let mut scene = Scene::from_skeletal_draw(&draw)?;
                let (center, radius) = *camera.get_or_insert((scene.center, scene.radius));
                scene.center = center;
                scene.radius = radius;
                let pixels = scene.render(512, 512, 0.65, 0.1, 1.0)?;
                let ppm = output.with_file_name(format!("skeletal-preview-{frame}.ppm"));
                let mut image = BufWriter::new(fs::File::create(&ppm)?);
                image.write_all(b"P6\n512 512\n255\n")?;
                for color in pixels {
                    image.write_all(&[(color >> 16) as u8, (color >> 8) as u8, color as u8])?;
                }
                image.flush()?;
                let snapshot = ppm.with_extension("rcscene");
                fs::write(&snapshot, scene.snapshot()?)?;
                previews.push(serde_json::json!({"frame":frame,"step":case["step"],"source_index":index,"animation":prior["runs"][0]["animation"],"entry":prior["runs"][0]["entry"],"lod":0,"triangles":draw.triangles.len(),"ppm":ppm,"snapshot":snapshot}));
            }
        }
    }
    writer.flush()?;
    let mut products = vec![];
    for seed in 0..64usize {
        let inverse =
            std::array::from_fn(|i| ((((i * 13 + seed * 17) % 47) as f32 - 23.) * 0.125).to_bits());
        let pose =
            std::array::from_fn(|i| ((((i * 7 + seed * 11) % 41) as f32 - 20.) * 0.2).to_bits());
        products.push(serde_json::json!({"inverse":inverse,"pose":pose,"result":rigid_render_product(inverse,pose)}));
    }
    fs::write(
        output,
        serde_json::to_vec_pretty(
            &serde_json::json!({"objects":objects,"triangle_counts":triangle_counts,"binary":binary,"previews":previews,"product_probes":products,
        "scope":"Original soft/rigid section and index selection; exact native rigid inverse/pose product; mesh-local CPU diagnostic transform and checker material preview. Actor/world transform, original material resolution/effects and GPU arithmetic remain external; no Android verification."}),
        )?,
    )?;
    println!(
        "{} soft triangles, {} rigid triangles; {} preview frames",
        triangle_counts[0],
        triangle_counts[1],
        previews.len()
    );
    Ok(())
}
