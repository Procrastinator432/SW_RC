//! Build a selected DC-17m combat scene from original defaults, notifies and embedded PCM.
use rc_game::combat::{Assets, Notify, Sound};
use rc_inspect::defaults;
use rc_package::{
    defaults::Catalog,
    properties::{self, Value},
    read_package, Package,
};
use std::{env, fs, path::Path};
fn number(c: &Catalog, class: &str, name: &str) -> Result<f64, String> {
    match c.resolve(class, name, 0)?.value {
        Value::Int(v) => Ok(f64::from(v)),
        Value::Float(v) => Ok(f64::from(v)),
        _ => Err(format!("Not a number: {class}.{name}")),
    }
}
fn index(b: &[u8], p: &mut usize) -> Result<i32, String> {
    let byte = |p: &mut usize| -> Result<u8, String> {
        let v = *b.get(*p).ok_or("Truncated sound list")?;
        *p += 1;
        Ok(v)
    };
    let first = byte(p)?;
    let mut value = u64::from(first & 63);
    let mut more = first & 64 != 0;
    let mut shift = 6;
    for i in 0..4 {
        if !more {
            break;
        }
        let v = byte(p)?;
        if i == 3 && v & 224 != 0 {
            return Err("Sound index overflow".into());
        }
        value |= u64::from(v & if i == 3 { 31 } else { 127 }) << shift;
        more = i != 3 && v & 128 != 0;
        shift += 7;
    }
    i32::try_from(if first & 128 != 0 {
        -(value as i64)
    } else {
        value as i64
    })
    .map_err(|_| "Sound index overflow".into())
}
fn sound(
    pkg: &Package,
    bytes: &[u8],
    object: &str,
    bank: &mut Vec<Sound>,
    receipt: &mut Vec<serde_json::Value>,
    depth: usize,
) -> Result<usize, String> {
    if depth > 4 {
        return Err("Sound group recursion".into());
    }
    let (i, e) = pkg
        .exports
        .iter()
        .enumerate()
        .find(|(i, _)| {
            pkg.object_path(*i as i32 + 1)
                .is_ok_and(|p| p.eq_ignore_ascii_case(object))
        })
        .ok_or_else(|| format!("Missing sound {object}"))?;
    let full = format!("GEN_Sound.{}", pkg.object_path(i as i32 + 1)?);
    if let Some(i) = bank.iter().position(|s| s.source == full) {
        return Ok(i);
    }
    let payload = pkg.payload(bytes, e)?;
    let props = properties::read(pkg, bytes, e)?;
    if pkg.object_path(e.class)? == "Engine.SoundMultiple" {
        let mut pos = props.native_offset;
        let count = index(payload, &mut pos)?;
        if !(1..=64).contains(&count) {
            return Err("Invalid selected sound group".into());
        }
        let mut children = vec![];
        for _ in 0..count {
            children.push(pkg.object_path(index(payload, &mut pos)?)?);
        }
        if payload.len() != pos + 1 {
            return Err("Unsupported sound group layout".into());
        }
        receipt.push(serde_json::json!({"group":full,"children":children,"selection_style":payload[pos],"adapter":"first variant; native random/likelihood selection pending"}));
        return sound(pkg, bytes, &children[0], bank, receipt, depth + 1);
    }
    if pkg.object_path(e.class)? != "Engine.Sound" {
        return Err("Unsupported sound class".into());
    }
    // Preserve the observed USound wrapper as opaque evidence; validate its one embedded WAV.
    let locations: Vec<_> = payload
        .windows(4)
        .enumerate()
        .filter_map(|(i, w)| (w == b"RIFF").then_some(i))
        .collect();
    let start = *locations.first().ok_or("Missing PCM RIFF")?;
    if start < props.native_offset || start > 128 {
        return Err("Unsupported PCM wrapper".into());
    }
    let header = payload
        .get(start + 4..start + 8)
        .ok_or("Truncated RIFF length")?;
    let len = u32::from_le_bytes(header.try_into().unwrap()) as usize + 8;
    let wave = payload
        .get(start..start + len)
        .ok_or("Truncated original WAV")?
        .to_vec();
    rc_game::combat::validate_wave(&wave)?;
    receipt.push(serde_json::json!({"source":full,"export":i+1,"wrapper_prefix":&payload[..start],"wave_offset":start,"wave_bytes":len,"trailing_bytes":payload.len()-start-len,"properties":props}));
    let result = bank.len();
    bank.push(Sound { source: full, wave });
    Ok(result)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-play-combat GAME_DATA ANIMATED_SCENE OUTPUT_DIRECTORY".into());
    }
    let game = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let c = defaults::load(game)?;
    let base = fs::read(&args[2])?;
    let scene = rc_game::bundle::decode(&base)?;
    let visuals = scene.visuals.as_ref().ok_or("Missing original meshes")?;
    let sound_bytes = fs::read(game.join("Sounds/gen_sound.uax"))?;
    let pkg = read_package(&sound_bytes)?;
    let mut bank = vec![];
    let mut sounds = vec![];
    let Value::Object { path, .. } = c.resolve("CTInventory.DC17mBlaster", "FireSound", 0)?.value
    else {
        return Err("FireSound object required".into());
    };
    let fire = sound(
        &pkg,
        &sound_bytes,
        path.split_once('.').ok_or("Sound package")?.1,
        &mut bank,
        &mut sounds,
        0,
    )?;
    let mut notifies = vec![];
    let mut notify_receipt = vec![];
    for (weapon, file, mesh) in [
        (true, "hudarms.ukx", &visuals.weapon),
        (false, "geonosian.ukx", &visuals.enemy),
    ] {
        let bytes = fs::read(game.join("Animations").join(file))?;
        let p = read_package(&bytes)?;
        for clip in &mesh.mesh.clips {
            for n in &clip.sequence.notifies {
                let e = p
                    .exports
                    .get((n.object_index - 1) as usize)
                    .ok_or("Notify export index")?;
                let props = properties::read(&p, &bytes, e)?;
                let class = p.object_path(e.class)?;
                let path = props.values.iter().find_map(|v| {
                    if v.name == "Sound" {
                        if let Value::Object { path, .. } = &v.value {
                            Some(path)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                });
                notify_receipt.push(serde_json::json!({"clip":clip.source,"time":f32::from_bits(n.time_bits),"object":p.object_path(n.object_index)?,"class":class,"properties":props,"dispatch":if class=="Engine.AnimNotify_Sound" && path.is_some(){"sound"}else{"unimplemented/null"}}));
                if class == "Engine.AnimNotify_Sound" {
                    if let Some(path) = path {
                        let snd = sound(
                            &pkg,
                            &sound_bytes,
                            path.split_once('.').ok_or("Notify sound package")?.1,
                            &mut bank,
                            &mut sounds,
                            0,
                        )?;
                        notifies.push(Notify {
                            weapon,
                            clip: clip.sequence.name.name.clone(),
                            time: f32::from_bits(n.time_bits),
                            sound: snd,
                        });
                    }
                }
            }
        }
    }
    notifies.sort_by(|a, b| a.time.total_cmp(&b.time));
    let weapon = "CTInventory.DC17mBlaster";
    let projectile = "CTInventory.DC17BlasterProj";
    let reload = &visuals.weapon.mesh.clips[visuals.weapon.clip("Reload")?].sequence;
    let provenance = serde_json::json!({"weapon":weapon,"ammo":"CTInventory.DC17mBlasterAmmo","projectile":projectile,"defaults_report":"combat-assets.json","scope":"Original selected scalar defaults and sound/notify data; projectile integrator, body hit boxes and enemy AI remain explicit port adapters.","reload":"(frames-1)/rate to last normalized channel frame; native full AnimEnd execution pending","sounds":sounds,"notifies":notify_receipt});
    let assets = Assets {
        jump: None,
        navigation: None,
        game_rules: None,
        bot: None,
        lunge: None,
        ballistics: None,
        melee: None,
        provenance,
        magazine: number(&c, weapon, "ReloadCount")? as u32,
        total_ammo: number(&c, "CTInventory.DC17mBlasterAmmo", "MaxAmmo")? as u32,
        shot_delay: number(&c, weapon, "ShotDelay")?,
        damage: number(&c, projectile, "Damage")? as u32,
        enemy_health: number(&c, "CTCharacters.GeonosianWarrior", "Health")? as u32,
        speed: number(&c, projectile, "Speed")?,
        acceleration: number(&c, projectile, "AccelFactor")?,
        max_speed: number(&c, projectile, "MaxSpeed")?,
        laser_bolt: matches!(
            c.resolve(projectile, "bLaserBolt", 0)?.value,
            Value::Bool(true)
        ),
        lifespan: number(&c, projectile, "LifeSpan")?,
        reload_duration: f64::from(reload.frames - 1) / f64::from(f32::from_bits(reload.rate_bits)),
        sounds: bank,
        fire_sound: fire,
        notifies,
    };
    let bundle = rc_game::bundle::encode_combat(&base, &assets)?;
    rc_game::bundle::decode(&bundle)?;
    fs::write(
        out.join("combat-receipt.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"provenance":assets.provenance,"magazine":assets.magazine,"total_ammo":assets.total_ammo,"shot_delay":assets.shot_delay,"damage":assets.damage,"enemy_health":assets.enemy_health,"speed":assets.speed,"acceleration":assets.acceleration,"max_speed":assets.max_speed,"reload_duration":assets.reload_duration,"sound_count":assets.sounds.len(),"sound_notify_count":assets.notifies.len(),"bundle_bytes":bundle.len()}),
        )?,
    )?;
    fs::write(out.join("geo_01a-combat.rcplay"), bundle)?;
    Ok(())
}
