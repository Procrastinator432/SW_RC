//! Extract selected normal walking connections from the original map PathLists.
use rc_game::{
    bundle, combat,
    navigation::{self, Assets, Edge, Node},
    warrior_ai,
};
use rc_inspect::defaults;
use rc_package::{
    properties::{self, Value},
    read_package,
};
use std::{collections::BTreeMap, env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-warrior-ai-build GAME_DATA ARTILLERY_SCENE OUTPUT_DIRECTORY".into());
    }
    let game_data = Path::new(&args[1]);
    let bytes = fs::read(&args[2])?;
    if !bytes.starts_with(b"RCPLAY10") {
        return Err("Need artillery scene".into());
    }
    let playable = bundle::decode(&bytes)?;
    let original_combat = fs::read("analysis/milestone3l/geo_01a-normal.rcplay")?;
    if !original_combat.starts_with(b"RCPLAY03") {
        return Err("Missing accepted original combat rules".into());
    }
    let size = u32::from_le_bytes(original_combat[8..12].try_into()?) as usize;
    let combat: combat::Assets = serde_json::from_slice(&original_combat[12..12 + size])?;
    let melee = combat.melee.ok_or("Missing original ForcePike rules")?;
    let catalog = defaults::load(game_data)?;
    let map = fs::read(game_data.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&map)?;
    let mut nodes = vec![];
    let mut lists = BTreeMap::new();
    let mut rejected = vec![];
    for (i, e) in pkg.exports.iter().enumerate() {
        if pkg.object_path(e.class)? != "Engine.PathNode" {
            continue;
        }
        let props = properties::read(&pkg, &map, e)?;
        let resolve = |name| catalog.instance("Engine.PathNode", &props, name, 0, "geo_01a");
        let location = match resolve("Location")?.value {
            Value::Vector(v) => v.map(f64::from),
            _ => return Err("Invalid node location".into()),
        };
        if (location[0] - playable.artillery.as_ref().unwrap().assets.spawns[0][0])
            .hypot(location[1] - playable.artillery.as_ref().unwrap().assets.spawns[0][1])
            > 4000.
        {
            continue;
        }
        if matches!(resolve("bBlocked")?.value, Value::Bool(true)) {
            continue;
        }
        let name = pkg.object_path(i as i32 + 1)?;
        let grounded = match ground(
            &playable.world,
            [location[0], location[1], location[2] + 160.],
            512.,
        ) {
            Ok(p) => p,
            Err(e) if e == "floor probe starts penetrating geometry" => None,
            Err(e) => return Err(e.into()),
        };
        let Some(position) = grounded else {
            rejected.push(name);
            continue;
        };
        let extra_cost = match resolve("ExtraCost")?.value {
            Value::Int(v) if v >= 0 => f64::from(v),
            _ => return Err("Invalid node extra cost".into()),
        };
        let mut links = vec![];
        if let Some(p) = props.values.iter().find(|p| p.name == "PathList") {
            let payload = pkg.payload(&map, e)?;
            links = properties::object_array(
                &pkg,
                &payload[p.payload_offset..p.payload_offset + p.bytes],
            )?;
        }
        lists.insert(i as i32 + 1, links);
        nodes.push(Node {
            original: i as i32 + 1,
            name,
            position,
            extra_cost,
        });
    }
    let lookup: BTreeMap<_, _> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.original, i))
        .collect();
    let mut edges = vec![];
    let mut candidates = 0;
    let mut rejected_edges = vec![];
    for (&start, links) in &lists {
        for &(index, ref name) in links {
            if index <= 0 {
                return Err("Invalid PathList reference".into());
            }
            let e = pkg
                .exports
                .get(index as usize - 1)
                .ok_or("Missing ReachSpec")?;
            if pkg.object_path(e.class)? != "Engine.ReachSpec" {
                return Err("PathList contains non-ReachSpec".into());
            }
            let props = properties::read(&pkg, &map, e)?;
            let resolve = |name| catalog.instance("Engine.ReachSpec", &props, name, 0, "geo_01a");
            let reference = |name| -> Result<i32, String> {
                match resolve(name)?.value {
                    Value::Object { index, .. } => Ok(index),
                    _ => Err("Invalid ReachSpec reference".into()),
                }
            };
            if reference("Start")? != start {
                return Err("PathList start mismatch".into());
            }
            let end = reference("End")?;
            let Some(&end) = lookup.get(&end) else {
                continue;
            };
            if !matches!(resolve("reachFlags")?.value, Value::Int(1)) {
                continue;
            }
            let mut special = false;
            for field in ["bPruned", "bJump", "bPerch", "bFly"] {
                special |= matches!(resolve(field)?.value, Value::Bool(true));
            }
            if special {
                continue;
            }
            candidates += 1;
            let start = lookup[&start];
            if !navigation::walkable_for(
                &playable.world,
                nodes[start].position,
                nodes[end].position,
                warrior_ai::EXTENT,
            )? {
                rejected_edges.push(name.clone());
                continue;
            }
            let distance = match resolve("Distance")?.value {
                Value::Int(v) if v > 0 => f64::from(v),
                _ => return Err("Invalid ReachSpec distance".into()),
            };
            edges.push(Edge {
                original: index,
                start,
                end,
                distance,
            });
        }
    }
    let number = |property| -> Result<f32, String> {
        match catalog
            .resolve("CTGame.StateGeonosianWarriorAttack", property, 0)?
            .value
        {
            Value::Float(v) => Ok(v),
            _ => Err(format!("Invalid {property}")),
        }
    };
    let nav = Assets {
        nodes,
        edges,
        lunge_distance: number("LungeDist")?,
        jump_distance: number("JumpDist")?,
        offensive_jump_probability: number("OffensiveJumpProbability")?,
        provenance: serde_json::json!({"source":"geo_01a.ctm Engine.PathNode.PathList -> Engine.ReachSpec; reachFlags exactly 1, no pruned/jump/perch/fly connections", "radius":4000,"body_extent":warrior_ai::EXTENT,"candidate_edges":candidates,"rejected_nodes":rejected,"rejected_edges":rejected_edges,"native_goals":"ctgame.dll MeleeAttack OnActivate 100339a0/PreUpdate 10033a70; Follow sleep 2 seconds and reachability interval 1 second", "adapters":"Dijkstra, 24-unit floor sampling, flat supported walking, original GroundSpeed, original BurstCount, alternating stationary clips, instantaneous turning; no original FollowLinks/root-motion/goal queue parity"}),
    };
    println!(
        "selected nodes={} edges={} candidates={}",
        nav.nodes.len(),
        nav.edges.len(),
        candidates
    );
    nav.validate()?;
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    fs::write(
        out.join("navigation-receipt.json"),
        serde_json::to_vec_pretty(&nav)?,
    )?;
    println!(
        "{} grounded nodes, {} checked directed edges / {candidates} candidates",
        nav.nodes.len(),
        nav.edges.len()
    );
    let burst = catalog.resolve("CTCharacters.GeonosianWarriorBot", "BurstCount", 0)?;
    let Value::Int(burst_count) = burst.value else {
        return Err("Invalid BurstCount".into());
    };
    let mut resolver = rc_inspect::assets::Assets::new(&[
        &game_data.join("Textures"),
        &game_data.join("Animations"),
        &game_data.join("System"),
    ])?;
    let mesh = rc_inspect::visual_assets::load(
        game_data,
        "Geonosian.ukx",
        "GeonosianWarrior",
        &[
            "ActionBreathe",
            "RunForward",
            "Death",
            "hit",
            "MeleeAttack",
            "MeleeAttack2",
            "Lunge",
            "Lunge2",
        ],
        &mut resolver,
    )?;
    let profile = warrior_ai::Assets {
        mesh: Some(mesh),
        navigation: nav,
        melee,
        burst_count,
        provenance: serde_json::json!({"burst":burst,"rules_source":"Accepted original ForcePike and Commando defense from milestone3l normal scene", "hosts":"LOS2600, last-seen memory3s, reaction0.25s, directed Dijkstra walking with original50/50/115 extent, original450 speed; stationary attack selector alternates; no general CTBot/Squad/flight parity"}),
    };
    fs::write(out.join("ai-assets.json"), serde_json::to_vec(&profile)?)?;
    let metadata = serde_json::to_vec(&profile)?;
    let mut result = b"RCPLAY11".to_vec();
    result.extend((metadata.len() as u32).to_le_bytes());
    result.extend(metadata);
    result.extend(bytes);
    bundle::decode(&result)?;
    fs::write(out.join("geo_01a-warrior-ai.rcplay"), result)?;
    Ok(())
}
fn ground(
    world: &rc_package::world_collision::StaticBodyWorld,
    position: [f64; 3],
    depth: f64,
) -> Result<Option<[f64; 3]>, String> {
    Ok(world
        .floor_contact(position, warrior_ai::EXTENT, depth, 0.7)?
        .map(|c| {
            [
                position[0],
                position[1],
                position[2] - depth * c.hit.fraction + 0.02,
            ]
        }))
}
