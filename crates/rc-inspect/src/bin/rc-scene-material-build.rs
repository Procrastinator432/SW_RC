//! Refresh legacy scene alpha from original per-face material identities.
//! Geometry, UVs, collision and scripts are preserved; unresolved faces stay unchanged.
use rc_inspect::{assets::Assets, defaults};
use rc_package::{
    geometry, mesh,
    properties::{self, Value},
    read_package,
};
use std::{collections::HashMap, env, fs, path::Path};

type Key = [u32; 9];
fn key(t: &geometry::Triangle) -> Key {
    // Historical world builders replace surface IDs with diagnostic colour IDs.
    // Conflicting coincident faces are rejected by insert(), rather than guessed.
    std::array::from_fn(|i| t.points[i / 3][i % 3].to_bits())
}
#[derive(Clone, PartialEq)]
struct Proof {
    material: String,
    opaque: bool,
}
fn insert(index: &mut HashMap<Key, Option<Proof>>, t: &geometry::Triangle, proof: Proof) {
    index
        .entry(key(t))
        .and_modify(|old| {
            if old.as_ref() != Some(&proof) {
                *old = None;
            }
        })
        .or_insert(Some(proof));
}
fn repair(scene: &mut rc_render::Scene, index: &HashMap<Key, Option<Proof>>) -> serde_json::Value {
    let mut exclusive: HashMap<usize, bool> = HashMap::new();
    for (t, face) in scene.triangles.iter().zip(&scene.appearance) {
        if let Some(face) = face {
            let opaque = matches!(index.get(&key(t)), Some(Some(p)) if p.opaque);
            *exclusive.entry(face.texture).or_insert(true) &= opaque;
        }
    }
    let mut copies = HashMap::new();
    let mut changed = 0;
    let mut matched = 0;
    let mut materials: HashMap<String, usize> = HashMap::new();
    for (t, f) in scene.triangles.iter().zip(&mut scene.appearance) {
        let Some(Some(proof)) = index.get(&key(t)) else {
            continue;
        };
        matched += 1;
        let Some(face) = f else {
            continue;
        };
        if !proof.opaque
            || scene.textures[face.texture]
                .pixels
                .iter()
                .all(|p| p >> 24 == 255)
        {
            continue;
        }
        let old = face.texture;
        face.texture = *copies.entry(old).or_insert_with(|| {
            if exclusive.get(&old) == Some(&true) {
                return old;
            }
            let mut texture = scene.textures[old].clone();
            for p in &mut texture.pixels {
                *p |= 0xff000000;
            }
            let at = scene.textures.len();
            scene.textures.push(texture);
            at
        });
        changed += 1;
        *materials.entry(proof.material.clone()).or_default() += 1;
    }
    for (&old, &new) in &copies {
        if old == new {
            for p in &mut scene.textures[old].pixels {
                *p |= 0xff000000;
            }
        }
    }
    serde_json::json!({"matched_faces":matched,"changed_faces":changed,"private_textures":copies.iter().filter(|(a,b)| a!=b).count(),"normalized_textures":copies.len(),"materials":materials})
}
fn snapshots(
    value: &mut serde_json::Value,
    index: &HashMap<Key, Option<Proof>>,
    audits: &mut Vec<serde_json::Value>,
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    match value {
        serde_json::Value::Object(object) => {
            for (k, v) in object {
                let path = format!("{path}/{k}");
                if k.ends_with("snapshot") && v.is_array() {
                    let bytes: Vec<u8> = serde_json::from_value(v.clone())?;
                    if bytes.starts_with(b"RCSC") {
                        let mut scene = rc_render::Scene::from_snapshot(&bytes)?;
                        let report = repair(&mut scene, index);
                        if report["changed_faces"].as_u64().unwrap() != 0 {
                            *v = serde_json::to_value(scene.snapshot()?)?;
                        }
                        audits.push(serde_json::json!({"path":path,"repair":report}));
                    }
                } else if !["pixels", "wave"].contains(&k.as_str()) {
                    snapshots(v, index, audits, &path)?;
                }
            }
        }
        serde_json::Value::Array(values) => {
            for (i, v) in values.iter_mut().enumerate() {
                if v.is_object() || v.is_array() {
                    snapshots(v, index, audits, &format!("{path}/{i}"))?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 5 {
        return Err("usage: rc-scene-material-build GAME_DATA MAP_NAME SCENE OUT".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[4]);
    fs::create_dir_all(out)?;
    let bytes = fs::read(root.join("Maps").join(format!("{}.ctm", args[2])))?;
    let pkg = read_package(&bytes)?;
    let catalog = defaults::load(root)?;
    let mut assets = Assets::new(&[
        &root.join("Textures"),
        &root.join("StaticMeshes"),
        &root.join("System"),
    ])?;
    let mut index = HashMap::new();
    let binding = rc_package::level::world_binding(&pkg, &bytes)?;
    let bsp = geometry::read_bsp(&pkg, &bytes, &pkg.exports[binding.model_export])?;
    for t in bsp.triangles()? {
        let surface = &bsp.surfaces[t.surface];
        let material = if surface.material > 0 {
            format!("{}.{}", args[2], surface.material_path)
        } else {
            surface.material_path.clone()
        };
        let opaque = surface.flags == 0 && assets.opaque(&material) == Ok(true);
        insert(&mut index, &t, Proof { material, opaque });
    }
    let mut meshes = HashMap::new();
    let mut errors = vec![];
    for e in &pkg.exports {
        let class = pkg.object_path(e.class)?;
        if !catalog
            .derives_from(&class, "Engine.Actor")
            .unwrap_or(false)
        {
            continue;
        }
        let props = properties::read(&pkg, &bytes, e)?;
        let get = |name| {
            catalog
                .instance(&class, &props, name, 0, &args[2])
                .map(|v| v.value)
        };
        let Value::Object {
            index: mesh_index,
            path,
        } = get("StaticMesh")?
        else {
            continue;
        };
        if mesh_index == 0 {
            continue;
        }
        let qualified = if mesh_index > 0 {
            format!("{}.{}", args[2], path)
        } else {
            path.clone()
        };
        if !meshes.contains_key(&qualified) {
            let loaded = (|| -> Result<mesh::StaticMesh, String> {
                if mesh_index > 0 {
                    return mesh::read_static_mesh(
                        &pkg,
                        &bytes,
                        &pkg.exports[mesh_index as usize - 1],
                    );
                }
                let (package, object) = path.split_once('.').ok_or("Invalid mesh path")?;
                let data = fs::read(root.join("StaticMeshes").join(format!("{package}.usx")))
                    .map_err(|e| e.to_string())?;
                let p = read_package(&data)?;
                let export = p
                    .exports
                    .iter()
                    .enumerate()
                    .find_map(|(i, e)| {
                        p.object_path(i as i32 + 1)
                            .is_ok_and(|s| s.eq_ignore_ascii_case(object))
                            .then_some(e)
                    })
                    .ok_or("Missing mesh")?;
                mesh::read_static_mesh(&p, &data, export)
            })();
            meshes.insert(qualified.clone(), loaded);
        }
        let m = match &meshes[&qualified] {
            Ok(m) => m,
            Err(error) => {
                errors.push(serde_json::json!({"mesh":qualified,"error":error}));
                continue;
            }
        };
        let vector = |field| match get(field)? {
            Value::Vector(v) => Ok(v),
            _ => Err(format!("Invalid {field}")),
        };
        let location = vector("Location")?;
        let pivot = vector("PrePivot")?;
        let Value::Rotator(rotation) = get("Rotation")? else {
            return Err("Invalid rotation".into());
        };
        let Value::Float(scale) = get("DrawScale")? else {
            return Err("Invalid draw scale".into());
        };
        let scale = vector("DrawScale3D")?.map(|v| v * scale);
        for mut t in m.triangles()? {
            let Some(Some(material)) = m.materials.get(t.surface) else {
                continue;
            };
            let package = qualified.split_once('.').unwrap().0;
            let mut material = assets.qualify(package, material);
            // Per-section actor skins override the mesh material in the original.
            if let Ok(skin) = catalog.instance(&class, &props, "Skins", t.surface as u32, &args[2])
            {
                if let Value::Object { index, path } = skin.value {
                    if index != 0 {
                        material = if skin.stage == "instance_delta" && index > 0 {
                            format!("{}.{}", args[2], path)
                        } else {
                            path
                        };
                    }
                }
            }
            t.points = t
                .points
                .map(|p| mesh::transform_point(p, location, rotation, scale, pivot));
            let triangle = geometry::Triangle {
                points: t.points,
                surface: t.surface,
            };
            let opaque = assets.opaque(&material) == Ok(true);
            insert(&mut index, &triangle, Proof { material, opaque });
        }
    }
    let raw = fs::read(&args[3])?;
    rc_game::bundle::decode(&raw)?;
    let mut at = 0;
    let mut result = vec![];
    let mut audits = vec![];
    for _ in 0..16 {
        let header = raw.get(at..at + 12).ok_or("Truncated layer")?;
        let size = u32::from_le_bytes(header[8..12].try_into()?) as usize;
        let end = at.checked_add(12 + size).ok_or("Layer overflow")?;
        let mut metadata: serde_json::Value =
            serde_json::from_slice(raw.get(at + 12..end).ok_or("Truncated metadata")?)?;
        snapshots(
            &mut metadata,
            &index,
            &mut audits,
            std::str::from_utf8(&header[..8])?,
        )?;
        let encoded = serde_json::to_vec(&metadata)?;
        result.extend_from_slice(&header[..8]);
        result.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
        result.extend(encoded);
        at = end;
        if &header[..8] == b"RCPLAY01" {
            break;
        }
    }
    let mut scene = rc_render::Scene::from_snapshot(&raw[at..])?;
    let world = repair(&mut scene, &index);
    result.extend(scene.snapshot()?);
    rc_game::bundle::decode(&result)?;
    fs::write(out.join(format!("{}.rcplay", args[2])), result)?;
    fs::write(
        out.join("material-refresh.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"map":args[2],"world":world,"snapshots":audits,"material_decisions":assets.opacity_decisions(),"unresolved_meshes":errors,"policy":"Exact original world-space face identity; original defaults and actor skins; shared textures copied when any user lacks opaque proof; conflicting, masked, blended and unknown bindings preserved; geometry/UV/collision unchanged"}),
        )?,
    )?;
    println!("{}: {world}", args[2]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_masked_texture_and_ambiguous_faces_are_preserved() {
        let triangle = |x| geometry::Triangle {
            points: [[x, 0., 0.], [x + 1., 0., 0.], [x, 1., 0.]],
            surface: 999,
        };
        let mut scene = rc_render::Scene::new(vec![triangle(0.), triangle(2.)]).unwrap();
        scene.textures.push(rc_render::Texture {
            width: 1,
            height: 1,
            pixels: vec![0x00123456],
        });
        let face = rc_render::FaceTexture {
            texture: 0,
            uv: [[0., 0.], [1., 0.], [0., 1.]],
        };
        scene.appearance = vec![Some(face), Some(face)];
        let mut index = HashMap::new();
        insert(
            &mut index,
            &triangle(0.),
            Proof {
                material: "opaque".into(),
                opaque: true,
            },
        );
        insert(
            &mut index,
            &triangle(2.),
            Proof {
                material: "masked".into(),
                opaque: false,
            },
        );
        let report = repair(&mut scene, &index);
        assert_eq!(report["changed_faces"], 1);
        assert_eq!(scene.textures[0].pixels, vec![0x00123456]);
        assert_eq!(scene.textures[1].pixels, vec![0xff123456]);
        assert_eq!(scene.appearance[1].unwrap().texture, 0);
        assert_eq!(scene.appearance[0].unwrap().uv, face.uv);
        insert(
            &mut index,
            &triangle(0.),
            Proof {
                material: "conflicting".into(),
                opaque: true,
            },
        );
        assert_eq!(index[&key(&triangle(0.))].as_ref().map(|p| p.opaque), None);
    }
    #[test]
    fn exclusively_opaque_texture_does_not_expand_snapshot() {
        let t = geometry::Triangle {
            points: [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]],
            surface: 0,
        };
        let mut index = HashMap::new();
        insert(
            &mut index,
            &t,
            Proof {
                material: "opaque".into(),
                opaque: true,
            },
        );
        let mut scene = rc_render::Scene::new(vec![t]).unwrap();
        scene.textures.push(rc_render::Texture {
            width: 1,
            height: 1,
            pixels: vec![0x00123456],
        });
        scene.appearance = vec![Some(rc_render::FaceTexture {
            texture: 0,
            uv: [[0.; 2]; 3],
        })];
        let before = scene.snapshot().unwrap().len();
        let report = repair(&mut scene, &index);
        assert_eq!(report["changed_faces"], 1);
        assert_eq!(scene.snapshot().unwrap().len(), before);
        assert_eq!(scene.textures[0].pixels[0], 0xff123456);
    }
}
