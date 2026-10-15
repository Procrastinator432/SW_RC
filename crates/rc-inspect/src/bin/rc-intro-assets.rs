//! Resolve the original landing actors, script defaults, ordered targets and audio.
use rc_package::{
    properties::{self, Value},
    read_package,
};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args_os().collect();
    if a.len() != 4 {
        return Err("usage: rc-intro-assets GAME_DATA SCENE OUT".into());
    }
    let root = Path::new(&a[1]);
    let out = Path::new(&a[3]);
    fs::create_dir_all(out)?;
    let data = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let p = read_package(&data)?;
    let cat = rc_inspect::defaults::load(root)?;
    let slots = rc_package::level::actor_references(&p, &data)?;
    let value = |id: usize, name: &str| -> Result<Value, String> {
        let e = &p.exports[id - 1];
        let props = properties::read(&p, &data, e)?;
        if let Some(v) = props.values.iter().find(|p| p.name == name) {
            Ok(v.value.clone())
        } else {
            Ok(cat
                .instance(&p.object_path(e.class)?, &props, name, 0, "geo_01a")?
                .value)
        }
    };
    let mut targets = vec![];
    for (slot, id) in slots.iter().enumerate() {
        if *id <= 0 {
            continue;
        }
        let tag = value(*id as usize, "Tag");
        if matches!(&tag,Ok(Value::Name(s)) if s=="GunshipWarrior01") {
            targets.push(serde_json::json!({"slot":slot,"original":id,"location":value(*id as usize,"Location")?,"rotation":value(*id as usize,"Rotation")?}));
        }
    }
    let mut actions = vec![];
    for script in [13, 197] {
        let e = &p.exports[script - 1];
        let props = properties::read(&p, &data, e)?;
        let f = props
            .values
            .iter()
            .find(|p| p.name == "Actions")
            .ok_or("Missing actions")?;
        let payload = p.payload(&data, e)?;
        for (id, _) in
            properties::object_array(&p, &payload[f.payload_offset..f.payload_offset + f.bytes])?
        {
            let e = &p.exports[id as usize - 1];
            let props = properties::read(&p, &data, e)?;
            let mut resolved = serde_json::Map::new();
            for name in [
                "BaseAnim",
                "bLoopAnim",
                "bWaitUntilAnimEnd",
                "AnimRate",
                "AnimIterations",
                "StartFrame",
                "Sound",
                "SoundSourceTag",
                "bWaitForFinish",
                "PauseTime",
                "ExternalEvent",
                "DestinationTag",
                "ActorToTeleportTag",
                "DestroyTag",
                "bCollideActors",
                "bCollideWorld",
                "bBlockActors",
                "bBlockPlayers",
                "bBlockZeroExtentTraces",
                "bBlockNonZeroExtentTraces",
                "bNoCheck",
                "WalkToPoint",
                "OrientToPoint",
            ] {
                if let Ok(v) = value(id as usize, name) {
                    resolved.insert(name.into(), serde_json::to_value(v)?);
                }
            }
            actions.push(serde_json::json!({"script":script,"original":id,"class":p.object_path(e.class)?,"properties":props,"resolved":resolved}));
        }
    }
    let mut actors = vec![];
    for id in [452, 453, 530, 413] {
        let mut resolved = serde_json::Map::new();
        for name in [
            "Tag",
            "Location",
            "Rotation",
            "Physics",
            "GroundSpeed",
            "AirSpeed",
            "CollisionHeight",
            "CollisionRadius",
            "bCollideActors",
            "bCollideWorld",
            "bBlockActors",
            "Mesh",
            "Skins",
        ] {
            if let Ok(v) = value(id, name) {
                resolved.insert(name.into(), serde_json::to_value(v)?);
            }
        }
        actors.push(serde_json::json!({"original":id,"resolved":resolved}));
    }
    let mut sound_receipts = vec![];
    let mut sounds = vec![];
    let mut audio_bindings = vec![];
    for action in actions
        .iter()
        .filter(|a| a["class"] == "Gameplay.ACTION_PlaySound")
    {
        let path = action["resolved"]["Sound"]["value"]["path"]
            .as_str()
            .ok_or("Missing original sound")?;
        let (sound, seconds) =
            rc_inspect::mission_assets::sound_variant(root, path, &mut sound_receipts)?;
        audio_bindings.push(serde_json::json!({"original":action["original"],"sound_index":sounds.len(),"duration":seconds,"wait":action["resolved"]["bWaitForFinish"],"owner":action["resolved"]["SoundSourceTag"]}));
        sounds.push(sound);
    }
    let mut resolver = rc_inspect::assets::Assets::new(&[
        &root.join("Textures"),
        &root.join("Animations"),
        &root.join("System"),
    ])?;
    let mut meshes = vec![];
    let mut root_bounds = vec![];
    for (id, file, object, set) in [
        (452, "Clone.ukx", "CloneTrooper", "Clone.cGEO01CloneSet"),
        (
            530,
            "Geonosian.ukx",
            "GeonosianWarrior",
            "Geonosian.cGEO01WarriorSet",
        ),
    ] {
        let mesh = rc_inspect::visual_assets::load_from_animation(
            root,
            file,
            object,
            &["WarriorIntroBreathe", "WarriorIntro"],
            &mut resolver,
            Some(set),
        )?;
        let clip = mesh
            .clips
            .iter()
            .find(|c| c.sequence.name.name == "WarriorIntro")
            .ok_or("Missing intro")?;
        let d = fs::read(root.join("Animations").join(file))?;
        let pkg = read_package(&d)?;
        for n in &clip.sequence.notifies {
            let e = &pkg.exports[n.object_index as usize - 1];
            let props = properties::read(&pkg, &d, e)?;
            if pkg.object_path(e.class)? != "Engine.AnimNotify_Sound" {
                return Err("Unknown intro notify".into());
            }
            let path = props
                .values
                .iter()
                .find_map(|v| {
                    if v.name == "Sound" {
                        if let Value::Object { path, .. } = &v.value {
                            Some(path)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .ok_or("Missing intro notify sound")?;
            let (sound, seconds) =
                rc_inspect::mission_assets::sound_variant(root, path, &mut sound_receipts)?;
            audio_bindings.push(serde_json::json!({"actor":id,"original":n.object_index,"time_bits":n.time_bits,"sound_index":sounds.len(),"duration":seconds}));
            sounds.push(sound);
        }
        fs::write(
            out.join(format!("actor-{id}-mesh.json")),
            serde_json::to_vec(&mesh)?,
        )?;
        let prepared = mesh.prepare_reference()?;
        let c = prepared.clip("WarriorIntro")?;
        let seq = &prepared.mesh.clips[c].sequence;
        let Value::Vector(loc) = value(id, "Location")? else {
            return Err("Missing actor location".into());
        };
        let Value::Rotator(rot) = value(id, "Rotation")? else {
            return Err("Missing actor rotation".into());
        };
        let actor: [u32; 16] =
            rc_package::mesh_transform::rotation_matrix(rot, loc.map(f32::to_bits))?
                .concat()
                .try_into()
                .unwrap();
        let first = prepared.root_matrix(c, 0.)?;
        let anchor = rc_package::root_motion::compose_relative(
            rc_package::skeletal_matrix_inverse::inverse_matrix(first),
            actor,
        )?;
        let mut rows = vec![];
        for i in 0..seq.frames {
            let f = i as f32 / seq.frames as f32;
            let m = rc_package::root_motion::compose_relative(prepared.root_matrix(c, f)?, anchor)?;
            rows.push(serde_json::json!({"frame":i,"matrix":m,"position":[f32::from_bits(m[12]),f32::from_bits(m[13]),f32::from_bits(m[14])]}));
        }
        meshes.push(serde_json::json!({"original":id,"sequence":prepared.mesh.clips[c].source,"frames":seq.frames,"rate":f32::from_bits(seq.rate_bits),"first_root":first,"actor":actor,"entry_anchor":anchor,"rows":rows}));
        for f in [0., 0.5, 1. - 1. / seq.frames as f32] {
            let geometry = prepared.geometry(&prepared.channel(c, f)?)?;
            let matrix =
                rc_package::root_motion::compose_relative(prepared.root_matrix(c, f)?, anchor)?
                    .map(f32::from_bits);
            let mut bounds = [[f32::INFINITY; 3], [f32::NEG_INFINITY; 3]];
            for v in geometry.triangles.iter().flat_map(|t| t.points) {
                for j in 0..3 {
                    let world = v[0] * matrix[j]
                        + v[1] * matrix[4 + j]
                        + v[2] * matrix[8 + j]
                        + matrix[12 + j];
                    bounds[0][j] = bounds[0][j].min(world);
                    bounds[1][j] = bounds[1][j].max(world);
                }
            }
            root_bounds.push(serde_json::json!({"original":id,"frame":f,"bounds":bounds}));
        }
    }
    let game = rc_game::bundle::decode(&fs::read(&a[2])?)?;
    let sound = |id| {
        audio_bindings
            .iter()
            .find(|a| a["original"] == id)
            .ok_or("Missing script voice")
    };
    let action = |id| {
        actions
            .iter()
            .find(|a| a["original"] == id)
            .ok_or("Missing script action")
    };
    let flow = rc_game::intro_flow::Flow {
        first_voice_seconds: sound(1259)?["duration"]
            .as_f64()
            .ok_or("Missing duration")?,
        second_voice_seconds: sound(1260)?["duration"]
            .as_f64()
            .ok_or("Missing duration")?,
        second_voice_wait: sound(1260)?["wait"]["value"]
            .as_bool()
            .ok_or("Missing wait flag")?,
        destroy_delay: action(1261)?["resolved"]["PauseTime"]["value"]
            .as_f64()
            .ok_or("Missing destruction delay")?,
        teleport_destination: targets.first().ok_or("Missing teleport receiver")?["original"]
            .as_u64()
            .unwrap() as u32,
        move_destination: slots.iter().copied().find(|id| *id>0 && matches!(value(*id as usize,"Tag"),Ok(Value::Name(s)) if s=="KillIntroWarrior")).ok_or("Missing MoveToPoint destination")? as u32,
        teleport_no_check: action(1768)?["resolved"]["bNoCheck"]["value"]
            .as_bool()
            .ok_or("Missing collision check")?,
        world_collision: action(1764)?["resolved"]["bCollideWorld"]["value"]
            .as_bool()
            .ok_or("Missing collision flag")?,
    };
    flow.validate()?;
    fs::write(out.join("flow.json"), serde_json::to_vec_pretty(&flow)?)?;
    let loc = |v: &serde_json::Value| -> Result<[f64; 3], Box<dyn std::error::Error>> {
        Ok(serde_json::from_value(v["value"].clone())?)
    };
    let first = targets.first().ok_or("Missing teleport targets")?;
    let from = loc(&first["location"])?;
    let kill = actors.iter().find(|v| v["original"] == 413).unwrap();
    let to = loc(&kill["resolved"]["Location"])?;
    let sweep = game.world.sweep_motion(from, to, [50., 50., 115.])?;
    fs::write(
        out.join("bindings.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"actions":actions,"actors":actors,"teleport_targets_in_level_order":targets,"selected_teleport":first,"audio":audio_bindings,"sound_receipts":sound_receipts,"move_to_kill_direct_probe":sweep,"move_to_kill_goal_native":false,"root_anchor_boundary":"inverse(first root)*actor; scalar relative composer reused for entry product, not bit-exact PostInit composition proof"}),
        )?,
    )?;
    fs::write(
        out.join("animation-curves.json"),
        serde_json::to_vec_pretty(&meshes)?,
    )?;
    fs::write(
        out.join("animation-bounds.json"),
        serde_json::to_vec_pretty(&root_bounds)?,
    )?;
    fs::write(out.join("sounds.json"), serde_json::to_vec(&sounds)?)?;
    println!(
        "{} ordered teleport receivers; selected {}; {} original sound bindings",
        targets.len(),
        first["original"],
        sounds.len()
    );
    Ok(())
}
