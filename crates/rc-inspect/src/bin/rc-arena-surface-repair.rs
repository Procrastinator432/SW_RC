//! Correct selected arena lightmap-only previews to their original base surface.
//! Lighting multiplication and vertex-mask blends remain explicit preview limits.
use rc_inspect::assets::Assets;
use rc_package::{properties::Value, read_package};
use std::{collections::HashMap, env, fs, path::Path};
type Binding = (usize, usize, [f32; 2], [f32; 2]);
fn key(points: [[f32; 3]; 3], _surface: usize) -> [u32; 9] {
    // The scene exporter adds an instance color offset to surface slots. Exact
    // ordered world-space vertex bits remain stable after actor/FX removal.
    let mut k = [0; 9];
    for (i, v) in points.into_iter().flatten().enumerate() {
        k[i] = v.to_bits();
    }
    k
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 6 {
        return Err("usage: rc-arena-surface-repair GAME_DATA SCENE INVENTORY AUDIT OUTPUT".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[5]);
    fs::create_dir_all(out)?;
    let raw = fs::read(&args[2])?;
    let old_game = rc_game::bundle::decode(&raw)?;
    let mut offset = 0;
    while &raw[offset..offset + 8] != b"RCPLAY01" {
        if &raw[offset..offset + 8] != b"RCPLAY04" && &raw[offset..offset + 8] != b"RCPLAY02" {
            return Err("Unsupported bundle layer".into());
        }
        let n = u32::from_le_bytes(raw[offset + 8..offset + 12].try_into()?) as usize;
        offset += 12 + n;
    }
    offset += 12 + u32::from_le_bytes(raw[offset + 8..offset + 12].try_into()?) as usize;
    let mut scene = rc_render::Scene::from_snapshot(&raw[offset..])?;
    let inventory: serde_json::Value = serde_json::from_slice(&fs::read(&args[3])?)?;
    let audit: Vec<serde_json::Value> = serde_json::from_slice(&fs::read(&args[4])?)?;
    let mut assets = Assets::new(&[
        &root.join("Textures"),
        &root.join("StaticMeshes"),
        &root.join("System"),
    ])?;
    let catalog = rc_inspect::defaults::load(root)?;
    let mut bindings: HashMap<String, Binding> = HashMap::new();
    let mut receipts = vec![];
    for row in audit {
        let path = row["material"].as_str().ok_or("No material")?;
        let (class, props) = assets.material_properties(path)?;
        let op = props
            .values
            .iter()
            .find(|p| p.name == "CombineOperation")
            .map(|p| &p.value);
        if class != "Engine.Combiner" || !matches!(op, Some(Value::Byte(2))) {
            continue;
        }
        let lightmap = row["inputs"]
            .as_array()
            .ok_or("No inputs")?
            .iter()
            .find(|i| i["field"] == "Material1")
            .ok_or("No lightmap")?;
        if lightmap["class"] != "Engine.Texture"
            || !lightmap["path"]
                .as_str()
                .ok_or("No lightmap path")?
                .to_ascii_lowercase()
                .ends_with("lm")
        {
            return Err("Unsupported arena multiply layout".into());
        }
        let mut source = row["inputs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["field"] == "Material2")
            .ok_or("No base surface")?["path"]
            .as_str()
            .ok_or("No base path")?
            .to_owned();
        let mut channel = 0;
        let mut scale = [1.; 2];
        let translation = [0.; 2];
        let mut chain = vec![];
        let mut finished = false;
        for _ in 0..16 {
            let (class, p) = assets.material_properties(&source)?;
            chain.push(source.clone());
            let field = match class.as_str() {
                "Engine.Texture" => {
                    finished = true;
                    break;
                }
                "Engine.TexCoordSource" => {
                    channel = match catalog
                        .instance(&class, &p, "SourceChannel", 0, &source)?
                        .value
                    {
                        Value::Int(v) if (0..8).contains(&v) => v as usize,
                        _ => return Err("Invalid UV source".into()),
                    };
                    "Material"
                }
                "Engine.TexScaler" => {
                    let float = |name| -> Result<f32, String> {
                        match catalog.instance(&class, &p, name, 0, &source)?.value {
                            Value::Float(v) if v.is_finite() => Ok(v),
                            _ => Err("Invalid UV scaler".into()),
                        }
                    };
                    let uv_scale = [float("UScale")?, float("VScale")?];
                    if uv_scale.contains(&0.) {
                        return Err("Zero UV scale".into());
                    }
                    scale = std::array::from_fn(|i| scale[i] / uv_scale[i]);
                    // Selected original UpperW_Scalar offsets are zero. Avoid
                    // guessing dimensions or composition for other modifiers.
                    if float("UOffset")? != 0. || float("VOffset")? != 0. {
                        return Err("Nonzero scaler offset needs a separate adapter".into());
                    }
                    "Material"
                }
                "Engine.Combiner" => {
                    let operation = match catalog
                        .instance(&class, &p, "CombineOperation", 0, &source)?
                        .value
                    {
                        Value::Byte(v) => v,
                        _ => return Err("No combine operation".into()),
                    };
                    match operation {
                        0 | 5 => "Material1",
                        1 => "Material2",
                        _ => return Err("Nested combine operation needs separate adapter".into()),
                    }
                }
                "Engine.Shader" => "Diffuse",
                _ => return Err(format!("Unsupported arena base material {class}").into()),
            };
            source = match p.values.iter().find(|p| p.name == field).map(|p| &p.value) {
                Some(Value::Object { index, path: next }) if *index != 0 => {
                    assets.qualify(source.split('.').next().unwrap(), next)
                }
                _ => return Err("Missing arena surface input".into()),
            };
        }
        if !finished {
            return Err("Arena material cycle/depth limit".into());
        }
        let d = assets.diffuse_at_time(&source, 128, 0.)?;
        if d.uv_transform.is_some() {
            return Err("Unexpected animated arena surface".into());
        }
        for v in &mut scale {
            *v *= d.scale;
        }
        let texture = scene.textures.len();
        scene.textures.push(d.texture);
        bindings.insert(path.to_owned(), (texture, channel, scale, translation));
        receipts.push(serde_json::json!({"material":path,"lightmap":lightmap["path"],"base_surface":d.source,"source_channel":channel,"uv_scale":scale,"chain":chain,"mode":"base surface preview; lightmap multiplication and vertex blend omitted"}));
    }
    let mut indices: HashMap<[u32; 9], Vec<usize>> = HashMap::new();
    for (i, t) in scene.triangles.iter().enumerate() {
        indices.entry(key(t.points, t.surface)).or_default().push(i);
    }
    let mut changed = std::collections::BTreeSet::new();
    let mut meshes = HashMap::new();
    let mut actors = vec![];
    for instance in inventory["instances"].as_array().ok_or("No instances")? {
        let path = instance["mesh"].as_str().ok_or("No instance mesh")?;
        let (package, object) = path.split_once('.').ok_or("No mesh package")?;
        if !meshes.contains_key(path) {
            let bytes = fs::read(root.join("StaticMeshes").join(format!("{package}.usx")))?;
            let pkg = read_package(&bytes)?;
            let e = pkg
                .exports
                .iter()
                .enumerate()
                .find(|(i, _)| {
                    pkg.object_path(*i as i32 + 1)
                        .is_ok_and(|p| p.eq_ignore_ascii_case(object))
                })
                .ok_or("Missing mesh")?
                .1;
            meshes.insert(
                path.to_owned(),
                rc_package::mesh::read_static_mesh(&pkg, &bytes, e)?,
            );
        }
        let m = &meshes[path];
        let triangles = m.triangles()?;
        let location = serde_json::from_value(instance["location"].clone())?;
        let rotation = serde_json::from_value(instance["rotation"].clone())?;
        let scale = serde_json::from_value(instance["scale"].clone())?;
        let pivot = serde_json::from_value(instance["pre_pivot"].clone())?;
        let mut face = 0;
        let mut count = 0;
        for (slot, section) in m.sections.iter().enumerate() {
            let material = m.materials[slot]
                .as_ref()
                .map(|s| assets.qualify(package, s));
            let binding = material.as_ref().and_then(|s| bindings.get(s));
            let start = section.first_index as usize;
            for vertices in
                m.indices[start..start + section.face_count as usize * 3].chunks_exact(3)
            {
                let t = &triangles[face];
                face += 1;
                let Some(&(texture, channel, uv_scale, uv_offset)) = binding else {
                    continue;
                };
                let points = t.points.map(|p| {
                    rc_package::mesh::transform_point(p, location, rotation, scale, pivot)
                });
                let Some(matched) = indices.get(&key(points, t.surface)) else {
                    continue;
                }; // Removed dynamic instances are not in this static snapshot.
                let stream = m
                    .uv_streams
                    .get(channel)
                    .ok_or("Arena mesh lacks source UV channel")?;
                let uv = std::array::from_fn(|i| {
                    std::array::from_fn(|axis| {
                        stream[vertices[i] as usize][axis] * uv_scale[axis] + uv_offset[axis]
                    })
                });
                for &at in matched {
                    if changed.contains(&at)
                        && scene.appearance[at].is_some_and(|f| f.texture != texture || f.uv != uv)
                    {
                        return Err("Ambiguous coincident arena material binding".into());
                    }
                    scene.appearance[at] = Some(rc_render::FaceTexture { texture, uv });
                    if changed.insert(at) {
                        count += 1;
                    }
                }
            }
        }
        if count > 0 {
            actors.push(serde_json::json!({"actor":instance["actor"],"faces":count}));
        }
    }
    if changed.is_empty() {
        return Err("No arena surfaces corrected".into());
    }
    let snapshot = scene.snapshot()?;
    let mut result = raw[..offset].to_vec();
    result.extend(snapshot);
    let new_game = rc_game::bundle::decode(&result)?;
    if serde_json::to_vec(&old_game.state)? != serde_json::to_vec(&new_game.state)? {
        return Err("Texture repair changed initial game state".into());
    }
    fs::write(out.join("geo_01a-grenades-arena.rcplay"), result)?;
    fs::write(
        out.join("arena-repair-validation.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "passed":true,"rebound_faces":changed.len(),"layered_materials":bindings.len(),"materials":receipts,"actors":actors,
            "untextured_faces":scene.appearance.iter().filter(|f|f.is_none()).count(),"initial_game_state_unchanged":true,
            "bundle_metadata_byte_exact":true,"scope":"Static scene surface-only repair; original base texture/UV channel replaces isolated lightmap. No full shader/lighting multiplication or vertex mask parity. Compatible with existing 0.21.0 app; objective3 remains unintegrated."
        }))?,
    )?;
    println!(
        "Rebound {} arena faces across {} layered materials",
        changed.len(),
        bindings.len()
    );
    Ok(())
}
