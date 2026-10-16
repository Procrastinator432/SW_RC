//! Preserve opaque BSP material semantics in legacy alpha-tested scene packets.
//! Only proven opaque bindings get a private alpha-normalized texture copy.
//! Masked, blended and unresolved materials retain their exact existing pixels.
use rc_inspect::{assets::Assets, defaults};
use rc_package::{defaults::Catalog, properties::Value};
use std::{collections::HashMap, env, fs, path::Path};

fn opaque(assets: &mut Assets, catalog: &Catalog, path: &str) -> Result<bool, String> {
    let (class, props) = assets.material_properties(path)?;
    let value = |field| {
        catalog
            .instance(&class, &props, field, 0, path)
            .map(|r| r.value)
    };
    match class.as_str() {
        "Engine.Shader" => Ok(matches!(value("OutputBlending")?, Value::Byte(0))
            && matches!(value("Opacity")?, Value::Object { index: 0, .. })),
        "Engine.FinalBlend" => Ok(matches!(value("FrameBufferBlending")?, Value::Byte(0))
            && matches!(value("AlphaTest")?, Value::Bool(false))),
        "Engine.Texture" => Ok(matches!(value("bMasked")?, Value::Bool(false))
            && matches!(value("bAlphaTexture")?, Value::Bool(false))),
        _ => Err(format!("Unresolved opacity class {class}")),
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-bsp-opacity-build GAME_DATA SCENE OUTPUT_DIRECTORY".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let raw = fs::read(&args[2])?;
    rc_game::bundle::decode(&raw)?;
    let mut offset = 0;
    for depth in 0..16 {
        let header = raw.get(offset..offset + 12).ok_or("Truncated bundle")?;
        if !header.starts_with(b"RCPLAY") {
            return Err("Invalid bundle layer".into());
        }
        let n = u32::from_le_bytes(header[8..12].try_into()?) as usize;
        offset = offset.checked_add(12 + n).ok_or("Bundle size overflow")?;
        if &header[..8] == b"RCPLAY01" {
            break;
        }
        if depth == 15 {
            return Err("Bundle depth exceeded".into());
        }
    }
    let mut scene = rc_render::Scene::from_snapshot(raw.get(offset..).ok_or("Missing scene")?)?;
    let data = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let pkg = rc_package::read_package(&data)?;
    let binding = rc_package::level::world_binding(&pkg, &data)?;
    let bsp = rc_package::geometry::read_bsp(&pkg, &data, &pkg.exports[binding.model_export])?;
    let mut assets = Assets::new(&[
        &root.join("Textures"),
        &root.join("StaticMeshes"),
        &root.join("System"),
        &root.join("Maps"),
    ])?;
    let catalog = defaults::load(root)?;
    let mut materials = HashMap::new();
    let mut copies = HashMap::new();
    let mut audit = vec![];
    let mut matched = 0;
    let mut changed = 0;
    let mut ramp = 0;
    let mut indices: HashMap<([u32; 9], usize), Vec<usize>> = HashMap::new();
    let key = |p: [[f32; 3]; 3]| -> [u32; 9] {
        let mut k = [0; 9];
        for (i, v) in p.into_iter().flatten().enumerate() {
            k[i] = v.to_bits();
        }
        k
    };
    for (i, t) in scene.triangles.iter().enumerate() {
        indices
            .entry((key(t.points), t.surface))
            .or_default()
            .push(i);
    }
    for t in bsp.triangles()? {
        let Some(faces) = indices.get(&(key(t.points), t.surface)) else {
            continue;
        };
        let s = &bsp.surfaces[t.surface];
        let path = if s.material > 0 {
            format!("geo_01a.{}", s.material_path)
        } else {
            s.material_path.clone()
        };
        let proof = materials
            .entry(path.clone())
            .or_insert_with(|| opaque(&mut assets, &catalog, &path));
        // Unknown surface bits are not silently reinterpreted as opaque.
        let is_opaque = s.flags == 0 && *proof == Ok(true);
        for &i in faces {
            matched += 1;
            let Some(mut face) = scene.appearance[i] else {
                continue;
            };
            let old = face.texture;
            let low = scene.textures[old]
                .pixels
                .iter()
                .filter(|p| **p >> 24 < 128)
                .count();
            if is_opaque && low > 0 {
                let texture = *copies.entry(old).or_insert_with(|| {
                    let mut copy = scene.textures[old].clone();
                    for p in &mut copy.pixels {
                        *p |= 0xff000000;
                    }
                    let at = scene.textures.len();
                    scene.textures.push(copy);
                    at
                });
                face.texture = texture;
                scene.appearance[i] = Some(face);
                changed += 1;
                if t.surface == 9 {
                    ramp += 1;
                }
            }
            if low > 0 || proof.is_err() {
                audit.push(serde_json::json!({"triangle":i,"surface":t.surface,"flags":s.flags,"material":path,"texture_before":old,"texture_after":face.texture,"low_alpha_texels":low,"opaque_proof":proof,"action":if is_opaque && low>0{"opaque private copy"}else{"preserved"}}));
            }
        }
    }
    if ramp != 2 {
        return Err(format!(
            "Expected two repaired ramp faces, got {ramp}; inspect material audit: {materials:?}"
        )
        .into());
    }
    let mut output = raw[..offset].to_vec();
    output.extend(scene.snapshot()?);
    rc_game::bundle::decode(&output)?;
    fs::write(out.join("geo_01a.rcplay"), output)?;
    fs::write(
        out.join("opacity-audit.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"matched_bsp_faces":matched,"changed_faces":changed,"repaired_ramp_faces":ramp,"new_private_textures":copies.len(),"geometry_uv_collision_scripts_unchanged":true,"masked_blended_unknown_preserved":true,"materials":materials,"faces":audit}),
        )?,
    )?;
    println!(
        "Repaired {changed} opaque BSP faces, including both ramp triangles; {} private textures",
        copies.len()
    );
    Ok(())
}
