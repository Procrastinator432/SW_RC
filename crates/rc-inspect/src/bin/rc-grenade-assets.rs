//! Original grenade pickup defaults and selected geo_01a action inventory.
use rc_game::{
    crash::Trigger,
    grenade_pickup::{Assets, Cue},
};
use rc_package::{
    properties::{self, Value},
    read_package,
};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-grenade-assets GAME_DATA OUTPUT_DIR".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let data = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&data)?;
    let catalog = rc_inspect::defaults::load(root)?;
    let mut resolved = vec![];
    for (class, names) in [
        (
            "CTInventory.ThermalDetonatorBox",
            vec![
                "AmmoAmount",
                "InventoryType",
                "PickupSound",
                "StaticMesh",
                "Mesh",
                "DrawType",
                "DrawScale",
                "CollisionRadius",
                "CollisionHeight",
                "bCollideActors",
                "bBlockActors",
                "PickupType",
            ],
        ),
        (
            "CTInventory.ThermalDetonatorAmmo",
            vec!["MaxAmmo", "InitialAmount", "AmmoAmount"],
        ),
        (
            "CTInventory.ThermalDetonator",
            vec!["AmmoName", "PickupAmmoCount", "InitAmmoAmount"],
        ),
    ] {
        for name in names {
            resolved.push(serde_json::json!({"class":class,"property":name,"resolved":catalog.resolve(class,name,0)}));
        }
    }
    let mut objects = vec![];
    for (i, e) in pkg.exports.iter().enumerate() {
        if e.class == 0 {
            continue;
        }
        let id = i as u32 + 1;
        let class = pkg.object_path(e.class)?;
        let props = properties::read(&pkg, &data, e)?;
        let selected = [610,694,745,747,749,750,909,910,911,912,913,1062,1233,3289,3291,3297,3299].contains(&id)
            || (2953..=2968).contains(&id) || (3292..=3295).contains(&id) || (3438..=3441).contains(&id)
            || props.values.iter().any(|p| matches!(&p.value, properties::Value::Name(v) if v=="DialogGunnerPosition" || v=="TrackerTarget04"));
        if selected {
            let mut inherited = vec![];
            for name in [
                "CollisionRadius",
                "CollisionHeight",
                "AmmoAmount",
                "RespawnTime",
                "bInstantRespawn",
                "bForceRespawn",
                "TriggerType",
                "UpdateAction",
                "bWaitForFinish",
                "Location",
                "DrawScale",
                "DrawScale3D",
                "Rotation",
                "PrePivot",
                "StaticMesh",
                "Skins",
            ] {
                if let Ok(v) = catalog.instance(&class, &props, name, 0, "geo_01a.ctm") {
                    inherited.push(serde_json::json!({"property":name,"resolved":v}));
                }
            }
            objects.push(serde_json::json!({"original":id,"name":pkg.object_path(id as i32)?,"class":class,"properties":props,"resolved":inherited}));
        }
    }
    let property = |id: u32, name: &str| -> Result<Value, String> {
        let e = &pkg.exports[id as usize - 1];
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
            Value::Int(v) => Ok(v as f64),
            Value::Float(v) => Ok(v as f64),
            Value::Byte(v) => Ok(v as f64),
            _ => Err(format!("Non-number {id}.{name}")),
        }
    };
    let text = |id, name| -> Result<String, String> {
        match property(id, name)? {
            Value::Name(v) | Value::String(v) => Ok(v),
            Value::Object { path, .. } => Ok(path),
            _ => Err(format!("Non-text {id}.{name}")),
        }
    };
    let trigger = |id: u32| -> Result<Trigger, String> {
        Ok(Trigger {
            original: id,
            position: match property(id, "Location")? {
                Value::Vector(v) => v.map(f64::from),
                _ => return Err("Missing contact location".into()),
            },
            radius: number(id, "CollisionRadius")?,
            height: number(id, "CollisionHeight")?,
        })
    };
    // Fail closed on the selected action order and bindings, including inherited defaults.
    let mut scripts = vec![];
    for (id, expected) in [
        (745, (2953..=2957).collect::<Vec<i32>>()),
        (747, (2958..=2968).collect()),
    ] {
        let e = &pkg.exports[id - 1];
        let p = properties::read(&pkg, &data, e)?;
        let a = p
            .values
            .iter()
            .find(|p| p.name == "Actions")
            .ok_or("No actions")?;
        let bytes = pkg.payload(&data, e)?;
        let actions =
            properties::object_array(&pkg, &bytes[a.payload_offset..a.payload_offset + a.bytes])?;
        if actions.iter().map(|(i, _)| *i).collect::<Vec<_>>() != expected {
            return Err("Selected grenade action order changed".into());
        }
        let rows=actions.iter().map(|(i,_)| {
            let e=&pkg.exports[*i as usize-1];
            Ok(serde_json::json!({"original":i,"class":pkg.object_path(e.class)?,"properties":properties::read(&pkg,&data,e)?}))
        }).collect::<Result<Vec<_>,String>>()?;
        scripts.push(serde_json::json!({"original":id,"actions":rows}));
    }
    for (id, name, expected) in [
        (746, "Event", "DialogGunnerPosition"),
        (694, "Event", "PickedUpGrenades"),
        (610, "Tag", "ThermalDetonatorBox01"),
        (2953, "ExternalEvent", "DialogGunnerPosition"),
        (2954, "ActorTag", "ThermalDetonatorBox01"),
        (2958, "ExternalEvent", "PickedUpGrenades"),
        (2960, "Event", "MissionObjectives"),
        (2961, "ActorTag", "GunnerTarget"),
        (2962, "Event", "TurnOffPickUpGrenadeReminder"),
        (2966, "Event", "PopUpGrenades"),
        (2968, "Event", "DialogBlowUpGunnerDialog"),
        (1233, "WhichWeapon", "CTInventory.ThermalDetonator"),
    ] {
        if !text(id, name)?.eq_ignore_ascii_case(expected) {
            return Err(format!("Changed grenade binding {id}.{name}").into());
        }
    }
    for id in [746, 694] {
        if pkg.object_path(pkg.exports[id - 1].class)? != "Engine.Trigger"
            || number(id as u32, "TriggerType")? != 0.
        {
            return Err("Unsupported grenade trigger class/type".into());
        }
    }
    if number(2959, "ObjectiveNum")? != 2. || number(2959, "UpdateAction")? != 0. {
        return Err("Unsupported grenade objective update".into());
    }
    for id in [2955, 2956, 2963] {
        if !matches!(property(id, "bWaitForFinish")?, Value::Bool(true)) {
            return Err("Non-blocking original advisor".into());
        }
    }
    for name in ["bInstantRespawn", "bForceRespawn"] {
        if !matches!(property(610, name)?, Value::Bool(true)) {
            return Err("Unsupported pickup respawn flags".into());
        }
    }
    let maximum_ammo = match catalog
        .resolve("CTInventory.ThermalDetonatorAmmo", "MaxAmmo", 0)?
        .value
    {
        Value::Int(v) if v > 0 => v as u32,
        _ => return Err("Invalid ammo maximum".into()),
    };
    let init = match catalog
        .resolve("CTInventory.ThermalDetonator", "InitAmmoAmount", 0)?
        .value
    {
        Value::Int(v) if v >= 0 => v,
        _ => return Err("Unsupported initial ammo rule".into()),
    };
    let initial_ammo =
        (init + number(1233, "AmmoAmount")? as i32).clamp(0, maximum_ammo as i32) as u32;
    let mut cues = vec![];
    let sound_dir = out.join("sounds");
    fs::create_dir_all(&sound_dir)?;
    for (slot, path) in [
        text(2955, "Sound")?,
        text(2956, "Sound")?,
        text(2963, "Sound")?,
        text(610, "PickupSound")?,
    ]
    .iter()
    .enumerate()
    {
        let (package, object) = path.split_once('.').ok_or("Bad sound path")?;
        let bytes = fs::read(root.join("Sounds").join(format!("{package}.uax")))?;
        let p = read_package(&bytes)?;
        let e = p
            .exports
            .iter()
            .enumerate()
            .find_map(|(i, e)| {
                p.object_path(i as i32 + 1)
                    .is_ok_and(|s| s.eq_ignore_ascii_case(object))
                    .then_some(e)
            })
            .ok_or("Missing grenade sound")?;
        let payload = p.payload(&bytes, e)?;
        let at = payload
            .windows(4)
            .position(|w| w == b"RIFF")
            .ok_or("No grenade wave")?;
        let size = u32::from_le_bytes(
            payload
                .get(at + 4..at + 8)
                .ok_or("Short RIFF")?
                .try_into()?,
        ) as usize
            + 8;
        let wave = payload.get(at..at + size).ok_or("Short grenade wave")?;
        rc_game::combat::validate_wave(wave)?;
        let (mut at, mut rate, mut samples) = (12, None, None);
        while at + 8 <= wave.len() {
            let n = u32::from_le_bytes(wave[at + 4..at + 8].try_into()?) as usize;
            let chunk = wave.get(at + 8..at + 8 + n).ok_or("Short sound chunk")?;
            match &wave[at..at + 4] {
                b"fmt " => {
                    rate = Some(
                        u32::from_le_bytes(chunk.get(8..12).ok_or("Short fmt")?.try_into()?) as f64,
                    )
                }
                b"data" => samples = Some(n as f64),
                _ => {}
            }
            at += 8 + n + (n & 1);
        }
        let seconds =
            samples.ok_or("No sample data")? / rate.filter(|v| *v > 0.).ok_or("No sample rate")?;
        cues.push(Cue {
            source: path.clone(),
            seconds,
        });
        fs::write(sound_dir.join(format!("{slot}.wav")), wave)?;
    }
    let assets = Assets {
        provenance: serde_json::json!({"map":"geo_01a","scripts":scripts,"defaults":resolved,"scope":"Selected scripts 745/747 and ammo pickup only. Independent player-cylinder contact adapters and PCM durations. No scene route, pickup graphics/Karma, audio playback, HUD, reminder loop, grenade throwing or gun-position destruction integration."}),
        dialog_trigger: trigger(746)?,
        objective_trigger: trigger(694)?,
        pickup: trigger(610)?,
        initial_ammo,
        maximum_ammo,
        pickup_amount: number(610, "AmmoAmount")? as u32,
        respawn_seconds: number(610, "RespawnTime")?,
        cues,
        blink_seconds: number(2964, "BlinkTime")?,
        prompt_delay: number(2965, "PauseTime")?,
        next_dialog_delay: number(2967, "PauseTime")?,
    };
    assets.validate()?;
    fs::write(
        out.join("original-grenade-data.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"defaults":resolved,"objects":objects}))?,
    )?;
    fs::write(
        out.join("grenade-assets.json"),
        serde_json::to_vec_pretty(&assets)?,
    )?;
    println!(
        "Original grenade pickup: initial {}, grant {}, maximum {}, {} PCM sounds",
        initial_ammo,
        assets.pickup_amount,
        maximum_ammo,
        assets.cues.len()
    );
    Ok(())
}
