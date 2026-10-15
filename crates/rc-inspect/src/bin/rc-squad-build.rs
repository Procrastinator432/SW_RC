//! Bind selected enemy maneuvers and Delta62 to an exact accepted mission scene.
use rc_game::{bundle, mission_tactics, navigation, squad};
use rc_package::{mesh_query::ActorTransform, properties::Value};
use std::{env, fs, path::Path};
fn squad_graph(
    root: &Path,
    game: &rc_game::Game,
    cat: &rc_package::defaults::Catalog,
    extent: [f64; 3],
) -> Result<navigation::Assets, Box<dyn std::error::Error>> {
    let bytes = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let pkg = rc_package::read_package(&bytes)?;
    let mut nodes = vec![];
    let mut links = vec![];
    let mut lookup = std::collections::BTreeMap::new();
    for (i, e) in pkg.exports.iter().enumerate() {
        if pkg.object_path(e.class)? != "Engine.PathNode" {
            continue;
        }
        let props = rc_package::properties::read(&pkg, &bytes, e)?;
        let resolve = |name| cat.instance("Engine.PathNode", &props, name, 0, "geo_01a");
        if matches!(resolve("bBlocked")?.value, Value::Bool(true)) {
            continue;
        }
        let Value::Vector(location) = resolve("Location")?.value else {
            return Err("Invalid PathNode location".into());
        };
        let Some(position) = rc_game::mission_motion::ground(
            &game.world,
            [
                location[0] as f64,
                location[1] as f64,
                location[2] as f64 + 160.,
            ],
            extent,
            512.,
        )?
        else {
            continue;
        };
        let Value::Int(extra_cost) = resolve("ExtraCost")?.value else {
            return Err("Invalid ExtraCost".into());
        };
        let mut path = vec![];
        if let Some(p) = props.values.iter().find(|p| p.name == "PathList") {
            let payload = pkg.payload(&bytes, e)?;
            path = rc_package::properties::object_array(
                &pkg,
                &payload[p.payload_offset..p.payload_offset + p.bytes],
            )?;
        }
        lookup.insert(i as i32 + 1, nodes.len());
        links.push((i as i32 + 1, path));
        nodes.push(navigation::Node {
            original: i as i32 + 1,
            name: pkg.object_path(i as i32 + 1)?,
            position,
            extra_cost: extra_cost as f64,
        });
    }
    let mut edges = vec![];
    for (start, list) in links {
        for (index, _) in list {
            let e = &pkg.exports[index as usize - 1];
            if pkg.object_path(e.class)? != "Engine.ReachSpec" {
                return Err("Invalid ReachSpec".into());
            }
            let props = rc_package::properties::read(&pkg, &bytes, e)?;
            let resolve = |name| cat.instance("Engine.ReachSpec", &props, name, 0, "geo_01a");
            let Value::Object { index: end, .. } = resolve("End")?.value else {
                return Err("Invalid ReachSpec End".into());
            };
            let Some(&end) = lookup.get(&end) else {
                continue;
            };
            if !matches!(resolve("reachFlags")?.value,Value::Int(v) if v&1!=0) {
                continue;
            }
            let mut reject = false;
            for name in ["bPruned", "bJump", "bPerch", "bFly"] {
                reject |= matches!(resolve(name)?.value, Value::Bool(true));
            }
            if reject {
                continue;
            }
            let start = lookup[&start];
            if squad::corridor(
                &game.world,
                nodes[start].position,
                nodes[end].position,
                extent,
            )?
            .is_none()
            {
                continue;
            }
            let Value::Int(distance) = resolve("Distance")?.value else {
                return Err("Invalid ReachSpec distance".into());
            };
            edges.push(navigation::Edge {
                original: index,
                start,
                end,
                distance: distance as f64,
            });
        }
    }
    let original = &game.warrior_ai.as_ref().unwrap().navigation;
    let graph = navigation::Assets {
        nodes,
        edges,
        lunge_distance: original.lunge_distance,
        jump_distance: original.jump_distance,
        offensive_jump_probability: original.offensive_jump_probability,
        provenance: serde_json::json!({"source":"Entire geo_01a PathNode.PathList/ReachSpec walking subset","extent":extent,"host":"16-unit existing body-solver corridors, 35-unit up/down support, body sweeps, Dijkstra; no native FollowLinks parity"}),
    };
    graph.validate()?;
    Ok(graph)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-squad-build GAME_DATA AI_SCENE OUT".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let base = fs::read(&args[2])?;
    if !base.starts_with(b"RCPLAY11") {
        return Err("Expected accepted Warrior AI scene".into());
    }
    let mut game = bundle::decode(&base)?;
    game.world
        .actors
        .retain(|a| !a.name.starts_with("Dynamic."));
    let cat = rc_inspect::defaults::load(root)?;
    let number = |class, name| -> Result<f64, String> {
        match cat.resolve(class, name, 0)?.value {
            Value::Float(v) => Ok(v as f64),
            Value::Int(v) => Ok(v as f64),
            _ => Err(format!("Invalid {class}.{name}")),
        }
    };
    let mut resolver = rc_inspect::assets::Assets::new(&[
        &root.join("Textures"),
        &root.join("Animations"),
        &root.join("System"),
    ])?;
    let enemy = rc_inspect::visual_assets::load(
        root,
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
            "TakeOff",
            "Land",
            "FallBreathe",
        ],
        &mut resolver,
    )?;
    let mut scorch = rc_inspect::visual_assets::load(
        root,
        "Clone.ukx",
        "CloneCommando",
        &[
            "ActionBreathe",
            "RunForward",
            "GestureSignalOK",
            "DemolitionSetChargeStart",
            "DemolitionSetChargeLoop",
            "DemolitionSetChargeStop",
            "Fire",
            "CrouchBreathe",
            "CrouchForward",
            "ImmobileBreathe",
            "ImmobileGetUp",
            "PlayerImmobileGetUp",
            "HealImmobileStart",
            "HealImmobileLoop",
            "HealImmobileStop",
        ],
        &mut resolver,
    )?;
    scorch.materials = serde_json::from_value(serde_json::to_value(
        &game.rendezvous.as_ref().unwrap().mesh.mesh.materials,
    )?)?;
    let combat = fs::read("analysis/milestone3l/geo_01a-normal.rcplay")?;
    let size = u32::from_le_bytes(combat[8..12].try_into()?) as usize;
    let old: rc_game::combat::Assets = serde_json::from_slice(&combat[12..12 + size])?;
    let melee = old.melee.as_ref().ok_or("Missing accepted damage report")?;
    let jump = old.jump.ok_or("Missing accepted JumpDest bindings")?;
    let nav = squad_graph(root, &game, &cat, squad::EXTENT)?;
    fs::write(
        out.join("squad-navigation.json"),
        serde_json::to_vec_pretty(&nav)?,
    )?;
    let mut materials = vec![];
    let weapon = rc_inspect::mission_assets::mesh_scene(
        root,
        "Weapons.CloneWeapons.DC17mBlaster",
        &ActorTransform {
            location: [0.; 3],
            rotation: [0; 3],
            scale: [1.; 3],
            pivot: [0.; 3],
        },
        &cat,
        &mut materials,
    )?;
    let mut audio = vec![];
    let (sound, _) = rc_inspect::mission_assets::sound_variant(
        root,
        "GEN_Sound.Weapons.wep_GEN_dc17Fire_02",
        &mut audio,
    )?;
    let class = "CTCharacters.Commando62Delta";
    let crouch_nav = squad_graph(root, &game, &cat, [40., 40., 56.])?;
    let ids = &melee.provenance["class_ids"];
    let arrays = &melee.provenance["damage_report"]["raw_classes"]
        ["properties.DV_OrganicCTSquadDamage"]["decoded_arrays"];
    let mut include = vec![];
    for row in arrays["IncludeTypes"]
        .as_array()
        .ok_or("Missing squad variance")?
    {
        let values = row["values"].as_array().ok_or("Missing variance fields")?;
        let field = |name| {
            values
                .iter()
                .find(|v| v["name"] == name)
                .map(|v| &v["value"]["value"])
                .ok_or("Missing variance field")
        };
        let object = field("DamageTypeClass")?;
        let class = if object["index"] == 0 {
            None
        } else {
            Some(
                ids[object["path"]
                    .as_str()
                    .ok_or("Bad class path")?
                    .to_lowercase()]
                .as_u64()
                .ok_or("Missing class id")? as u32,
            )
        };
        include.push(rc_game::melee::DamageRule {
            class,
            multiplier: field("Multiplier")?
                .as_f64()
                .ok_or("Bad variance multiplier")? as f32,
        });
    }
    let mut exclude = vec![];
    for row in arrays["ExcludeTypes"]
        .as_array()
        .ok_or("Missing squad excludes")?
    {
        exclude.push(Some(
            ids[row[1].as_str().ok_or("Bad exclude")?.to_lowercase()]
                .as_u64()
                .ok_or("Missing excluded class id")? as u32,
        ));
    }
    let defense = rc_game::melee::Defense {
        armor: number(class, "ArmorFactor")? as f32,
        include,
        exclude,
    };
    let squad = squad::Assets {
        defense,
        shield_delay: number(class, "ShieldRechargeDelay")?,
        shield_rate: number(class, "ShieldRechargeRate")?,
        crouch_navigation: crouch_nav,
        crouch_ratio: number(class, "CrouchSpeedRatio")?,
        mesh: Some(scorch),
        navigation: nav,
        weapon_snapshot: weapon.snapshot()?,
        sounds: vec![sound],
        fire_sound: 0,
        health: number(class, "Health")? as u32,
        max_health: number(class, "MaxHealth")? as u32,
        shields: number(class, "Shields")? as f32,
        max_shields: number(class, "MaxShields")? as f32,
        ground_speed: number(class, "GroundSpeed")?,
        damage: number("CTInventory.DC17BlasterProj", "Damage")? as u32,
        shot_delay: number("CTInventory.DC17mBlaster", "ShotDelay")?,
        projectile_speed: number("CTInventory.DC17BlasterProj", "Speed")?,
        projectile_life: number("CTInventory.DC17BlasterProj", "LifeSpan")?,
        revive_duration: number("CTGame.MarkerReviveOther", "Duration")?,
        provenance: serde_json::json!({"weapon":"DC17mBlasterAttachment.StaticMesh; weaponAttach_R original bone; selected first sound variant", "materials":materials,"audio":audio,"revive":"MarkerReviveOther.Duration, Pawn.Reviving Health=1 then MaxHealth/2 after GetUp; explicit selected mission host, no full native goal queue", "orders":"Use edge aimed at Warrior focus; near Scorch toggle hold/follow after demolition; hold Use to revive Scorch; Use when incapacitated requests aid", "limits":"Selected Scorch only, no full four-member campaign squad, bounded navigation, no native morale/cover/grenade planner"}),
    };
    let assets = mission_tactics::Assets {
        enemy_mesh: Some(enemy),
        jump,
        squad,
        provenance: serde_json::json!({"original":"Geonosian attack priorities and conditional random draws; raw original-size root curves and JumpDest binding","adapters":"swept ballistic motion, flat path steering, static occlusion, nearest visible player/Scorch selection"}),
    };
    let metadata = serde_json::to_vec(&assets)?;
    let mut result = b"RCPLAY12".to_vec();
    result.extend((metadata.len() as u32).to_le_bytes());
    result.extend(metadata);
    result.extend(base);
    let decoded = bundle::decode(&result)?;
    fs::write(out.join("geo_01a-squad.rcplay"), &result)?;
    fs::write(
        out.join("build-validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"bytes":result.len(),"sounds":decoded.mission.as_ref().unwrap().sounds.len(),"nodes":assets.squad.navigation.nodes.len(),"edges":assets.squad.navigation.edges.len(),"enemy_clips":decoded.artillery.as_ref().unwrap().warrior.mesh.clips.len(),"scorch_clips":decoded.rendezvous.as_ref().unwrap().mesh.mesh.clips.len()}),
        )?,
    )?;
    println!(
        "RCPLAY12 {} bytes, {} sounds",
        result.len(),
        decoded.mission.as_ref().unwrap().sounds.len()
    );
    Ok(())
}
