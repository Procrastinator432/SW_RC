//! Bind the selected original energy weapon to a campaign scene's base layer.
use rc_package::properties::Value;
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 4 {
        return Err("usage: rc-dc15-build GAME_DATA SCENE OUTPUT_DIRECTORY".into());
    }
    let catalog = rc_inspect::defaults::load(Path::new(&args[1]))?;
    let mut receipt = serde_json::Map::new();
    let mut get = |class: &str, name: &str| -> Result<Value, String> {
        let resolved = catalog.resolve(class, name, 0)?;
        receipt.insert(
            format!("{class}.{name}"),
            serde_json::to_value(&resolved).map_err(|e| e.to_string())?,
        );
        Ok(resolved.value)
    };
    let Value::Int(maximum) = get("CTInventory.DC15Ammo", "MaxAmmo")? else {
        return Err("Missing energy maximum".into());
    };
    let Value::Float(recharge_rate) = get("CTInventory.DC15Ammo", "RechargeRate")? else {
        return Err("Missing recharge rate".into());
    };
    let Value::Float(cooldown_time) = get("CTInventory.DC15Ammo", "CoolDownTime")? else {
        return Err("Missing cooldown time".into());
    };
    let Value::Bool(wait_for_recharge) = get("CTInventory.DC15Ammo", "bWaitForRecharge")? else {
        return Err("Missing recharge gate".into());
    };
    let Value::Float(damage) = get("CTInventory.DC15Ammo", "Damage")? else {
        return Err("Missing ammo damage".into());
    };
    if !matches!(get("CTInventory.DC15Proj","Damage")?,Value::Float(v) if v==damage)
        || !matches!(get("CTInventory.DC15s", "bCanReload")?, Value::Bool(false))
        || !matches!(get("CTInventory.DC15s","AmmoName")?,Value::Object{ref path,..} if path.eq_ignore_ascii_case("CTInventory.DC15Ammo"))
        || !damage.is_finite()
        || damage.fract() != 0.
        || damage <= 0.
    {
        return Err("Unsupported original DC15 weapon variant".into());
    }
    let Value::Float(shot_delay) = get("CTInventory.DC15s", "ShotDelay")? else {
        return Err("Missing shot delay".into());
    };
    for name in ["bCoolAndRecharge", "bNoRecharge"] {
        if !matches!(get("CTInventory.DC15Ammo", name)?, Value::Bool(false)) {
            return Err("Unsupported initial recharge state".into());
        }
    }
    let fire_mode = match get("CTInventory.DC15s", "FireMode")? {
        Value::Byte(0) => rc_game::dc15::FireMode::SemiAuto,
        Value::Byte(2) => rc_game::dc15::FireMode::Automatic,
        _ => return Err("Unsupported original DC15 fire mode".into()),
    };
    let Value::Name(fire_anim) = get("CTInventory.DC15s", "FireAnim")? else {
        return Err("Missing original weapon FireAnim".into());
    };
    let Value::Float(idle_delay) = get("CTInventory.DC15s", "PlayIdleTime")? else {
        return Err("Missing original weapon PlayIdleTime".into());
    };
    let assets = rc_game::dc15::Assets {
        maximum: maximum.try_into()?,
        recharge_rate,
        cooldown_time,
        wait_for_recharge,
        damage: damage as u32,
        shot_delay,
        fire_mode: Some(fire_mode),
        animation: Some(rc_game::weapon_animation::Assets {
            fire_anim,
            idle_delay,
        }),
        provenance: serde_json::json!({"defaults":receipt,"scripts":["DC15Ammo.Tick/UseCharge","CTAmmoRecharging.PostBeginPlay/Tick/UseCharge/HasAmmo","DC15s.Fire","Weapon.PlayFiring","Weapon.EFireMode/ServerFire/NormalFire.Tick/StopFire","Weapon.PlayIdleAnim/Idle.AnimEnd/NormalFire.AnimEnd"],"scope":"Original selected energy accounting and base damage/shot delay. Human semi-auto requests require a fresh press; automatic requests remain held. Requests blocked by cooldown, locks or charge are not queued. Selected non-zoom idle/walk/run/fidget and independent fire channel use original clips, rates and idle delay. Tweening, forced-fire, pawn animations, full state machine, hitscan, aim, hitboxes and difficulty adapters remain; not full Weapon.NormalFire or ballistic parity."}),
    };
    assets.validate()?;
    let raw = fs::read(&args[2])?;
    let game = rc_game::bundle::decode(&raw)?;
    if game.dc15.is_some()
        || game.combat.is_some()
        || (game.opening.is_none() && game.map.is_none())
    {
        return Err("Need unextended campaign weapon scene".into());
    }
    let weapon = &game
        .visuals
        .as_ref()
        .ok_or("Need original weapon visuals")?
        .weapon
        .mesh;
    let profile = assets
        .animation
        .as_ref()
        .ok_or("Missing animation profile")?;
    let missing: Vec<_> = ["Run", "Idle", profile.fire_anim.as_str()]
        .into_iter()
        .filter(|name| {
            !weapon
                .clips
                .iter()
                .any(|c| c.sequence.name.name.eq_ignore_ascii_case(name))
        })
        .collect();
    let mut extra = vec![];
    if !missing.is_empty() {
        let (package, object) = weapon
            .source
            .split_once('.')
            .ok_or("Invalid weapon source")?;
        let animation = weapon
            .clips
            .first()
            .ok_or("Missing original clip linkup")?
            .source
            .split(':')
            .next()
            .ok_or("Invalid original animation source")?;
        let mut resolver = rc_inspect::assets::Assets::new(&[
            &Path::new(&args[1]).join("Textures"),
            &Path::new(&args[1]).join("Animations"),
            &Path::new(&args[1]).join("System"),
        ])?;
        let loaded = rc_inspect::visual_assets::load_from_animation(
            Path::new(&args[1]),
            &format!("{package}.ukx"),
            object,
            &missing,
            &mut resolver,
            Some(animation),
        )?;
        if loaded.scale != weapon.scale
            || loaded.origin != weapon.origin
            || loaded.rotation != weapon.rotation
            || loaded.bones.len() != weapon.bones.len()
            || loaded
                .bones
                .iter()
                .zip(&weapon.bones)
                .any(|(a, b)| !a.name.name.eq_ignore_ascii_case(&b.name.name))
        {
            return Err("Additional weapon clips do not match the installed mesh".into());
        }
        extra = loaded.clips;
    }
    let mut result = vec![];
    let mut at = 0;
    loop {
        let header = raw.get(at..at + 12).ok_or("Truncated campaign layer")?;
        if &header[..6] != b"RCPLAY" {
            return Err("Invalid campaign layer".into());
        }
        let n = u32::from_le_bytes(header[8..12].try_into()?) as usize;
        let end = at.checked_add(12 + n).ok_or("Layer overflow")?;
        let metadata = raw.get(at + 12..end).ok_or("Truncated campaign metadata")?;
        if &header[..8] == b"RCPLAY01" {
            let mut base: serde_json::Value = serde_json::from_slice(metadata)?;
            base["dc15"] = serde_json::to_value(&assets)?;
            let encoded = serde_json::to_vec(&base)?;
            result.extend_from_slice(&header[..8]);
            result.extend((encoded.len() as u32).to_le_bytes());
            result.extend(encoded);
            result.extend_from_slice(&raw[end..]);
            break;
        }
        if &header[..8] == b"RCPLAY02" && !extra.is_empty() {
            let mut visuals: serde_json::Value = serde_json::from_slice(metadata)?;
            let clips = visuals["weapon"]["clips"]
                .as_array_mut()
                .ok_or("Missing weapon clip array")?;
            for clip in &extra {
                clips.push(serde_json::to_value(clip)?);
            }
            let data = serde_json::to_vec(&visuals)?;
            result.extend_from_slice(&header[..8]);
            result.extend((data.len() as u32).to_le_bytes());
            result.extend(data);
        } else {
            result.extend_from_slice(&raw[at..end]);
        }
        at = end;
    }
    let checked = rc_game::bundle::decode(&result)?;
    assert_eq!(checked.state.ammo, assets.maximum);
    assert_eq!(checked.state.reserve, 0);
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    fs::write(
        out.join(
            Path::new(&args[2])
                .file_name()
                .ok_or("Missing scene name")?,
        ),
        &result,
    )?;
    fs::write(out.join("weapon-clips.json"), serde_json::to_vec_pretty(
        &checked.visuals.as_ref().ok_or("Missing final weapon")?.weapon.mesh.clips.iter()
            .map(|c| serde_json::json!({"source":c.source,"frames":c.sequence.frames,"rate":f32::from_bits(c.sequence.rate_bits)}))
            .collect::<Vec<_>>()
    )?)?;
    fs::write(
        out.join("dc15-energy.json"),
        serde_json::to_vec_pretty(&assets)?,
    )?;
    println!(
        "DC-15s: {} charges, recharge {}, damage {}, delay {}",
        assets.maximum, recharge_rate, damage, shot_delay
    );
    Ok(())
}
