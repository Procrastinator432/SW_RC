use rc_game::map3_escape::{translated, Assets, Door};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    sync::Arc,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = rc_inspect::build_inputs::output()?;
    fs::create_dir_all(&out)?;
    let raw = fs::read(rc_inspect::build_inputs::input("map_c_planning_scene")?)?;
    let n = u32::from_le_bytes(raw[8..12].try_into()?) as usize;
    let mut map: rc_game::map_runtime::Assets = serde_json::from_slice(&raw[12..12 + n])?;
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
    let point = |id| -> Result<[f64; 3], serde_json::Error> {
        serde_json::from_value(props(id)["Location"].clone())
    };
    let mut g = rc_game::bundle::decode(&raw)?;
    let base = &raw[12 + n..];
    let vn = u32::from_le_bytes(base[8..12].try_into()?) as usize;
    let static_base = &base[12 + vn..];
    let sn = u32::from_le_bytes(static_base[8..12].try_into()?) as usize;
    let mut scene = rc_render::Scene::from_snapshot(&static_base[12 + sn..])?;
    let faces: BTreeMap<String, Vec<usize>> = serde_json::from_slice(&fs::read(
        rc_inspect::build_inputs::input("map_c_actor_faces")?,
    )?)?;
    let old_removed: BTreeSet<_> = faces["WindowShatter_Test1"].iter().copied().collect();
    let mut removed = BTreeSet::new();
    let mut doors = vec![];
    for id in [134, 135] {
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
        let a = g
            .world
            .actors
            .iter_mut()
            .find(|a| a.name == actor)
            .ok_or("Missing original door collision")?;
        if let rc_package::world_collision::BodyShape::Hulls(h) = &a.shape {
            a.shape =
                rc_package::world_collision::BodyShape::Hulls(Arc::new(translated(h, offset)));
        } else {
            return Err("Invalid door collision".into());
        }
        doors.push(Door {
            original: id,
            actor,
            offset,
            snapshot: gate.snapshot()?,
        });
    }
    let extent = [50., 50., 115.];
    let ground = |at: [f64; 3]| -> Result<[f64; 3], String> {
        rc_game::mission_motion::ground(&g.world, [at[0], at[1], at[2] + 150.], extent, 400.)?
            .ok_or("Missing supported escape point".into())
    };
    let start = ground(point(328)?)?;
    let target = ground(point(198)?)?;
    let mut nav = rc_game::navigation::Assets {
        provenance: serde_json::json!({"scope":"Geometric walking graph over nearby original navigation actors; not native ReachSpec parity"}),
        nodes: vec![],
        edges: vec![],
        lunge_distance: 0.,
        jump_distance: 0.,
        offensive_jump_probability: 0.,
    };
    for r in &rows {
        if !r["class"].as_str().unwrap_or("").contains("PathNode")
            && r["class"] != "CTGame.AutoDoor"
        {
            continue;
        }
        let id = r["original"].as_u64().unwrap() as u32;
        let Ok(at) = point(id) else {
            continue;
        };
        if !(-21100. ..=-18500.).contains(&at[0]) || !(8500. ..=11000.).contains(&at[1]) {
            continue;
        }
        if let Ok(position) = ground(at) {
            nav.nodes.push(rc_game::navigation::Node {
                original: id as i32,
                name: r["name"].as_str().unwrap().into(),
                position,
                extra_cost: 0.,
            });
        }
    }
    for i in 0..nav.nodes.len() {
        for j in 0..nav.nodes.len() {
            if i == j {
                continue;
            }
            let from = nav.nodes[i].position;
            let to = nav.nodes[j].position;
            let distance = distance(from, to);
            if distance < 1100.
                && rc_game::squad::corridor_with_tolerance(&g.world, from, to, extent, 4.)?
                    .is_some()
            {
                nav.edges.push(rc_game::navigation::Edge {
                    original: 0,
                    start: i,
                    end: j,
                    distance,
                });
            }
        }
    }
    let route = rc_game::squad::plan_with_tolerance(&nav, &g.world, start, target, extent, 4.)?;
    if route.is_empty() {
        return Err(format!(
            "No physical escape route: {:?} -> {:?}; {} nodes, {} links",
            start,
            target,
            nav.nodes.len(),
            nav.edges.len()
        )
        .into());
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
    let root = &rc_inspect::build_inputs::game_data()?;
    let catalog = rc_inspect::defaults::load(root)?;
    let scalar = |class, name| -> Result<f64, Box<dyn std::error::Error>> {
        match catalog.resolve(class, name, 0)?.value {
            rc_package::properties::Value::Float(f) => Ok(f as f64),
            _ => Err("Missing original scalar".into()),
        }
    };
    let speed = scalar("Properties.GeonosianSunFac", "GroundSpeed")?;
    let move_time = scalar(row(134)["class"].as_str().unwrap(), "MoveTimes")?;
    let trigger = props(144);
    let trigger_height = scalar("Engine.Trigger", "CollisionHeight")?;
    let o = map.opening.as_mut().unwrap();
    let p = o.planning.as_mut().unwrap();
    let mesh = p.warrior.as_ref().unwrap();
    let run = mesh
        .clips
        .iter()
        .find(|c| c.sequence.name.name == "RunForward")
        .ok_or("Missing Sun Fac RunForward")?;
    o.actors[5]
        .mesh
        .as_mut()
        .unwrap()
        .clips
        .push(serde_json::from_value(serde_json::to_value(run)?)?);
    p.escape = Some(Box::new(Assets {
        route: route.clone(),
        start,
        destination: point(32)?,
        destination_rotation: serde_json::from_value(props(32)["Rotation"].clone())?,
        speed,
        move_time,
        doors,
        door_position: point(139)?,
        trigger: point(144)?,
        trigger_radius: trigger["CollisionRadius"].as_f64().unwrap(),
        trigger_height,
        provenance: serde_json::json!({"original_actions":[1008,1009,1010,1011,1012,1013,1014,1015,1016],"original_door":139,"original_destination":198,"original_teleport_destination":32,"original_trigger":144,"scope":"Collision-tested walking, original targets, speed, mover offsets and trigger dimensions; bounded steering/door interpolation/overlap host; native AI, mover timing/encroachment parity and SBD execution pending"}),
    }));
    let metadata = serde_json::to_vec(&map)?;
    let metadata_bytes = metadata.len();
    let mut bytes = b"RCPLAY14".to_vec();
    bytes.extend((metadata.len() as u32).to_le_bytes());
    bytes.extend(metadata);
    bytes.extend(&base[..12 + vn]);
    bytes.extend(&static_base[..12 + sn]);
    bytes.extend(scene.snapshot()?);
    rc_game::bundle::decode(&bytes)?;
    fs::write(out.join("geo_01c.rcplay"), &bytes)?;
    fs::write(
        out.join("build-receipt.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"bytes":bytes.len(),"metadata_bytes":metadata_bytes,"door_faces":removed.len(),"speed":speed,"move_time":move_time,"trigger_height":trigger_height,"start_authored":point(328)?,"start_supported":start,"target_authored":point(198)?,"target_supported":target,"route_points":route.len(),"route":route,"navigation":nav,"sun_fac_script":row(193),"door_controller":row(139),"proximity_trigger":row(144)}),
        )?,
    )?;
    println!(
        "Sun Fac escape: {} bytes; {} route points; speed {}; MoveTime {}",
        bytes.len(),
        route.len(),
        speed,
        move_time
    );
    Ok(())
}
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}
