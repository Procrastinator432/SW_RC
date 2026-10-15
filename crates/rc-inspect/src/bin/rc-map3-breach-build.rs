use rc_game::{
    map3_breach::{Assets, Station},
    map3_escape::Door,
    map3_opening::{Action, Notify, Program},
    navigation::{Assets as Nav, Edge, Node},
};
use rc_package::{
    hull::{Hull, HullSet},
    world_collision::{BodyActor, BodyShape},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    sync::Arc,
};
fn copy<T: serde::Serialize, U: serde::de::DeserializeOwned>(
    v: &T,
) -> Result<U, serde_json::Error> {
    serde_json::from_value(serde_json::to_value(v)?)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = &rc_inspect::build_inputs::game_data()?;
    let out = rc_inspect::build_inputs::output()?;
    fs::create_dir_all(&out)?;
    let raw = fs::read(rc_inspect::build_inputs::input("map_c_guard_scene")?)?;
    let n = u32::from_le_bytes(raw[8..12].try_into()?) as usize;
    let mut map: rc_game::map_runtime::Assets = serde_json::from_slice(&raw[12..12 + n])?;
    let mut g = rc_game::bundle::decode(&raw)?;
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
    let rotation = |id| -> Result<[i32; 3], serde_json::Error> {
        serde_json::from_value(props(id)["Rotation"].clone())
    };
    let catalog = rc_inspect::defaults::load(root)?;
    let scalar = |class, key| -> Result<f64, Box<dyn std::error::Error>> {
        match catalog.resolve(class, key, 0)?.value {
            rc_package::properties::Value::Float(v) => Ok(v as f64),
            _ => Err("Missing scalar".into()),
        }
    };
    let mut stations = vec![];
    for id in [310, 261] {
        let position = point(id)?;
        let floor = rc_game::mission_motion::ground(
            &g.world,
            [position[0], position[1], position[2] + 30.],
            [1., 1., 1.],
            500.,
        )?
        .ok_or("No station floor")?[2]
            - 1.;
        let actor = row(id)["name"].as_str().unwrap().to_owned();
        if !g.world.actors.iter().any(|a| a.name == actor) {
            g.world.actors.push(BodyActor {
                name: actor.clone(),
                shape: BodyShape::Hulls(Arc::new(HullSet {
                    hulls: vec![Hull {
                        offset: 0,
                        planes: vec![],
                        bounds: [
                            [position[0] - 40., position[1] - 40., floor],
                            [position[0] + 40., position[1] + 40., floor + 240.],
                        ],
                    }],
                    missing_solid_leaves: 0,
                    empty_root_solid: false,
                })),
            });
        }
        stations.push(Station {
            original: id,
            actor,
            position,
            rotation: rotation(id)?,
            floor,
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
    let old_removed: BTreeSet<_> = ["WindowShatter_Test1", "MoverDB_GEO29", "MoverDBSlave_GEO29"]
        .into_iter()
        .flat_map(|name| faces[name].iter().copied())
        .collect();
    let mut removed = BTreeSet::new();
    let mut doors = vec![];
    for id in [120, 121] {
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
            offset: serde_json::from_value(props(id)["KeyPos"].clone())?,
            snapshot: gate.snapshot()?,
        });
    }
    // The graph is only a planning aid; every live step uses the current closed/open gates.
    for a in &mut g.world.actors {
        if [
            "WindowShatter_Test1",
            "MoverDB_GEO29",
            "MoverDBSlave_GEO29",
            "MoverDB_GEO18",
            "MoverDBSlave_GEO18",
        ]
        .contains(&a.name.as_str())
        {
            a.shape = BodyShape::Clear;
        }
    }
    let ground = |at: [f64; 3]| -> Result<Option<[f64; 3]>, String> {
        for rise in [0., 80., 160., 250.] {
            if let Some(p) = rc_game::mission_motion::ground(
                &g.world,
                [at[0], at[1], at[2] + rise],
                rc_game::squad::EXTENT,
                600.,
            )? {
                return Ok(Some(p));
            }
        }
        Ok(None)
    };
    let mut nav = Nav {
        nodes: vec![],
        edges: vec![],
        provenance: serde_json::json!({"scope":"Original navigation actor locations, grounded static walking links; geometric IDs and Dijkstra adapter, not native ReachSpecs. Planning gates clear; live controller checks current collision."}),
        lunge_distance: 0.,
        jump_distance: 0.,
        offensive_jump_probability: 0.,
    };
    for r in &rows {
        let id = r["original"].as_u64().unwrap() as u32;
        let p = props(id);
        if !p.contains_key("PathList") {
            continue;
        }
        let Ok(at) = point(id) else {
            continue;
        };
        if !(-21100. ..=-17300.).contains(&at[0]) || !(4300. ..=13300.).contains(&at[1]) {
            continue;
        }
        if let Some(position) = ground(at)? {
            nav.nodes.push(Node {
                original: id as i32,
                name: r["name"].as_str().unwrap().into(),
                position,
                extra_cost: 0.,
            });
        }
    }
    for i in 0..nav.nodes.len() {
        let mut neighbors: Vec<_> = (0..nav.nodes.len())
            .filter(|j| *j != i)
            .map(|j| (j, distance(nav.nodes[i].position, nav.nodes[j].position)))
            .filter(|(_, d)| *d < 1000.)
            .collect();
        neighbors.sort_by(|a, b| a.1.total_cmp(&b.1));
        for (j, d) in neighbors.into_iter().take(14) {
            if rc_game::squad::corridor_with_tolerance(
                &g.world,
                nav.nodes[i].position,
                nav.nodes[j].position,
                rc_game::squad::EXTENT,
                4.,
            )?
            .is_some()
            {
                nav.edges.push(Edge {
                    original: nav.edges.len() as i32 + 1,
                    start: i,
                    end: j,
                    distance: d,
                });
            }
        }
    }
    nav.validate()?;
    let spots = [
        ground(point(183)?)?.ok_or("No doorman spot")?,
        ground(point(184)?)?.ok_or("No grenadier spot")?,
        ground(point(185)?)?.ok_or("No coverman spot")?,
    ];
    let names = [
        "DoorBreachLoudSM1Doorman",
        "DoorBreachLoudSM2Grenademan",
        "DoorBreachLoudSM3Coverman",
    ];
    let clips: Vec<rc_game::visual::Clip> = serde_json::from_slice(&fs::read(
        rc_inspect::build_inputs::input("map_c_breach_clips")?,
    )?)?;
    map.squad_mesh.as_mut().unwrap().clips.push(copy(
        clips
            .iter()
            .find(|c| c.sequence.name.name == names[0])
            .unwrap(),
    )?);
    map.fixer_mesh.as_mut().unwrap().clips.push(copy(
        clips
            .iter()
            .find(|c| c.sequence.name.name == names[1])
            .unwrap(),
    )?);
    map.opening.as_mut().unwrap().actors[2]
        .mesh
        .as_mut()
        .unwrap()
        .clips
        .push(copy(
            clips
                .iter()
                .find(|c| c.sequence.name.name == names[2])
                .unwrap(),
        )?);
    let mut resolver = rc_inspect::assets::Assets::new(&[
        &root.join("Textures"),
        &root.join("Animations"),
        &root.join("System"),
    ])?;
    let mut bacta = rc_inspect::visual_assets::load_from_animation(
        root,
        "BactaDispensers.ukx",
        "BactaDispenserGEO",
        &["Holster", "Load"],
        &mut resolver,
        Some("BactaDispensers.BactaDispenserGEOSet"),
    )?;
    for material in &mut bacta.materials {
        let width = material.width.min(128);
        let height = material.height.min(128);
        let mut pixels = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                pixels.push(
                    material.pixels[(y * material.height / height) * material.width
                        + x * material.width / width],
                );
            }
        }
        material.width = width;
        material.height = height;
        material.pixels = pixels;
    }
    let mut sounds = vec![vec![], vec![], vec![]];
    for (role, notify, path) in [
        (0, 312, "GEN_Sound.Interface.int_GEN_doorBreachSet_01"),
        (1, 307, "GEN_Sound.Foley_Movement.fol_GEN_ccFol_drBreach_01"),
        (0, 309, "GEN_Sound.Impacts_Explos.exp_GEN_smallElec_01"),
    ] {
        let i = map.sounds.len();
        map.sounds
            .push(rc_inspect::mission_assets::sound(root, path)?.0);
        sounds[role].push(Notify {
            original: notify,
            sound: i,
        });
    }
    let b = Assets {
        balcony: None,
        stations,
        bacta_mesh: Some(bacta),
        heal_duration: scalar("CTGame.BactaDispenser", "BactaRate")?,
        heal_range: scalar("CTGame.BactaDispenser", "PlayerRange")?,
        navigation: nav,
        spots,
        rotations: [rotation(183)?, rotation(184)?, rotation(185)?],
        clips: names.iter().map(|s| s.to_string()).collect(),
        sounds,
        doors,
        move_time: scalar("MarkerProperties.MoverDB_GEO", "MoveTimes")?,
        provenance: serde_json::json!({"original_stations":[310,261],"original_marker":122,"original_controller":125,"original_spots":[183,184,185],"roles":["Scorch/doorman","Fixer/grenadier","Sev/coverman"],"scope":"Selected native animation calculations and original notify/events/positions; bounded squad assignment, grounded navigation, healing (BactaRate interpreted as full-heal duration), root-body sweeps and door interpolation. Native goal/cue/activation/Karma/grenade damage parity pending"}),
    };
    let node_count = b.navigation.nodes.len();
    let edge_count = b.navigation.edges.len();
    map.opening
        .as_mut()
        .unwrap()
        .planning
        .as_mut()
        .unwrap()
        .guard
        .as_mut()
        .unwrap()
        .breach = Some(Box::new(b));
    let mut actions = vec![];
    for (id, op, text, role) in [
        (885, "wait", "SunFacRoomDoorOpening", 0),
        (886, "emit", "MissionObjectives", 0),
        (887, "objective", "", 4),
        (888, "emit", "ContinueToAARoom", 0),
        (889, "emit", "SpawnAARoom", 0),
        (890, "emit", "TrackSunFacInsideShield", 0),
    ] {
        actions.push(Action {
            original: id,
            op: op.into(),
            text: text.into(),
            role,
            seconds: 0.,
        });
    }
    map.opening.as_mut().unwrap().scripts.push(Program {
        original: 140,
        actions,
    });
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
    let metadata = serde_json::to_vec(&map)?;
    let metadata_bytes = metadata.len();
    let mut bytes = b"RCPLAY14".to_vec();
    bytes.extend((metadata.len() as u32).to_le_bytes());
    bytes.extend(metadata);
    bytes.extend(&base[..12 + vn]);
    bytes.extend(&static_base[..12 + sn]);
    bytes.extend(scene.snapshot()?);
    println!("Proposed metadata bytes: {metadata_bytes}");
    let game = rc_game::bundle::decode(&bytes)?;
    fs::write(out.join("geo_01c.rcplay"), &bytes)?;
    fs::write(
        out.join("build-receipt.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"bytes":bytes.len(),"metadata_bytes":metadata_bytes,"navigation_nodes":node_count,"navigation_edges":edge_count,"spots":spots,"door_faces":removed.len(),"sounds":map.sounds.len(),"bacta_bounds":game.map.as_ref().unwrap().opening.as_ref().unwrap().planning.as_ref().unwrap().guard.as_ref().unwrap().breach.as_ref().unwrap().bacta.rest_bounds,"controller":row(125),"marker":row(122)}),
        )?,
    )?;
    println!(
        "C breach: {} bytes; metadata {}; {} nodes/{} links",
        bytes.len(),
        metadata_bytes,
        node_count,
        edge_count
    );
    Ok(())
}
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}
