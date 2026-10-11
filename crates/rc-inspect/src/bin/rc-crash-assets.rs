//! Extract the original crash-site event chain, factories and inherited trigger dimensions.
use rc_game::crash::{Assets, Factory, Trigger};
use rc_package::{
    properties::{self, Value},
    read_package,
};
use std::{env, fs, path::Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-crash-assets GAME_DATA OUTPUT_DIR".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let data = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&data)?;
    let catalog = rc_inspect::defaults::load(root)?;
    let property = |id: u32, name: &str| -> Result<Value, String> {
        let e = &pkg.exports[id as usize - 1];
        let class = pkg.object_path(e.class)?;
        let p = properties::read(&pkg, &data, e)?;
        Ok(catalog.instance(&class, &p, name, 0, "geo_01a.ctm")?.value)
    };
    let number = |id, name| -> Result<f64, String> {
        match property(id, name)? {
            Value::Float(v) => Ok(v as f64),
            Value::Int(v) => Ok(v as f64),
            Value::Byte(v) => Ok(v as f64),
            _ => Err(format!("Non-number {id}.{name}")),
        }
    };
    let point = |id| -> Result<[f64; 3], String> {
        match property(id, "Location")? {
            Value::Vector(v) => Ok(v.map(f64::from)),
            _ => Err("Invalid location".into()),
        }
    };
    let text = |id, name| -> Result<String, String> {
        match property(id, name)? {
            Value::Name(v) | Value::String(v) => Ok(v),
            Value::Object { path, .. } => Ok(path),
            _ => Err(format!("Non-text {id}.{name}")),
        }
    };
    let trigger = |id| -> Result<Trigger, String> {
        if pkg.object_path(pkg.exports[id as usize - 1].class)? != "Engine.Trigger" {
            return Err("Not Trigger".into());
        }
        Ok(Trigger {
            original: id,
            position: point(id)?,
            radius: number(id, "CollisionRadius")?,
            height: number(id, "CollisionHeight")?,
        })
    };
    let mut factories = vec![];
    let mut defaults = vec![];
    for (i, e) in pkg.exports.iter().enumerate() {
        if pkg.object_path(e.class)? != "CTGame.PawnFactory" {
            continue;
        }
        let id = i as u32 + 1;
        if text(id, "Tag")? != "SpawnGunshipArea" {
            continue;
        }
        if text(id, "PawnTagFromFactory")? != "GunshipDroids" {
            continue;
        }
        if text(id, "PawnEventFromFactory")? != "DeadGunshipDroid" {
            return Err("Unsupported factory event".into());
        }
        let prototype = text(id, "Prototype")?;
        let health = catalog.resolve(&prototype, "Health", 0)?;
        let hp = match health.value {
            Value::Int(v) if v > 0 => v as u32,
            Value::Float(v) if v > 0. && v.fract() == 0. => v as u32,
            _ => return Err(format!("Invalid droid health: {:?}", health.value).into()),
        };
        let rotation = match property(id, "Rotation")? {
            Value::Rotator(v) => v,
            _ => return Err("Invalid rotation".into()),
        };
        factories.push(Factory {
            original: id,
            position: point(id)?,
            rotation,
            prototype: prototype.clone(),
            health: hp,
            capacity: number(id, "Capacity")? as u32,
        });
        defaults.push(serde_json::json!({"factory":id,"health":health,"mesh":catalog.resolve(&prototype,"Mesh",0)?,
            "collision_radius":catalog.resolve(&prototype,"CollisionRadius",0)?,
            "collision_height":catalog.resolve(&prototype,"CollisionHeight",0)?,
            "spawn_mode":property(id,"SpawnMode")?,"start_state":property(id,"StartStateFromFactory")?}));
    }
    for (id, name, expected) in [
        (128, "Event", "SpawnGunshipArea"),
        (608, "Event", "DialogThereIsTooMany"),
        (620, "Tag", "DeadGunshipDroid"),
        (620, "Event", "CrashSiteSecured"),
        (2900, "CollideActorTag", "BactaBarricadeBlockingVolume"),
        (2910, "TargetTag", "PathToGunshipBarricade"),
        (3293, "CollideActorTag", "GunshipBarricadeBlockingVolume"),
    ] {
        if text(id, name)? != expected {
            return Err(format!("Original binding changed at {id}.{name}").into());
        }
    }
    for (id, name, expected) in [
        (2901, "ObjectiveNum", 0.),
        (2901, "UpdateAction", 0.),
        (1544, "ObjectiveNum", 1.),
        (1544, "UpdateAction", 0.),
        (2902, "PauseTime", 1.),
    ] {
        if number(id, name)? != expected {
            return Err(format!("Unsupported action {id}.{name}").into());
        }
    }
    for (id, name, expected) in [
        (1538, "bMakeGod", true),
        (1542, "bMakeGod", false),
        (2905, "bMakeGod", false),
        (2900, "bCollideActors", false),
        (2900, "bCollideWorld", false),
        (2900, "bBlockZeroExtentTraces", false),
        (2900, "bBlockNonZeroExtentTraces", false),
        (3293, "bCollideActors", false),
        (3293, "bCollideWorld", false),
        (3293, "bBlockZeroExtentTraces", false),
        (3293, "bBlockNonZeroExtentTraces", false),
    ] {
        if !matches!(property(id,name)?,Value::Bool(v) if v==expected) {
            return Err(format!("Unsupported flag {id}.{name}").into());
        }
    }
    let barrier = |tag: &str| -> Result<u32, String> {
        for (i, e) in pkg.exports.iter().enumerate() {
            if e.class != 0 && text(i as u32 + 1, "Tag").is_ok_and(|t| t == tag) {
                return Ok(i as u32 + 1);
            }
        }
        Err(format!("Missing {tag}"))
    };
    // The blocking advisor uses the original RIFF duration; no guessed timer.
    let voice = fs::read(root.join("Sounds/GEO_Voice.uax"))?;
    let vp = read_package(&voice)?;
    let e = vp
        .exports
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            vp.object_path(i as i32 + 1)
                .is_ok_and(|p| p == "GEO_01.CA1G1791")
                .then_some(e)
        })
        .ok_or("Missing Bacta advisor")?;
    let payload = vp.payload(&voice, e)?;
    let start = payload
        .windows(4)
        .position(|w| w == b"RIFF")
        .ok_or("No RIFF")?;
    let riff = payload.get(start..).ok_or("Bad RIFF offset")?;
    let total = u32::from_le_bytes(riff.get(4..8).ok_or("Short RIFF")?.try_into()?) as usize + 8;
    let wave = riff.get(..total).ok_or("Short RIFF body")?;
    rc_game::combat::validate_wave(wave)?;
    let mut at = 12;
    let mut byte_rate = None;
    let mut sample_bytes = None;
    while at + 8 <= wave.len() {
        let size = u32::from_le_bytes(wave[at + 4..at + 8].try_into()?) as usize;
        let chunk = wave.get(at + 8..at + 8 + size).ok_or("Short wave chunk")?;
        match &wave[at..at + 4] {
            b"fmt " => {
                byte_rate = Some(u32::from_le_bytes(
                    chunk.get(8..12).ok_or("Short fmt")?.try_into()?,
                ) as f64);
            }
            b"data" => {
                sample_bytes = Some(size as f64);
            }
            _ => {}
        }
        at += 8 + size + (size & 1);
    }
    let advisor_duration = sample_bytes.ok_or("Missing samples")?
        / byte_rate.filter(|r| *r > 0.).ok_or("Missing rate")?;
    let mut scripts = vec![];
    for id in [739, 129, 679, 681, 683, 911] {
        let e = &pkg.exports[id - 1];
        let p = properties::read(&pkg, &data, e)?;
        let prop = p
            .values
            .iter()
            .find(|p| p.name == "Actions")
            .ok_or("Missing script actions")?;
        let bytes = pkg.payload(&data, e)?;
        let actions = properties::object_array(
            &pkg,
            &bytes[prop.payload_offset..prop.payload_offset + prop.bytes],
        )?;
        let expected: Option<Vec<i32>> = match id {
            739 => Some((2899..=2913).collect()),
            129 => Some((1537..=1551).collect()),
            911 => Some((3292..=3295).collect()),
            _ => None,
        };
        if expected.is_some_and(|v| v != actions.iter().map(|(i, _)| *i).collect::<Vec<_>>()) {
            return Err(format!("Original action order changed in script {id}").into());
        }
        let rows=actions.iter().map(|(i,_)|{
            let action=&pkg.exports[*i as usize-1];
            Ok(serde_json::json!({"original":i,"class":pkg.object_path(action.class)?,"properties":properties::read(&pkg,&data,action)?}))
        }).collect::<Result<Vec<_>,String>>()?;
        scripts.push(serde_json::json!({"original":id,"actions":rows}));
    }
    let a = Assets {
        provenance: serde_json::json!({"map":"geo_01a","scripts":scripts,"factory_defaults":defaults,
        "scope":"Selected event host only. Spawn/collision/damage/AI/graphics application and full mission integration are pending. Timers use original float defaults and PCM duration; frame partition/contact tests are port adapters."}),
        spawn: trigger(128)?,
        vulnerability: trigger(608)?,
        factories,
        counter: number(620, "NumToCount")? as u32,
        counter_export: 620,
        bacta_barrier: barrier("BactaBarricadeBlockingVolume")?,
        path_barrier: barrier("PathToGunshipBarricade")?,
        path_barrier_health: number(617, "Health")? as u32,
        path_barrier_damage: number(2910, "amount")? as u32,
        exit_barrier: barrier("GunshipBarricadeBlockingVolume")?,
        advisor_duration,
        retreat_delay: number(2908, "PauseTime")?,
    };
    a.validate()?;
    fs::write(
        out.join("crash-assets.json"),
        serde_json::to_vec_pretty(&a)?,
    )?;
    println!(
        "Extracted {} original droid factories, Counter{}, advisor {:.3}s, barrier {} -> {}",
        a.factories.len(),
        a.counter,
        a.advisor_duration,
        a.path_barrier_health,
        a.path_barrier_damage
    );
    Ok(())
}
