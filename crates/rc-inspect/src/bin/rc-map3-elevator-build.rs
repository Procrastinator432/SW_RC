use rc_game::{map3_elevator::Assets, map3_escape::Door};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = Path::new("analysis/mission27-map3-elevator/delivery");
    fs::create_dir_all(out)?;
    let raw = fs::read("analysis/mission26-map3-finale/delivery/geo_01c.rcplay")?;
    let n = u32::from_le_bytes(raw[8..12].try_into()?) as usize;
    let mut map: rc_game::map_runtime::Assets = serde_json::from_slice(&raw[12..12 + n])?;
    let rows: Vec<serde_json::Value> = serde_json::from_slice(&fs::read(
        "analysis/mission19-map3/original-map-inventory.json",
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
    let point = |id| -> Result<[f64; 3], serde_json::Error> {
        serde_json::from_value(props(id)["Location"].clone())
    };
    let base = &raw[12 + n..];
    let vn = u32::from_le_bytes(base[8..12].try_into()?) as usize;
    let static_base = &base[12 + vn..];
    let sn = u32::from_le_bytes(static_base[8..12].try_into()?) as usize;
    let mut scene = rc_render::Scene::from_snapshot(&static_base[12 + sn..])?;
    let faces: BTreeMap<String, Vec<usize>> = serde_json::from_slice(&fs::read(
        "analysis/mission19-map3/surfaces/actor-faces.json",
    )?)?;
    let old_removed: BTreeSet<_> = [
        "WindowShatter_Test1",
        "MoverDB_GEO29",
        "MoverDBSlave_GEO29",
        "MoverDB_GEO18",
        "MoverDBSlave_GEO18",
    ]
    .into_iter()
    .flat_map(|name| faces[name].iter().copied())
    .collect();
    let mut removed = BTreeSet::new();
    let mut doors = vec![];
    for id in [465, 466] {
        let actor = row(id)["name"].as_str().unwrap().to_owned();
        let indices: Vec<_> = faces[&actor]
            .iter()
            .map(|i| i - old_removed.range(..i).count())
            .collect();
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
        let triangles = indices
            .iter()
            .map(|&i| scene.triangles[i].clone())
            .collect();
        let offset: [f64; 3] = serde_json::from_value(props(id)["KeyPos"].clone())?;
        let gate = rc_render::Scene {
            triangles,
            appearance,
            textures,
            center: point(id)?.map(|v| v as f32),
            radius: 500.,
            starts: vec![],
            solid: None,
        };
        removed.extend(indices);
        doors.push(Door {
            original: id,
            actor,
            offset,
            snapshot: gate.snapshot()?,
        });
    }
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
    let catalog = rc_inspect::defaults::load(Path::new(
        "D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData",
    ))?;
    let scalar = |class, key| -> Result<f64, Box<dyn std::error::Error>> {
        match catalog.resolve(class, key, 0)?.value {
            rc_package::properties::Value::Float(f) => Ok(f as f64),
            _ => Err("Missing scalar".into()),
        }
    };
    let a = Assets {
        doors,
        move_time: scalar("MarkerProperties.MoverDB_GEO", "MoveTimes")?,
        door_trigger: point(468)?,
        door_radius: 200.,
        door_height: 100.,
        approach: point(227)?,
        approach_radius: 600.,
        approach_height: scalar("Engine.Trigger", "CollisionHeight")?,
        exit: point(39)?,
        exit_radius: 900.,
        exit_height: scalar("Engine.Teleporter", "CollisionHeight")?,
        provenance: serde_json::json!({"original":[39,45,227,318,465,466,467,468,469,470],"scope":"Original unlocked auto-door offsets/MoveTimes and cylinder dimensions. ConditionSunFacDead gates exit; physical player entry and clear line through opened door required. Static factory elevator, no invented moving platform. Bounded overlap and encroachment prevention; exact native door state machine/antiportal/exit icon pending."}),
    };
    map.opening
        .as_mut()
        .unwrap()
        .planning
        .as_mut()
        .unwrap()
        .guard
        .as_mut()
        .unwrap()
        .breach
        .as_mut()
        .unwrap()
        .balcony
        .as_mut()
        .unwrap()
        .finale
        .as_mut()
        .unwrap()
        .elevator = Some(Box::new(a));
    let metadata = serde_json::to_vec(&map)?;
    let size = metadata.len();
    let mut bytes = b"RCPLAY14".to_vec();
    bytes.extend((size as u32).to_le_bytes());
    bytes.extend(metadata);
    bytes.extend(&base[..12 + vn]);
    bytes.extend(&static_base[..12 + sn]);
    bytes.extend(scene.snapshot()?);
    rc_game::bundle::decode(&bytes)?;
    fs::write(out.join("geo_01c.rcplay"), &bytes)?;
    fs::write(
        out.join("build-receipt.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"bytes":bytes.len(),"metadata_bytes":size,"door_faces":removed.len(),"originals":([39,45,227,318,465,466,467,468,469,470].iter().map(|id|row(*id)).collect::<Vec<_>>())}),
        )?,
    )?;
    println!(
        "Elevator: {} bytes; metadata {}; door faces {}",
        bytes.len(),
        size,
        removed.len()
    );
    Ok(())
}
