//! Shared original mesh and PCM extraction for selected mission extensions.
use crate::assets::Assets as Resolver;
use rc_game::bundle;
use rc_package::{hull::HullSet, mesh_query::ActorTransform, properties::Value, read_package};
use std::{fs, path::Path};
pub fn collider(name: String, h: &HullSet) -> bundle::Collider {
    bundle::Collider {
        name,
        hulls: h
            .hulls
            .iter()
            .map(|h| bundle::HullData {
                offset: h.offset,
                planes: h.planes.clone(),
                bounds: h.bounds,
            })
            .collect(),
    }
}
pub fn sound(
    root: &Path,
    path: &str,
) -> Result<(rc_game::combat::Sound, f64), Box<dyn std::error::Error>> {
    let (package, object) = path.split_once('.').ok_or("Bad audio path")?;
    let bytes = fs::read(root.join("Sounds").join(format!("{package}.uax")))?;
    let p = read_package(&bytes)?;
    let e = p
        .exports
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            p.object_path(i as i32 + 1)
                .is_ok_and(|s| s.eq_ignore_ascii_case(object))
                .then_some(e)
        })
        .ok_or("Missing sound")?;
    let payload = p.payload(&bytes, e)?;
    let at = payload
        .windows(4)
        .position(|w| w == b"RIFF")
        .ok_or("No RIFF")?;
    let n = u32::from_le_bytes(payload[at + 4..at + 8].try_into()?) as usize + 8;
    let wave = payload.get(at..at + n).ok_or("Truncated wave")?.to_vec();
    rc_game::combat::validate_wave(&wave)?;
    let (mut at, mut rate, mut size) = (12, None, None);
    while at + 8 <= wave.len() {
        let n = u32::from_le_bytes(wave[at + 4..at + 8].try_into()?) as usize;
        let chunk = wave.get(at + 8..at + 8 + n).ok_or("Short WAV chunk")?;
        match &wave[at..at + 4] {
            b"fmt " => rate = Some(u32::from_le_bytes(chunk[8..12].try_into()?) as f64),
            b"data" => size = Some(n as f64),
            _ => {}
        }
        at += 8 + n + (n & 1);
    }
    Ok((
        rc_game::combat::Sound {
            source: path.into(),
            wave,
        },
        size.ok_or("No samples")? / rate.ok_or("No rate")?,
    ))
}
/// Existing bounded audio policy: first original SoundMultiple variant, with provenance.
pub fn sound_variant(
    root: &Path,
    path: &str,
    receipts: &mut Vec<serde_json::Value>,
) -> Result<(rc_game::combat::Sound, f64), Box<dyn std::error::Error>> {
    let mut current = path.to_owned();
    for _ in 0..5 {
        let (package, object) = current.split_once('.').ok_or("Invalid audio path")?;
        let data = fs::read(root.join("Sounds").join(format!("{package}.uax")))?;
        let p = read_package(&data)?;
        let e = p
            .exports
            .iter()
            .enumerate()
            .find_map(|(i, e)| {
                p.object_path(i as i32 + 1)
                    .is_ok_and(|s| s.eq_ignore_ascii_case(object))
                    .then_some(e)
            })
            .ok_or("Missing sound variant")?;
        if p.object_path(e.class)? != "Engine.SoundMultiple" {
            let (mut sound, seconds) = sound(root, &current)?;
            sound.source = path.into();
            return Ok((sound, seconds));
        }
        let payload = p.payload(&data, e)?;
        let props = rc_package::properties::read(&p, &data, e)?;
        let mut pos = props.native_offset;
        let index = |pos: &mut usize| -> Result<i32, String> {
            let first = *payload.get(*pos).ok_or("Truncated sound group")?;
            *pos += 1;
            let mut v = u64::from(first & 63);
            let mut more = first & 64 != 0;
            let mut shift = 6;
            for i in 0..4 {
                if !more {
                    break;
                }
                let b = *payload.get(*pos).ok_or("Truncated sound group")?;
                *pos += 1;
                if i == 3 && b & 224 != 0 {
                    return Err("Sound group overflow".into());
                }
                v |= u64::from(b & if i == 3 { 31 } else { 127 }) << shift;
                more = i != 3 && b & 128 != 0;
                shift += 7;
            }
            i32::try_from(if first & 128 != 0 {
                -(v as i64)
            } else {
                v as i64
            })
            .map_err(|e| e.to_string())
        };
        let count = index(&mut pos)?;
        if !(1..=64).contains(&count) {
            return Err("Invalid sound group count".into());
        }
        let mut children = vec![];
        for _ in 0..count {
            let id = index(&mut pos)?;
            let object = p.object_path(id)?;
            children.push(if id > 0 {
                format!("{package}.{object}")
            } else {
                object
            });
        }
        if payload.len() != pos + 1 {
            return Err("Unsupported original sound group layout".into());
        }
        receipts.push(serde_json::json!({"group":current,"children":children,"selection_style":payload[pos],"adapter":"first original variant; no native random/likelihood host"}));
        current = children[0].clone();
    }
    Err("Sound group recursion".into())
}
pub fn mesh_scene(
    root: &Path,
    path: &str,
    t: &ActorTransform,
    catalog: &rc_package::defaults::Catalog,
    receipts: &mut Vec<serde_json::Value>,
) -> Result<rc_render::Scene, Box<dyn std::error::Error>> {
    mesh_scene_skinned(root, path, t, catalog, receipts, None)
}
/// Selected actor transition material; original mesh material remains in receipts.
pub fn mesh_scene_skinned(
    root: &Path,
    path: &str,
    t: &ActorTransform,
    catalog: &rc_package::defaults::Catalog,
    receipts: &mut Vec<serde_json::Value>,
    skin: Option<&str>,
) -> Result<rc_render::Scene, Box<dyn std::error::Error>> {
    let (package, object) = path.split_once('.').ok_or("Bad mesh path")?;
    let bytes = fs::read(root.join("StaticMeshes").join(format!("{package}.usx")))?;
    let p = read_package(&bytes)?;
    let e = p
        .exports
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            p.object_path(i as i32 + 1)
                .is_ok_and(|s| s.eq_ignore_ascii_case(object))
                .then_some(e)
        })
        .ok_or("Missing wall mesh")?;
    let mesh = rc_package::mesh::read_static_mesh(&p, &bytes, e)?;
    let mut triangles = mesh.triangles()?;
    for tri in &mut triangles {
        tri.points = tri.points.map(|p| {
            rc_package::mesh::transform_point(p, t.location, t.rotation, t.scale, t.pivot)
        });
    }
    let mut scene = rc_render::Scene::new(
        triangles
            .into_iter()
            .map(|t| rc_package::geometry::Triangle {
                points: t.points,
                surface: t.surface,
            })
            .collect(),
    )?;
    let mut resolver = Resolver::new(&[
        &root.join("Textures"),
        &root.join("StaticMeshes"),
        &root.join("System"),
    ])?;
    let mut bindings = vec![];
    for m in &mesh.materials {
        let original = resolver.qualify(package, m.as_ref().ok_or("No wall material")?);
        let material = skin.map_or_else(|| original.clone(), str::to_owned);
        let mut source = material.clone();
        let mut channel = 0;
        let mut chain = vec![];
        for _ in 0..16 {
            let (class, props) = resolver.material_properties(&source)?;
            chain.push(source.clone());
            let field = match class.as_str() {
                "Engine.TexCoordSource" => {
                    channel = match catalog
                        .instance(&class, &props, "SourceChannel", 0, &source)?
                        .value
                    {
                        Value::Int(v) if v >= 0 => v as usize,
                        _ => return Err("Bad UV channel".into()),
                    };
                    "Material"
                }
                "Engine.Shader" => {
                    // The original DC17m sniper scope has no Diffuse; its
                    // SelfIllumination and Opacity reference the same texture.
                    if matches!(
                        catalog
                            .instance(&class, &props, "Diffuse", 0, &source)?
                            .value,
                        Value::Object { index: 0, .. }
                    ) {
                        "SelfIllumination"
                    } else {
                        "Diffuse"
                    }
                }
                "Engine.FinalBlend" => "Material",
                "Engine.Combiner" => match catalog
                    .instance(&class, &props, "CombineOperation", 0, &source)?
                    .value
                {
                    Value::Byte(1) => "Material2",
                    // Selected Arena multiply wrappers put their isolated lightmap
                    // in Material1 and the original surface/UV source in Material2.
                    Value::Byte(2) => "Material2",
                    Value::Byte(7) => "Mask",
                    _ => "Material1",
                },
                _ => break,
            };
            let path = match catalog.instance(&class, &props, field, 0, &source)?.value {
                Value::Object { path, index } if index != 0 => path,
                _ => return Err("Missing wall material layer".into()),
            };
            let package = source.split_once('.').ok_or("Bad layer path")?.0;
            source = resolver.qualify(package, &path);
        }
        let d = resolver.diffuse_layer_at_time(&source, &material, 256, 0.)?;
        let opaque = &d.opacity;
        receipts.push(serde_json::json!({"mesh":path,"material":original,"diffuse":d.source,"channel":channel,"opaque":opaque,"chain":chain}));
        bindings.push((channel, d.scale));
        scene.textures.push(d.texture);
    }
    let mut face = 0;
    for section in &mesh.sections {
        let first = section.first_index as usize;
        for ids in mesh.indices[first..first + section.face_count as usize * 3].chunks_exact(3) {
            let material = scene.triangles[face].surface;
            let (channel, scale) = bindings[material];
            let uv = mesh
                .uv_streams
                .get(channel)
                .ok_or("Missing source UV channel")?;
            scene.appearance.push(Some(rc_render::FaceTexture {
                texture: material,
                uv: std::array::from_fn(|i| uv[ids[i] as usize].map(|v| v * scale)),
            }));
            face += 1;
        }
    }
    if face != scene.triangles.len() {
        return Err("Incomplete wall textures".into());
    }
    Ok(scene)
}
