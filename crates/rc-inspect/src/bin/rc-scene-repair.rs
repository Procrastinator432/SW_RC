//! Repair the existing mission's missing surface bindings and DC15s fire sample.
//! Collision, mission scripts and the prepared actor geometry remain unchanged.
use rc_game::{bundle, mission};
use rc_inspect::{assets::Assets, defaults};
use rc_package::{geometry, properties::Value, read_package};
use std::{collections::HashMap, env, fs, path::Path};
type MaterialBinding = (usize, f32, usize, [f32; 2]);
fn package_object(
    root: &Path,
    folder: &str,
    extension: &str,
    path: &str,
) -> Result<(Vec<u8>, String), String> {
    let (package, object) = path.split_once('.').ok_or("Unqualified asset")?;
    let file = fs::read_dir(root.join(folder))
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| {
            p.file_name().is_some_and(|n| {
                n.to_string_lossy()
                    .eq_ignore_ascii_case(&format!("{package}.{extension}"))
            })
        })
        .ok_or(format!("Missing asset package {package}"))?;
    Ok((fs::read(file).map_err(|e| e.to_string())?, object.into()))
}
fn texture_size(root: &Path, path: &str) -> Result<[f32; 2], String> {
    let (data, object) = package_object(root, "Textures", "utx", path)?;
    let p = read_package(&data)?;
    let e = p
        .exports
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            p.object_path(i as i32 + 1)
                .is_ok_and(|s| s.eq_ignore_ascii_case(&object))
                .then_some(e)
        })
        .ok_or("Missing texture")?;
    let t = rc_package::texture::read_texture(&p, &data, e)?;
    let m = t.mips.first().ok_or("Missing texture dimensions")?;
    Ok([m.width as f32, m.height as f32])
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-scene-repair GAME_DATA ORIGINAL_MISSION OUTPUT_DIR".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let raw = fs::read(&args[2])?;
    bundle::decode(&raw)?;
    if !raw.starts_with(b"RCPLAY04") {
        return Err("Expected mission".into());
    }
    let n = u32::from_le_bytes(raw[8..12].try_into()?) as usize;
    let mut mission: mission::Assets = serde_json::from_slice(&raw[12..12 + n])?;
    let animated = &raw[12 + n..];
    let n = u32::from_le_bytes(animated[8..12].try_into()?) as usize;
    let visual = &animated[12..12 + n];
    let base = &animated[12 + n..];
    let n = u32::from_le_bytes(base[8..12].try_into()?) as usize;
    let manifest: bundle::Manifest = serde_json::from_slice(&base[12..12 + n])?;
    let mut scene = rc_render::Scene::from_snapshot(&base[12 + n..])?;
    let mut assets = Assets::new(&[
        &root.join("Textures"),
        &root.join("StaticMeshes"),
        &root.join("System"),
    ])?;
    let map = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&map)?;
    let binding = rc_package::level::world_binding(&pkg, &map)?;
    let bsp = geometry::read_bsp(&pkg, &map, &pkg.exports[binding.model_export])?;
    let bsp_triangles: Vec<_> = bsp
        .triangles()?
        .into_iter()
        .filter(|t| bsp.surfaces[t.surface].flags & 0x81 == 0)
        .collect();
    let mut receipts = vec![];
    let mut errors = vec![];
    let mut cache: HashMap<String, Result<MaterialBinding, String>> = HashMap::new();
    let mut resolve = |path: &str| -> Result<(usize, f32, usize, [f32; 2]), String> {
        if let Some(result) = cache.get(path) {
            return result.clone();
        }
        let result = (|| {
            let mut channel = 0;
            let mut source = path.to_owned();
            let mut chain = vec![];
            let mut visited = std::collections::HashSet::new();
            for _ in 0..16 {
                if !visited.insert(source.to_lowercase()) {
                    return Err("Material cycle".into());
                }
                let (class, props) = assets.material_properties(&source)?;
                let field = match class.as_str() {
                    "Engine.TexCoordSource" => {
                        channel = match props
                            .values
                            .iter()
                            .find(|p| p.name == "SourceChannel")
                            .map(|p| &p.value)
                        {
                            Some(Value::Int(v)) if (0..8).contains(v) => *v as usize,
                            None => 0,
                            _ => return Err("Invalid UV source channel".into()),
                        };
                        "Material"
                    }
                    "Engine.Combiner" => {
                        let op = match props
                            .values
                            .iter()
                            .find(|p| p.name == "CombineOperation")
                            .map(|p| &p.value)
                        {
                            Some(Value::Byte(v)) => *v,
                            None => 0,
                            _ => return Err("Invalid combiner operation".into()),
                        };
                        match op {
                            1 => "Material2",
                            7 => "Mask",
                            _ => "Material1",
                        }
                    }
                    "Engine.Shader" => "Diffuse",
                    "Engine.FinalBlend" => "Material",
                    _ => break,
                };
                chain.push(class);
                let next = props
                    .values
                    .iter()
                    .find(|p| p.name == field)
                    .ok_or("Missing material input")?;
                source = match &next.value {
                    Value::Object { index, path: next } if *index != 0 => {
                        assets.qualify(source.split('.').next().unwrap(), next)
                    }
                    _ => return Err("Invalid material input".into()),
                };
            }
            let d = assets.diffuse_at_time(&source, 128, 0.)?;
            if d.uv_transform.is_some() {
                return Err("Animated UV transform needs separate host".into());
            }
            let size = texture_size(root, &d.source)?;
            let index = scene.textures.len();
            receipts.push(serde_json::json!({"material":path,"diffuse":d.source,"source_channel":channel,"original_size":size,"scale":d.scale,"chain":chain.into_iter().chain(d.chain).collect::<Vec<_>>()}));
            scene.textures.push(d.texture);
            Ok((index, d.scale, channel, size))
        })();
        cache.insert(path.to_owned(), result.clone());
        result
    };
    let mut bsp_bound = 0;
    for (i, t) in bsp_triangles.iter().enumerate() {
        if t.points != scene.triangles[i].points || t.surface != scene.triangles[i].surface {
            return Err("BSP prefix differs from original mission".into());
        }
        let s = &bsp.surfaces[t.surface];
        let path = if s.material > 0 {
            format!("geo_01a.{}", s.material_path)
        } else {
            s.material_path.clone()
        };
        match resolve(&path) {
            Ok((texture, scale, channel, size)) => {
                if channel != 0 {
                    return Err("BSP material requires unsupported UV stream".into());
                }
                let base = bsp.points[s.base as usize];
                let u = bsp.vectors[s.texture_u as usize];
                let v = bsp.vectors[s.texture_v as usize];
                let uv = t.points.map(|p| {
                    let d = std::array::from_fn::<_, 3, _>(|j| p[j] - base[j]);
                    [dot(d, u) / size[0] * scale, dot(d, v) / size[1] * scale]
                });
                scene.appearance[i] = Some(rc_render::FaceTexture { texture, uv });
                bsp_bound += 1;
            }
            Err(e) => errors.push(serde_json::json!({"kind":"bsp","material":path,"error":e})),
        }
    }
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read("analysis/scenes/geo_01a-textured/scene.json")?)?;
    let mut offset = bsp_triangles.len();
    let mut mesh_bound = 0;
    let mut skipped_fx = vec![];
    for instance in report["instances"]
        .as_array()
        .ok_or("Missing instance inventory")?
    {
        let count = instance["triangles"].as_u64().ok_or("No triangle count")? as usize;
        if scene.appearance[offset..offset + count]
            .iter()
            .any(Option::is_none)
        {
            let path = instance["mesh"].as_str().ok_or("No mesh")?;
            let (data, object) = package_object(root, "StaticMeshes", "usx", path)?;
            let p = read_package(&data)?;
            let e = p
                .exports
                .iter()
                .enumerate()
                .find_map(|(i, e)| {
                    p.object_path(i as i32 + 1)
                        .is_ok_and(|s| s.eq_ignore_ascii_case(&object))
                        .then_some(e)
                })
                .ok_or("No mesh export")?;
            let m = rc_package::mesh::read_static_mesh(&p, &data, e)?;
            let mut local = 0;
            for (section_index, section) in m.sections.iter().enumerate() {
                let material = m.materials[section_index]
                    .as_ref()
                    .ok_or("Missing section material")?;
                let material = assets_path(root, material, path.split('.').next().unwrap());
                let start = section.first_index as usize;
                for face in
                    m.indices[start..start + section.face_count as usize * 3].chunks_exact(3)
                {
                    let at = offset + local;
                    local += 1;
                    if scene.appearance[at].is_some() {
                        continue;
                    }
                    match resolve(&material) {
                        Ok((texture, scale, channel, _)) => {
                            let stream = m
                                .uv_streams
                                .get(channel)
                                .ok_or("Material UV channel missing on mesh")?;
                            let uv = face.try_into().map(|indices: [u16; 3]| {
                                indices.map(|i| stream[i as usize].map(|v| v * scale))
                            })?;
                            scene.appearance[at] = Some(rc_render::FaceTexture { texture, uv });
                            mesh_bound += 1;
                        }
                        Err(e) => {
                            if e == "Unsupported material class Engine.HsFalloff" {
                                skipped_fx.push(at);
                            } else {
                                errors.push(serde_json::json!({"kind":"mesh","actor":instance["actor"],"material":material,"error":e}));
                            }
                        }
                    }
                }
            }
            if local != count {
                return Err("Mesh triangle inventory changed".into());
            }
        }
        offset += count;
    }
    // Volumetric falloff effects have no opaque diffuse surface. Never draw them
    // as solid diagnostic geometry while their shader is unsupported.
    for &i in skipped_fx.iter().rev() {
        scene.triangles.remove(i);
        scene.appearance.remove(i);
    }
    let catalog = defaults::load(root)?;
    let sound = catalog.resolve("CTInventory.DC15s", "FireSound", 0)?;
    let path = match &sound.value {
        Value::Object { path, .. } => path.clone(),
        _ => return Err("DC15s FireSound is not an object".into()),
    };
    let (data, object) = package_object(root, "Sounds", "uax", &path)?;
    let p = read_package(&data)?;
    let e = p
        .exports
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            p.object_path(i as i32 + 1)
                .is_ok_and(|s| s.eq_ignore_ascii_case(&object))
                .then_some(e)
        })
        .ok_or("Missing original weapon sound")?;
    let payload = p.payload(&data, e)?;
    let at = payload
        .windows(4)
        .position(|v| v == b"RIFF")
        .ok_or("Missing WAV")?;
    let len = u32::from_le_bytes(payload[at + 4..at + 8].try_into()?) as usize + 8;
    let wave = payload.get(at..at + len).ok_or("Short WAV")?.to_vec();
    rc_game::combat::validate_wave(&wave)?;
    mission.weapon_fire_sound = Some(mission.sounds.len());
    mission.sounds.push(rc_game::combat::Sound {
        source: path.clone(),
        wave: wave.clone(),
    });
    let base = bundle::encode(&manifest, &scene)?;
    let mut animated = b"RCPLAY02".to_vec();
    animated.extend_from_slice(&(visual.len() as u32).to_le_bytes());
    animated.extend(visual);
    animated.extend(base);
    let metadata = serde_json::to_vec(&mission)?;
    let mut result = b"RCPLAY04".to_vec();
    result.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    result.extend(metadata);
    result.extend(animated);
    bundle::decode(&result)?;
    if !errors.is_empty() || scene.appearance.iter().any(Option::is_none) {
        return Err("Surface texture audit failed; refusing incomplete repaired delivery".into());
    }
    fs::write(
        out.join("mission-receipt.json"),
        serde_json::to_vec_pretty(&mission)?,
    )?;
    fs::write(out.join("geo_01a-crash.rcplay"), result)?;
    fs::write(out.join("dc15s-fire.wav"), wave)?;
    fs::write(
        out.join("repair-receipt.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"untextured_surface_triangles":scene.appearance.iter().filter(|f|f.is_none()).count(),"bsp_triangles_textured":bsp_bound,"mesh_triangles_repaired":mesh_bound,"volumetric_effect_triangles_omitted":skipped_fx.len(),"remaining_errors":errors,"materials":receipts,"fire_sound_default":sound,"fire_sound":path,"fire_sound_index":mission.weapon_fire_sound,"scope":"Original base diffuse and selected UV streams; no full material shader/lighting parity. Falloff volumes omitted rather than opaque placeholders."}),
        )?,
    )?;
    println!("Textured {bsp_bound} BSP and {mesh_bound} mesh triangles; omitted {} falloff effect triangles; {} errors; DC15s={path}",skipped_fx.len(),errors.len());
    Ok(())
}
fn assets_path(root: &Path, path: &str, package: &str) -> String {
    let first = path.split('.').next().unwrap();
    let qualified = [
        ("Textures", "utx"),
        ("StaticMeshes", "usx"),
        ("System", "u"),
    ]
    .iter()
    .any(|(folder, ext)| {
        fs::read_dir(root.join(folder)).is_ok_and(|entries| {
            entries.filter_map(Result::ok).any(|e| {
                e.file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&format!("{first}.{ext}"))
            })
        })
    });
    if qualified {
        path.into()
    } else {
        format!("{package}.{path}")
    }
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
