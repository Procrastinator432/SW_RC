//! Inspect music assignments from the installed original, without invented defaults.
use rc_package::{properties, read_package};
use std::{env, fs, path::Path};
fn index(bytes: &[u8], at: &mut usize) -> Result<i32, String> {
    let b = *bytes.get(*at).ok_or("Truncated compact music index")?;
    *at += 1;
    let mut v = i32::from(b & 63);
    let mut more = b & 64 != 0;
    let mut shift = 6;
    while more {
        let b = *bytes.get(*at).ok_or("Truncated music index")?;
        *at += 1;
        if shift > 27 {
            return Err("Music index overflow".into());
        }
        v |= i32::from(b & 127) << shift;
        shift += 7;
        more = b & 128 != 0;
    }
    Ok(if b & 128 != 0 { -v } else { v })
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() < 3 {
        return Err("usage: rc-music-audit GAME_DATA OUT.json [MAP.ctm ...]".into());
    }
    let root = Path::new(&args[1]);
    let cat = rc_inspect::defaults::load(root)?;
    let mut rows = vec![];
    let mut files = if args.len() == 3 {
        vec!["Maps/geo_01a.ctm", "Maps/geo_01b.ctm", "Maps/geo_01c.ctm"]
    } else {
        args[3..].iter().map(String::as_str).collect()
    };
    let mut music_files: Vec<_> = fs::read_dir(root.join("Sounds"))?
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter(|name| name.to_lowercase().ends_with("_music.uax"))
        .map(|name| format!("Sounds/{name}"))
        .collect();
    music_files.sort_by_key(|name| name.to_lowercase());
    files.extend(music_files.iter().map(String::as_str));
    for file in files {
        let data = fs::read(root.join(file))?;
        let pkg = read_package(&data)?;
        for (i, e) in pkg.exports.iter().enumerate() {
            let class = pkg.object_path(e.class)?;
            let props = match properties::read(&pkg, &data, e) {
                Ok(p) => p,
                Err(_) => continue,
            };
            if !(file.starts_with("Sounds/")
                || class.to_lowercase().contains("imuse")
                || class.contains("Battle")
                || class.contains("Music")
                || class.contains("Flourish")
                || class == "Engine.LevelInfo"
                || class == "Gameplay.ACTION_SetCollision"
                || class == "Gameplay.ACTION_WaitForEvent"
                || class == "Gameplay.ACTION_WaitForTimer")
                && !(args.len() > 3
                    && (class.contains("ScriptedTrigger") || class.contains("ScriptedSequence")))
            {
                continue;
            }
            let zone = if cat
                .derives_from(&class, "Engine.EngineBattleVolume")
                .unwrap_or(false)
            {
                let shape = rc_inspect::volumes::shape(&pkg, &data, &cat, &class, &props, file)?;
                let mut factories = vec![];
                let mut forever = vec![];
                for (j, f) in pkg.exports.iter().enumerate() {
                    let fc = pkg.object_path(f.class)?;
                    if !cat.derives_from(&fc, "CTGame.PawnFactory").unwrap_or(false) {
                        continue;
                    }
                    let fp = properties::read(&pkg, &data, f)?;
                    let properties::Value::Vector(at) =
                        cat.instance(&fc, &fp, "Location", 0, file)?.value
                    else {
                        return Err("Factory location".into());
                    };
                    if shape.contains(at)? {
                        factories.push(j + 1);
                        if matches!(
                            cat.instance(&fc, &fp, "bGenerateForever", 0, file)?.value,
                            properties::Value::Bool(true)
                        ) {
                            forever.push(j + 1);
                        }
                    }
                }
                if let rc_package::volumes::VolumeShape::Brush {
                    bsp,
                    solid,
                    transform,
                } = shape
                {
                    Some(
                        serde_json::json!({"original":i+1,"name":pkg.object_path(i as i32+1)?,"nodes":bsp.nodes.iter().map(|n|serde_json::json!({"plane":n.plane,"front":n.front,"back":n.back,"flags":n.flags,"vertex_count":n.vertex_count})).collect::<Vec<_>>(),"root_outside":solid.root_outside,"transform":{"location":transform.location,"rotation":transform.rotation,"scale":transform.scale,"pivot":transform.pivot},"factories":factories,"forever":forever}),
                    )
                } else {
                    return Err("BattleVolume without a brush".into());
                }
            } else {
                None
            };
            let mut resolved = serde_json::Map::new();
            for name in [
                "onOff",
                "Music",
                "MusicState",
                "OverrideFadeDefaults",
                "FadeInTime",
                "FadeOutTime",
                "FadeInType",
                "FadeOutType",
                "PauseTime",
                "ExternalEvent",
                "Event",
                "iMuseState",
                "iMuseOverlays",
                "BattleMusic",
                "BattleBaddieThreshold",
                "BattleCriteria",
                "BattleRadiusInner",
                "BattleRadiusOuter",
                "radiusInner",
                "radiusOuter",
                "multiSound",
                "BattleWonMusic",
                "BattleLostMusic",
                "Looping",
                "FirstSound",
                "BattleContext",
                "MinCampaign",
                "MaxCampaign",
                "Likelihood",
                "criteria",
                "baddieCount",
            ] {
                if let Ok(v) = cat.instance(&class, &props, name, 0, file) {
                    resolved.insert(name.into(), serde_json::to_value(v)?);
                }
            }
            let native = if file.starts_with("Sounds/") {
                pkg.payload(&data, e)?
                    .get(props.native_offset..)
                    .unwrap_or_default()
                    .to_vec()
            } else {
                vec![]
            };
            let mut children = vec![];
            if class == "Engine.SoundMultiple" {
                let mut at = 0;
                let count = index(&native, &mut at)?;
                if !(1..=64).contains(&count) {
                    return Err("Music group count".into());
                }
                for _ in 0..count {
                    let id = index(&native, &mut at)?;
                    let path = pkg.object_path(id)?;
                    children.push(if id > 0 {
                        format!(
                            "{}.{}",
                            Path::new(file).file_stem().unwrap().to_str().unwrap(),
                            path
                        )
                    } else {
                        path
                    });
                }
                if at + 1 != native.len() {
                    return Err("Music group tail".into());
                }
            }
            rows.push(serde_json::json!({"file":file,"index":i+1,"path":pkg.object_path(i as i32+1)?,"class":class,"properties":props,"resolved":resolved,"native":native,"children":children,"zone":zone}));
        }
    }
    let mut initial = serde_json::Map::new();
    for field in [
        "mCurrentRoom",
        "mCurrentOverlay",
        "mCurrentEpisode",
        "CurrentBattleTrio",
        "mActiveBattleCueID",
        "mbBattleEngineEnabled",
        "mbPlayerDown",
    ] {
        initial.insert(
            field.into(),
            serde_json::to_value(cat.resolve("Engine.IMuseStateInfo", field, 0)?)?,
        );
    }
    rows.push(serde_json::json!({"file":"System/engine.u","class":"Engine.IMuseStateInfo.defaults","resolved":initial}));
    fs::write(&args[2], serde_json::to_vec_pretty(&rows)?)?;
    println!("{} original music records", rows.len());
    Ok(())
}
