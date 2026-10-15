use rc_game::{
    map3_opening::{Action, Program},
    map3_planning::{Assets, Factory},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = rc_inspect::build_inputs::output()?;
    fs::create_dir_all(&out)?;
    let raw = fs::read(rc_inspect::build_inputs::input("map_c_opening_scene")?)?;
    let n = u32::from_le_bytes(raw[8..12].try_into()?) as usize;
    let mut map: rc_game::map_runtime::Assets = serde_json::from_slice(&raw[12..12 + n])?;
    let b = fs::read(rc_inspect::build_inputs::input("map_b_scene")?)?;
    let bn = u32::from_le_bytes(b[8..12].try_into()?) as usize;
    let mut bm: rc_game::map_runtime::Assets = serde_json::from_slice(&b[12..12 + bn])?;
    let bg = bm.gameplay.as_mut().unwrap();
    let rows: Vec<serde_json::Value> = serde_json::from_slice(&fs::read(
        rc_inspect::build_inputs::input("map_c_inventory")?,
    )?)?;
    let row = |id| rows.iter().find(|r| r["original"] == id).unwrap();
    let props = |id| {
        row(id)["properties"]["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                (
                    p["name"].as_str().unwrap().to_owned(),
                    p["value"]["value"].clone(),
                )
            })
            .collect::<BTreeMap<_, _>>()
    };
    let collision = rc_game::bundle::decode(&raw)?;
    let mut spawn_receipts = vec![];
    let mut factories = vec![];
    for id in [164, 165, 190, 191, 192] {
        let p = props(id);
        let value = |key: &str| {
            row(id)["prototype_defaults"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["property"] == key)
                .unwrap()["resolved"]["Ok"]["value"]["value"]
                .as_f64()
                .unwrap()
        };
        let authored: [f64; 3] = serde_json::from_value(p["Location"].clone())?;
        let extent = [
            value("CollisionRadius"),
            value("CollisionRadius"),
            value("CollisionHeight"),
        ];
        let mut position = authored;
        let mut placed = false;
        for rise in 0..=160 {
            position[2] = authored[2] + rise as f64;
            if collision.world.body_placement(position, extent)?.state
                != rc_package::movement::PlacementState::Penetrating
            {
                placed = true;
                break;
            }
        }
        if !placed {
            return Err(format!("No bounded supported placement for factory {id}").into());
        }
        spawn_receipts.push(serde_json::json!({"original":id,"authored":authored,"position":position,"extent":extent,"z_lift":position[2]-authored[2],"host":"Smallest integral upward clearance for the static AABB collision host; original cylinder spawn parity pending"}));
        factories.push(Factory {
            original: id,
            position,
            health: value("Health") as u32,
            extent,
            warrior: id >= 190,
        });
    }
    let base = &raw[12 + n..];
    let vn = u32::from_le_bytes(base[8..12].try_into()?) as usize;
    let static_base = &base[12 + vn..];
    let sn = u32::from_le_bytes(static_base[8..12].try_into()?) as usize;
    let mut scene = rc_render::Scene::from_snapshot(&static_base[12 + sn..])?;
    let faces: BTreeMap<String, Vec<usize>> = serde_json::from_slice(&fs::read(
        rc_inspect::build_inputs::input("map_c_actor_faces")?,
    )?)?;
    let indices = faces
        .get("WindowShatter_Test1")
        .ok_or("Missing glass faces")?;
    let mut textures = vec![];
    let mut remap = BTreeMap::new();
    let appearance = indices
        .iter()
        .map(|&i| {
            scene.appearance[i].map(|mut f| {
                f.texture = *remap.entry(f.texture).or_insert_with(|| {
                    let n = textures.len();
                    textures.push(scene.textures[f.texture].clone());
                    n
                });
                f
            })
        })
        .collect();
    let triangles: Vec<_> = indices
        .iter()
        .map(|&i| scene.triangles[i].clone())
        .collect();
    let mut bounds = [[f32::INFINITY; 3], [f32::NEG_INFINITY; 3]];
    for p in triangles.iter().flat_map(|t| t.points) {
        for i in 0..3 {
            bounds[0][i] = bounds[0][i].min(p[i]);
            bounds[1][i] = bounds[1][i].max(p[i]);
        }
    }
    let center = std::array::from_fn(|i| (bounds[0][i] + bounds[1][i]) * 0.5);
    let glass = rc_render::Scene {
        triangles,
        appearance,
        textures,
        center,
        radius: 1000.,
        starts: vec![],
        solid: None,
    };
    let removed: BTreeSet<_> = indices.iter().copied().collect();
    scene.triangles = scene
        .triangles
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !removed.contains(i))
        .map(|(_, t)| t)
        .collect();
    scene.appearance = scene
        .appearance
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !removed.contains(i))
        .map(|(_, t)| t)
        .collect();
    let squad_sound = map.sounds.len();
    map.sounds.push(serde_json::from_value(serde_json::to_value(
        &bm.sounds[bg.squad_fire_sound],
    )?)?);
    let o = map.opening.as_mut().unwrap();
    o.planning = Some(Box::new(Assets {
        guard: None,
        escape: None,
        factories,
        droid: bg.droid_mesh.take(),
        warrior: bg.warrior_mesh.take(),
        glass_actor: "WindowShatter_Test1".into(),
        glass_health: 150,
        glass_snapshot: glass.snapshot()?,
        squad_sound,
        provenance: serde_json::json!({"factories":[164,165,190,191,192],"glass":263,"destroy_event":"WindowDestroyed","destroyed_mesh":"None","effect":"CTEffects.WindowShattering_doubleSized","scope":"Selected original actor bindings and event; bounded hitscan/AI/assistance and collision retirement; native Prop damage modifiers, particles, Karma and Sun Fac path execution remain pending"}),
    }));
    let mut actions = vec![];
    let mut sound_receipts = vec![];
    let root = &rc_inspect::build_inputs::game_data()?;
    for id in [1490, 1491, 1492, 1493, 1494] {
        let p = props(id);
        let mut a = Action {
            original: id,
            op: String::new(),
            text: String::new(),
            role: 0,
            seconds: 0.,
        };
        match id {
            1490 => {
                a.op = "wait".into();
                a.text = p["ExternalEvent"].as_str().unwrap().into();
            }
            1491 => {
                a.op = "objective".into();
                a.role = p["ObjectiveNum"].as_u64().unwrap() as usize;
            }
            _ => {
                let (sound, _) = rc_inspect::mission_assets::sound_variant(
                    root,
                    p["Sound"]["path"].as_str().unwrap(),
                    &mut sound_receipts,
                )?;
                a.op = "sound".into();
                a.role = map.sounds.len();
                a.seconds = wave_seconds(&sound.wave)?;
                a.text = p["Comment"].as_str().unwrap().into();
                map.sounds.push(sound);
            }
        }
        actions.push(a);
    }
    o.scripts.push(Program {
        original: 356,
        actions,
    });
    let metadata = serde_json::to_vec(&map)?;
    let mut bytes = b"RCPLAY14".to_vec();
    bytes.extend((metadata.len() as u32).to_le_bytes());
    bytes.extend(metadata);
    bytes.extend(&base[..12 + vn]);
    bytes.extend(&static_base[..12 + sn]);
    bytes.extend(scene.snapshot()?);
    let g = rc_game::bundle::decode(&bytes)?;
    let p = g
        .map
        .as_ref()
        .unwrap()
        .opening
        .as_ref()
        .unwrap()
        .planning
        .as_ref()
        .unwrap();
    fs::write(out.join("geo_01c.rcplay"), &bytes)?;
    fs::write(
        out.join("build-receipt.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"bytes":bytes.len(),"glass_faces":indices.len(),"glass_bounds":bounds,"glass_hulls":p.collider.1.hulls.len(),"factories":p.assets.factories,"placement":spawn_receipts,"sounds":map.sounds.len(),"sound_receipts":sound_receipts,"scope":p.assets.provenance}),
        )?,
    )?;
    println!(
        "Planning room: {} bytes; {} glass faces; bounds {:?}",
        bytes.len(),
        indices.len(),
        bounds
    );
    Ok(())
}
fn wave_seconds(bytes: &[u8]) -> Result<f64, String> {
    let (mut offset, mut rate, mut data) = (12, 0, 0);
    while offset + 8 <= bytes.len() {
        let n = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let p = offset + 8;
        if p + n > bytes.len() {
            return Err("Truncated wave".into());
        }
        match &bytes[offset..offset + 4] {
            b"fmt " if n >= 12 => {
                rate = u32::from_le_bytes(bytes[p + 8..p + 12].try_into().unwrap())
            }
            b"data" => data = n,
            _ => {}
        }
        offset = p + n + (n & 1);
    }
    if rate == 0 || data == 0 {
        return Err("Missing duration".into());
    }
    Ok(data as f64 / rate as f64)
}
