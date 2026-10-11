//! Inspect original detonator defaults and geo_01a gun-position script bindings.
use rc_package::{properties, read_package};
use std::{env, fs, path::Path};
fn sound(root: &Path, path: &str) -> Result<(Vec<u8>, f64), Box<dyn std::error::Error>> {
    let (package, object) = path.split_once('.').ok_or("Invalid sound path")?;
    let data = fs::read(root.join("Sounds").join(format!("{package}.uax")))?;
    let pkg = read_package(&data)?;
    let e = pkg
        .exports
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            pkg.object_path(i as i32 + 1)
                .is_ok_and(|p| p.eq_ignore_ascii_case(object))
                .then_some(e)
        })
        .ok_or("Missing original sound")?;
    let payload = pkg.payload(&data, e)?;
    let at = payload
        .windows(4)
        .position(|v| v == b"RIFF")
        .ok_or("No wave")?;
    let count = u32::from_le_bytes(
        payload
            .get(at + 4..at + 8)
            .ok_or("Short wave")?
            .try_into()?,
    ) as usize
        + 8;
    let wave = payload
        .get(at..at + count)
        .ok_or("Truncated wave")?
        .to_vec();
    rc_game::combat::validate_wave(&wave)?;
    let (mut at, mut rate, mut size) = (12, None, None);
    while at + 8 <= wave.len() {
        let n = u32::from_le_bytes(wave[at + 4..at + 8].try_into()?) as usize;
        let chunk = wave.get(at + 8..at + 8 + n).ok_or("Truncated wave chunk")?;
        match &wave[at..at + 4] {
            b"fmt " => {
                rate = Some(u32::from_le_bytes(
                    chunk.get(8..12).ok_or("Short wave fmt")?.try_into()?,
                ) as f64)
            }
            b"data" => size = Some(n as f64),
            _ => {}
        }
        at += 8 + n + (n & 1);
    }
    let seconds =
        size.ok_or("No PCM data")? / rate.filter(|r| *r > 0.).ok_or("Invalid byte rate")?;
    Ok((wave, seconds))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-detonator-assets GAME_DATA OUTPUT".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let catalog = rc_inspect::defaults::load(root)?;
    let mut defaults = vec![];
    for (class, names) in [
        (
            "CTInventory.ThermalDetonatorProj",
            vec![
                "Speed",
                "MaxSpeed",
                "TossZ",
                "FuseTime",
                "PostImpactFuseTime",
                "BounceDampening",
                "ActorBounceDampening",
                "RestSpeed",
                "MinBounceSoundTime",
                "Damage",
                "DamageRadius",
                "MyDamageType",
                "SpawnSound",
                "ImpactSound",
                "StaticMesh",
                "Mesh",
                "DrawScale",
                "DrawType",
                "Physics",
                "CollisionRadius",
                "CollisionHeight",
                "bBounce",
                "bActorDetonate",
                "bWorldDetonate",
                "ExplosionEffect",
                "HitEffectsByMaterial",
                "ExplosionDelay",
                "LifeSpan",
            ],
        ),
        (
            "CTInventory.ThermalDetonatorAmmo",
            vec![
                "ProjectileClass",
                "bInstantHit",
                "FireSound",
                "AmmoAmount",
                "MaxAmmo",
            ],
        ),
        (
            "CTInventory.ThermalDetonator",
            vec!["FireSound", "GrenadeEmptySound", "FireRate", "AmmoName"],
        ),
        ("CTInventory.DC15s", vec!["FPGrenadeOffset"]),
    ] {
        for name in names {
            defaults.push(serde_json::json!({"class":class,"property":name,"resolved":catalog.resolve(class,name,0)}));
        }
    }
    let data = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&data)?;
    let mut objects = vec![];
    for (i, e) in pkg.exports.iter().enumerate() {
        if e.class == 0 {
            continue;
        }
        let class = pkg.object_path(e.class)?;
        let props = properties::read(&pkg, &data, e)?;
        // Preserve the action graph, not just names that happen to mention the target.
        if class.contains("ACTION_")
            || class.ends_with("ScriptedTrigger")
            || serde_json::to_string(&props)?
                .to_lowercase()
                .contains("gunner")
            || serde_json::to_string(&props)?
                .to_lowercase()
                .contains("bunker")
            || serde_json::to_string(&props)?.contains("PopUpGrenades")
        {
            let mut resolved = vec![];
            for name in [
                "Location",
                "CollisionRadius",
                "CollisionHeight",
                "Health",
                "bInvulnerable",
                "DamageFilter",
                "Event",
                "Tag",
                "StaticMesh",
                "DamageType",
                "Damage",
                "Radius",
                "bWaitForFinish",
            ] {
                if let Ok(value) = catalog.instance(&class, &props, name, 0, "geo_01a.ctm") {
                    resolved.push(serde_json::json!({"property":name,"resolved":value}));
                }
            }
            let actions = if let Some(a) = props.values.iter().find(|p| p.name == "Actions") {
                let bytes = pkg.payload(&data, e)?;
                properties::object_array(
                    &pkg,
                    &bytes[a.payload_offset..a.payload_offset + a.bytes],
                )?
            } else {
                vec![]
            };
            objects.push(serde_json::json!({"original":i+1,"name":pkg.object_path(i as i32+1)?,"class":class,"properties":props,"resolved":resolved,"actions":actions}));
        }
    }
    let property = |id: usize, name: &str| -> Result<properties::Value, String> {
        let e = pkg.exports.get(id - 1).ok_or("Bad original export")?;
        Ok(catalog
            .instance(
                &pkg.object_path(e.class)?,
                &properties::read(&pkg, &data, e)?,
                name,
                0,
                "geo_01a.ctm",
            )?
            .value)
    };
    let number = |id, name| -> Result<f64, String> {
        match property(id, name)? {
            properties::Value::Float(v) => Ok(v as f64),
            properties::Value::Int(v) => Ok(v as f64),
            properties::Value::Byte(v) => Ok(v as f64),
            _ => Err("Not an original number".into()),
        }
    };
    let text = |id, name| -> Result<String, String> {
        match property(id, name)? {
            properties::Value::Name(v) | properties::Value::String(v) => Ok(v),
            properties::Value::Object { path, .. } => Ok(path),
            _ => Err("Not original text/object".into()),
        }
    };
    for (id, name, expected) in [
        (621, "ClassProximityType", "CTInventory.GrenadeProj"),
        (621, "Event", "MachineGunBunkerDestroyed"),
        (2084, "ExternalEvent", "MachineGunBunkerDestroyed"),
        (2089, "TargetTag", "SmallBunker"),
        (2090, "DestroyTag", "SmallBunkerHatch"),
        (2091, "CollideActorTag", "BunkerCollision"),
        (3003, "ExternalEvent", "MachineGunBunkerDestroyed"),
        (3004, "Event", "UnblockBunkerPath"),
        (3005, "ActorTag", "TrackerTarget09"),
        (3014, "Event", "MissionObjectives"),
        (3015, "Event", "Conversation04"),
    ] {
        if text(id, name)? != expected {
            return Err(format!("Original binding changed: {id}.{name}").into());
        }
    }
    if number(621, "TriggerType")? != 2.
        || number(3006, "ObjectiveNum")? != 3.
        || number(3006, "UpdateAction")? != 0.
    {
        return Err("Unexpected original trigger/objective semantics".into());
    }
    for (id, start, end) in [(325, 2084, 2103), (696, 2792, 2816), (754, 3003, 3015)] {
        let row = objects
            .iter()
            .find(|o| o["original"] == id)
            .ok_or("Missing original script")?;
        let actual = row["actions"]
            .as_array()
            .ok_or("Missing action graph")?
            .iter()
            .map(|a| a[0].as_u64().ok_or("Bad action index"))
            .collect::<Result<Vec<_>, _>>()?;
        if actual != (start..=end).collect::<Vec<_>>() {
            return Err("Original gun-position action order changed".into());
        }
    }
    for id in [3011, 3012, 3013] {
        if !matches!(property(id, "bDisabled")?, properties::Value::Bool(true)) {
            return Err("Original disabled line changed".into());
        }
    }
    let value = |name| -> Result<f64, String> {
        match catalog
            .resolve("CTInventory.ThermalDetonatorProj", name, 0)?
            .value
        {
            properties::Value::Float(v) => Ok(v as f64),
            _ => Err("Not numeric projectile default".into()),
        }
    };
    for (name, expected) in [
        ("bBounce", true),
        ("bActorDetonate", false),
        ("bWorldDetonate", false),
    ] {
        if !matches!(catalog.resolve("CTInventory.ThermalDetonatorProj",name,0)?.value,properties::Value::Bool(v) if v==expected)
        {
            return Err(format!("Unsupported original detonator branch: {name}").into());
        }
    }
    if !matches!(
        catalog
            .resolve("CTInventory.ThermalDetonatorProj", "Physics", 0)?
            .value,
        properties::Value::Byte(2)
    ) || value("CollisionRadius")? != 0.
        || value("CollisionHeight")? != 0.
    {
        return Err("Unsupported original projectile physics/extent".into());
    }
    let projectile = rc_game::detonator::Assets {
        speed: value("Speed")? as f32,
        max_speed: value("MaxSpeed")? as f32,
        toss_z: value("TossZ")? as f32,
        fuse_time: value("FuseTime")?,
        post_impact_fuse: value("PostImpactFuseTime")?,
        life_span: value("LifeSpan")?,
        world_damping: value("BounceDampening")? as f32,
        actor_damping: value("ActorBounceDampening")? as f32,
        rest_speed: value("RestSpeed")? as f32,
        min_bounce_sound_time: value("MinBounceSoundTime")?,
        damage: value("Damage")? as f32,
        damage_radius: value("DamageRadius")? as f32,
    };
    projectile.validate()?;
    let sounds_dir = out.join("sounds");
    fs::create_dir_all(&sounds_dir)?;
    let mut sounds = vec![];
    let mut confirmation_seconds = None;
    for id in [2793, 2799, 2804, 2809, 2814, 3008] {
        let path = text(id, "Sound")?;
        let (wave, seconds) = sound(root, &path)?;
        fs::write(sounds_dir.join(format!("{id}.wav")), wave)?;
        sounds.push(serde_json::json!({"action":id,"source":path,"seconds":seconds,"file":format!("{id}.wav")}));
        if id == 3008 {
            confirmation_seconds = Some(seconds);
        }
    }
    for (class, property, role) in [
        ("CTInventory.ThermalDetonatorProj", "ImpactSound", "bounce"),
        ("CTInventory.ThermalDetonator", "GrenadeEmptySound", "empty"),
    ] {
        let path = match catalog.resolve(class, property, 0)?.value {
            properties::Value::Object { index, path } if index != 0 => path,
            _ => return Err("Missing original detonator sound".into()),
        };
        let (wave, seconds) = sound(root, &path)?;
        fs::write(sounds_dir.join(format!("{role}.wav")), wave)?;
        sounds.push(serde_json::json!({"role":role,"source":path,"seconds":seconds,"file":format!("{role}.wav")}));
    }
    let position = match property(621, "Location")? {
        properties::Value::Vector(v) => v.map(f64::from),
        _ => return Err("Invalid target position".into()),
    };
    let gunner = rc_game::gunner::Assets {
        contact: rc_game::crash::Trigger {
            original: 621,
            position,
            radius: number(621, "CollisionRadius")?,
            height: number(621, "CollisionHeight")?,
        },
        destruction_delay: number(2085, "PauseTime")?,
        confirmation_delay: number(3007, "PauseTime")?,
        confirmation_seconds: confirmation_seconds.ok_or("Missing confirmation")?,
        after_confirmation_delay: number(3010, "PauseTime")?,
        bunker_health: number(850, "Health")? as u32,
        bunker_script_damage: number(2089, "amount")? as u32,
    };
    gunner.validate()?;
    fs::write(
        out.join("detonator-assets.json"),
        serde_json::to_vec_pretty(&projectile)?,
    )?;
    fs::write(
        out.join("gunner-assets.json"),
        serde_json::to_vec_pretty(&gunner)?,
    )?;
    fs::write(out.join("sounds.json"), serde_json::to_vec_pretty(&sounds)?)?;
    fs::write(
        out.join("original-detonator-data.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"defaults":defaults,"objects":objects}))?,
    )?;
    println!(
        "Inspected {} detonator defaults and {} original mission graph objects",
        defaults.len(),
        objects.len()
    );
    Ok(())
}
