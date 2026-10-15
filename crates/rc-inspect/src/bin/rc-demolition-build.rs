//! Build a self-contained RCPLAY08 selected objective0-5 scene from verified inputs.
use rc_game::{
    bundle,
    demolition_runtime::{Assets, Notify},
};
use std::{env, fs, path::Path};
fn wrap(magic: &[u8], meta: &[u8], base: &[u8]) -> Vec<u8> {
    let mut b = magic.to_vec();
    b.extend((meta.len() as u32).to_le_bytes());
    b.extend(meta);
    b.extend(base);
    b
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 5 {
        return Err("usage: rc-demolition-build GAME_DATA SCORCH_SCENE PREPARED OUTPUT".into());
    }
    let root = Path::new(&args[1]);
    let prepared = Path::new(&args[3]);
    let out = Path::new(&args[4]);
    fs::create_dir_all(out)?;
    let flow: rc_game::demolition::Assets =
        serde_json::from_slice(&fs::read(prepared.join("demolition-assets.json"))?)?;
    let original: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join("original-bindings.json"))?)?;
    let charge = original["actors_and_actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["original"] == 465)
        .unwrap();
    let charge_at =
        serde_json::from_value(charge["resolved"]["Location"]["value"]["value"].clone())?;
    let yaw = charge["resolved"]["Rotation"]["value"]["value"][1]
        .as_i64()
        .ok_or("Missing original yaw")? as f64
        * std::f64::consts::TAU
        / 65536.;
    let world: serde_json::Value =
        serde_json::from_slice(&fs::read(prepared.join("world-validation.json"))?)?;
    assert_eq!(world["passed"], true);
    let route = serde_json::from_value(world["route"].clone())?;
    let mut sources = std::collections::BTreeSet::new();
    let bank: serde_json::Value =
        serde_json::from_slice(&fs::read(prepared.join("sound-bindings.json"))?)?;
    sources.extend(bank.as_object().unwrap().keys().cloned());
    let mut notifies = vec![];
    for clip in original["clips"].as_array().unwrap() {
        for n in clip["notifies"].as_array().unwrap() {
            if n["class"] == "Engine.AnimNotify_Sound" {
                let path = n["properties"]["values"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|p| p["name"] == "Sound")
                    .unwrap()["value"]["value"]["path"]
                    .as_str()
                    .unwrap()
                    .to_owned();
                sources.insert(path.clone());
                notifies.push(Notify {
                    clip: clip["name"].as_str().unwrap().into(),
                    time: n["time"].as_f64().unwrap() as f32,
                    source: path,
                });
            }
        }
    }
    let hands_notify: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join("hands-notifies.json"))?)?;
    let mut fire = None;
    for n in hands_notify["notifies"].as_array().unwrap() {
        if n["class"] == "Gameplay.AnimNotify_FireWeapon" {
            fire = Some(n["time"].as_f64().unwrap() as f32);
        }
        if n["class"] == "Engine.AnimNotify_Sound" {
            let path = n["properties"]["values"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["name"] == "Sound")
                .unwrap()["value"]["value"]["path"]
                .as_str()
                .unwrap()
                .to_owned();
            sources.insert(path.clone());
            notifies.push(Notify {
                clip: "Detonate".into(),
                time: n["time"].as_f64().unwrap() as f32,
                source: path,
            });
        }
    }
    let mut audio_receipts = vec![];
    let sounds = sources
        .into_iter()
        .map(|s| {
            rc_inspect::mission_assets::sound_variant(root, &s, &mut audio_receipts)
                .map(|(s, _)| s)
                .map_err(|e| format!("Sound {s}: {e}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    fs::write(
        out.join("audio-variants.json"),
        serde_json::to_vec_pretty(&audio_receipts)?,
    )?;
    let object: rc_game::dynamic::Object =
        serde_json::from_slice(&fs::read(prepared.join("bunker-dynamic.json"))?)?;
    let original_bunker = rc_render::Scene::from_snapshot(&object.snapshot)?;
    let mut base = fs::read(&args[2])?;
    let mut layers = vec![];
    while !base.starts_with(b"RCPLAY01") {
        let n = u32::from_le_bytes(base[8..12].try_into()?) as usize;
        layers.push((base[..8].to_vec(), base[12..12 + n].to_vec()));
        base = base[12 + n..].to_vec();
    }
    if layers.first().is_none_or(|(m, _)| m != b"RCPLAY07") {
        return Err("Expected original Scorch scene".into());
    }
    let mut scorch: rc_game::rendezvous_runtime::Assets = serde_json::from_slice(&layers[0].1)?;
    scorch.mesh = Some(serde_json::from_slice(&fs::read(
        prepared.join("scorch-demolition-mesh.json"),
    )?)?);
    // Complete the two original tutorial speech actions after activating the marker.
    let intro = scorch
        .flow
        .programs
        .iter_mut()
        .find(|p| p.script == 1128)
        .ok_or("Missing original tutorial")?;
    for (id, path) in [
        (3550, "GEO_Voice.GEO_01.D38G1535"),
        (3551, "GEO_Voice.GEO_01.D62G1511"),
    ] {
        let (sound, seconds) = rc_inspect::mission_assets::sound(root, path)?;
        intro.actions.push(rc_game::rendezvous::Action {
            original: id,
            operation: rc_game::rendezvous::Operation::Sound {
                source: path.into(),
                seconds,
                owner: "None".into(),
            },
        });
        scorch.sounds.push(sound);
    }
    layers[0].1 = serde_json::to_vec(&scorch)?;
    let n = u32::from_le_bytes(base[8..12].try_into()?) as usize;
    let mut manifest: bundle::Manifest = serde_json::from_slice(&base[12..12 + n])?;
    let scene = rc_render::Scene::from_snapshot(&base[12 + n..])?;
    let key = |t: &rc_package::geometry::Triangle| t.points.map(|p| p.map(f32::to_bits));
    let mut remove = original_bunker
        .triangles
        .iter()
        .map(|t| (key(t), 1usize))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut triangles = vec![];
    let mut faces = vec![];
    let mut removed = 0;
    for (i, t) in scene.triangles.into_iter().enumerate() {
        if let Some(n) = remove.get_mut(&key(&t)) {
            if *n > 0 {
                *n -= 1;
                removed += 1;
                continue;
            }
        }
        triangles.push(t);
        faces.push(scene.appearance[i]);
    }
    assert_eq!(removed, original_bunker.triangles.len());
    assert_eq!(remove.values().sum::<usize>(), 0);
    let i = manifest
        .colliders
        .iter()
        .position(|c| c.name == "Prop12")
        .ok_or("Missing static Prop12")?;
    manifest.colliders.remove(i);
    let mut rebuilt = rc_render::Scene::new(triangles)?;
    rebuilt.appearance = faces;
    rebuilt.textures = scene.textures;
    rebuilt.starts = scene.starts;
    rebuilt.solid = scene.solid;
    let mut base = bundle::encode(&manifest, &rebuilt)?;
    for (magic, meta) in layers.into_iter().rev() {
        base = wrap(&magic, &meta, &base);
    }
    let a = Assets {
        flow,
        object: Some(object),
        route,
        marker: serde_json::from_value(world["marker"].clone())?,
        charge: charge_at,
        yaw,
        sounds,
        notifies,
        hands: Some(serde_json::from_slice(&fs::read(
            out.join("hands-mesh.json"),
        )?)?),
        plunger: fs::read(out.join("plunger.rcscene"))?,
        charges: [
            fs::read(out.join("charge-0.rcscene"))?,
            fs::read(out.join("charge-1.rcscene"))?,
        ],
        fire_fraction: fire.ok_or("Missing original detonation notify")?,
        provenance: serde_json::json!({"original_marker":466,"original_charge":465,"original_prop":851,"released_navigation":1094,"static_faces_moved":removed,"controller":"bounded physical marker host","extra_factories":"SpawnDemoDroids has no map receiver; SpawnWarriorAttack remains separate combat extension"}),
    };
    fs::write(out.join("runtime-assets.json"), serde_json::to_vec(&a)?)?;
    let result = wrap(b"RCPLAY08", &serde_json::to_vec(&a)?, &base);
    if result.len() > 96 * 1024 * 1024 {
        return Err("Scene exceeds app budget".into());
    }
    let game = bundle::decode(&result)?;
    fs::write(out.join("geo_01a-demolition.rcplay"), &result)?;
    fs::write(
        out.join("build-validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"bytes":result.len(),"static_faces_moved":removed,"sound_count":game.mission.as_ref().unwrap().sounds.len(),"destroyed_hulls":game.dynamic.as_ref().unwrap().objects.last().unwrap().released_hulls.as_ref().unwrap().hulls.len(),"scope":"Built live selected objective5; input-driven acceptance separate"}),
        )?,
    )?;
    println!(
        "Built objective0-5 scene: {} bytes, {removed} bunker faces moved to dynamic, {} sounds",
        result.len(),
        game.mission.as_ref().unwrap().sounds.len()
    );
    Ok(())
}
