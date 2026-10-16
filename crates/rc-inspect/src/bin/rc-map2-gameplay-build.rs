use std::{env, fs, path::Path};
fn copy<T: serde::Serialize, U: serde::de::DeserializeOwned>(
    v: &T,
) -> Result<U, serde_json::Error> {
    serde_json::from_value(serde_json::to_value(v)?)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args().collect();
    if a.len() != 5 {
        return Err("usage: rc-map2-gameplay-build GAME_DATA WORLD FIRST_MAP OUT".into());
    }
    let root = Path::new(&a[1]);
    let out = Path::new(&a[4]);
    fs::create_dir_all(out)?;
    let mut game =
        rc_inspect::play_scene::load(root, &root.join("Maps/geo_01b.ctm"), Path::new(&a[2]))?;
    let mut gameplay: rc_game::map_gameplay::Assets = serde_json::from_slice(&fs::read(
        rc_inspect::build_inputs::input("map_b_gameplay")?,
    )?)?;
    let faces: std::collections::BTreeMap<String, Vec<usize>> = serde_json::from_slice(&fs::read(
        rc_inspect::build_inputs::input("map_b_actor_faces")?,
    )?)?;
    let mut removed = std::collections::BTreeSet::new();
    for gate in &mut gameplay.gates {
        let indices = faces
            .get(&gate.actor)
            .ok_or(format!("Missing gate geometry {}", gate.actor))?;
        if indices.is_empty() {
            continue;
        }
        let mut textures = vec![];
        let mut map = std::collections::BTreeMap::new();
        let appearance = indices
            .iter()
            .map(|&i| {
                game.scene.appearance[i].map(|mut f| {
                    f.texture = *map.entry(f.texture).or_insert_with(|| {
                        let n = textures.len();
                        textures.push(game.scene.textures[f.texture].clone());
                        n
                    });
                    f
                })
            })
            .collect();
        let triangles: Vec<_> = indices
            .iter()
            .map(|&i| game.scene.triangles[i].clone())
            .collect();
        let mut bounds = [[f32::INFINITY; 3], [f32::NEG_INFINITY; 3]];
        for v in triangles.iter().flat_map(|t| t.points) {
            for (i, x) in v.into_iter().enumerate() {
                bounds[0][i] = bounds[0][i].min(x);
                bounds[1][i] = bounds[1][i].max(x);
            }
        }
        let center = std::array::from_fn(|i| (bounds[0][i] + bounds[1][i]) * 0.5);
        let radius = (0..3)
            .map(|i| (bounds[1][i] - bounds[0][i]).powi(2))
            .sum::<f32>()
            .sqrt()
            * 0.5;
        let scene = rc_render::Scene {
            triangles: indices
                .iter()
                .map(|&i| game.scene.triangles[i].clone())
                .collect(),
            appearance,
            textures,
            center,
            radius,
            starts: vec![],
            solid: None,
        };
        gate.snapshot = scene.snapshot()?;
        removed.extend(indices.iter().copied());
    }
    game.scene.triangles = game
        .scene
        .triangles
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !removed.contains(i))
        .map(|(_, t)| t)
        .collect();
    game.scene.appearance = game
        .scene
        .appearance
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !removed.contains(i))
        .map(|(_, t)| t)
        .collect();
    let mut resolver = rc_inspect::assets::Assets::new(&[
        &root.join("Textures"),
        &root.join("Animations"),
        &root.join("System"),
    ])?;
    gameplay.droid_mesh = Some(rc_inspect::visual_assets::load(
        root,
        "Droid.ukx",
        "BattleDroid",
        &["ActionBreathe", "RunForward", "Death"],
        &mut resolver,
    )?);
    gameplay.warrior_mesh = Some(rc_inspect::visual_assets::load(
        root,
        "Geonosian.ukx",
        "GeonosianWarrior",
        &["ActionBreathe", "RunForward", "Death"],
        &mut resolver,
    )?);
    let mut fixer = rc_inspect::visual_assets::load(
        root,
        "Clone.ukx",
        "CloneCommando",
        &["ActionBreathe", "RunForward", "Death"],
        &mut resolver,
    )?;
    let data = fs::read(root.join("Properties/properties.u"))?;
    let package = rc_package::read_package(&data)?;
    let entry = package
        .exports
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            (e.class == 0
                && package
                    .object_path(i as i32 + 1)
                    .is_ok_and(|n| n.eq_ignore_ascii_case("Commando40DeltaDefaults")))
            .then_some(e)
        })
        .ok_or("Missing Fixer template")?;
    let props = rc_package::classes::read(&package, &data, entry)?.properties;
    let payload = package.payload(&data, entry)?;
    let skins = props
        .values
        .iter()
        .find(|p| p.name == "Skins")
        .ok_or("Missing Fixer skins")?;
    let skins = rc_package::properties::object_array(
        &package,
        &payload[skins.payload_offset..skins.payload_offset + skins.bytes],
    )?;
    if skins.len() != fixer.materials.len() {
        return Err("Fixer skin count changed".into());
    }
    let catalog = rc_inspect::defaults::load(root)?;
    for (material, (_, source)) in fixer.materials.iter_mut().zip(skins) {
        let mut d = resolver.diffuse_at_time(&source, 256, 0.)?;
        let (class, props) = resolver.material_properties(&source)?;
        if class == "Engine.Shader"
            && matches!(
                catalog
                    .instance(&class, &props, "Opacity", 0, &source)?
                    .value,
                rc_package::properties::Value::Object { index: 0, .. }
            )
        {
            for pixel in &mut d.texture.pixels {
                *pixel |= 0xff000000;
            }
        }
        material.source = source;
        material.chain = d.chain;
        material.width = d.texture.width;
        material.height = d.texture.height;
        material.pixels = d.texture.pixels;
        material.uv_scale = d.scale;
        material.uv_transform = d.uv_transform;
    }
    gameplay.fixer_mesh = Some(fixer);
    let original = rc_game::bundle::decode(&fs::read(&a[3])?)?;
    let visual = original
        .visuals
        .as_ref()
        .ok_or("Missing original visual assets")?;
    let mut manifest = rc_game::bundle::Manifest::capture(
        &game,
        serde_json::json!({"map":"geo_01b.ctm","entry":"Original PlayerStart0","scope":"Selected original GEO_01B objectives, one-shot event chains and factories; bounded interaction/combat hosts"}),
    )?;
    manifest.name = "GEO_01B".into();
    let base = rc_game::bundle::encode(&manifest, &game.scene)?;
    let base = rc_game::bundle::encode_animated(
        &base,
        &rc_game::visual::Assets {
            enemy: copy(&visual.enemy.mesh)?,
            weapon: copy(&visual.weapon.mesh)?,
        },
    )?;
    let mut sounds = vec![copy(&original.mission.as_ref().unwrap().sounds[4])?];
    let mut sound_indices = std::collections::BTreeMap::new();
    let mut receipts = vec![];
    for script in &mut gameplay.scripts {
        for action in &mut script.actions {
            if action.op != "sound" {
                continue;
            }
            let (index, duration) = if let Some(value) = sound_indices.get(&action.text) {
                *value
            } else {
                let (sound, duration) =
                    rc_inspect::mission_assets::sound_variant(root, &action.text, &mut receipts)?;
                let value = (sounds.len(), duration);
                sounds.push(sound);
                sound_indices.insert(action.text.clone(), value);
                value
            };
            action.number = index;
            if action.seconds < 0. {
                action.seconds = duration;
            }
        }
    }
    let squad = &original.tactics.as_ref().unwrap().assets.squad;
    gameplay.squad_fire_sound = sounds.len();
    sounds.push(copy(&squad.sounds[squad.fire_sound])?);
    gameplay.demolition_sound = sounds.len();
    sounds.push(
        rc_inspect::mission_assets::sound(root, "GEN_Sound.Impacts_Explos.exp_GEN_demoCharge_01")?
            .0,
    );
    let assets = rc_game::map_runtime::Assets {
        enemy_fire_sound: None,
        sev_mesh: None,
        opening: None,
        fixer_mesh: None,
        gameplay: Some(Box::new(gameplay)),
        destination: "GEO_01B.ctm#playerstart".into(),
        start: game.scene.starts[0].actor.clone(),
        pitch: game.scene.starts[0].rotation[0] as f64 * std::f64::consts::TAU / 65536.,
        weapon_snapshot: original
            .tactics
            .as_ref()
            .unwrap()
            .assets
            .squad
            .weapon_snapshot
            .clone(),
        squad_mesh: Some(copy(
            &original
                .rendezvous
                .as_ref()
                .ok_or("Missing original Scorch")?
                .mesh
                .mesh,
        )?),
        sounds,
    };
    let metadata = serde_json::to_vec(&assets)?;
    let mut bytes = b"RCPLAY14".to_vec();
    bytes.extend((metadata.len() as u32).to_le_bytes());
    bytes.extend(&metadata);
    bytes.extend(base);
    println!("Map2 gameplay metadata: {} bytes", metadata.len());
    let mut decoded = rc_game::bundle::decode(&bytes)?;
    let handoff: rc_game::transition::Handoff = serde_json::from_slice(&fs::read(
        rc_inspect::build_inputs::input("map_a_handoff")?,
    )?)?;
    decoded.restore_handoff(&handoff)?;
    for _ in 0..600 {
        decoded.advance(rc_game::STEP, rc_game::Input::default())?;
    }
    fs::write(
        out.join("audio-receipt.json"),
        serde_json::to_vec_pretty(&receipts)?,
    )?;
    fs::write(out.join("geo_01b.rcplay"), &bytes)?;
    fs::write(
        out.join("build-receipt.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"bytes":bytes.len(),"spawn":manifest.spawn,"starts":game.scene.starts,"colliders":manifest.colliders.len(),"triangles":game.scene.triangles.len(),"textures":game.scene.textures.len(),"entry_state":decoded.state,"scope":"Selected original map2 event chains/8 objectives/42 factories/5 interactions; bounded gameplay hosts, not full native script/AI equivalence"}),
        )?,
    )?;
    println!(
        "GEO_01B: {} bytes; {} triangles; {} colliders; spawn {:?}; carried {}HP",
        bytes.len(),
        game.scene.triangles.len(),
        manifest.colliders.len(),
        manifest.spawn,
        decoded.state.health
    );
    Ok(())
}
