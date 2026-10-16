//! Add an explicit original BattleDroid fire binding to compatible A/B scenes.
use rc_package::properties::Value;
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-enemy-feedback-build GAME_DATA SCENE OUTPUT_DIRECTORY".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let catalog = rc_inspect::defaults::load(root)?;
    let binding = catalog.resolve("Properties.BattleDroidBlasterDefaults", "FireSound", 0)?;
    let path = match &binding.value {
        Value::Object { index, path } if *index != 0 => path.clone(),
        _ => return Err("Missing BattleDroid fire sound".into()),
    };
    let mut receipts = vec![];
    let (sound, _) = rc_inspect::mission_assets::sound_variant(root, &path, &mut receipts)?;
    let raw = fs::read(&args[2])?;
    let original = rc_game::bundle::decode(&raw)?;
    if original
        .mission
        .as_ref()
        .is_some_and(|m| m.enemy_fire_sound.is_some())
    {
        return Err("Use the pinned pre-feedback scene; an existing inner binding may have shifted opening cues".into());
    }
    let mut offset = 0;
    let mut output = vec![];
    let mut changed = 0;
    let mut map = "geo_01a";
    for depth in 0..16 {
        let header = raw.get(offset..offset + 12).ok_or("Truncated bundle")?;
        if !header.starts_with(b"RCPLAY") {
            return Err("Invalid layer".into());
        }
        let n = u32::from_le_bytes(header[8..12].try_into()?) as usize;
        let end = offset.checked_add(12 + n).ok_or("Layer overflow")?;
        let metadata = raw.get(offset + 12..end).ok_or("Short metadata")?;
        if &header[..8] == b"RCPLAY13" || &header[..8] == b"RCPLAY14" {
            let mut value: serde_json::Value = serde_json::from_slice(metadata)?;
            if &header[..8] == b"RCPLAY14" {
                if value["destination"] != "GEO_01B.ctm#playerstart" {
                    return Err("Only A/B missing fire bindings are supported".into());
                }
                map = "geo_01b";
            }
            if value.get("enemy_fire_sound").is_some() || value.get("enemy_fire_source").is_some() {
                return Err("Scene already has an enemy fire binding".into());
            }
            let opening = if &header[..8] == b"RCPLAY13" {
                let assets: rc_game::opening::Assets = serde_json::from_value(value.clone())?;
                let bank = &original
                    .mission
                    .as_ref()
                    .ok_or("Missing mission bank")?
                    .sounds;
                let bindings: Vec<_> = assets
                    .sound_roles()
                    .into_iter()
                    .map(|role| {
                        let s = &bank[role];
                        rc_game::opening::SoundBinding {
                            role,
                            source: s.source.clone(),
                            pcm: rc_game::replay::fingerprint(&s.wave),
                        }
                    })
                    .collect();
                value["sound_bindings"] = serde_json::to_value(bindings)?;
                true
            } else {
                false
            };
            let sounds = value["sounds"].as_array_mut().ok_or("Missing sound bank")?;
            let index = sounds.len();
            sounds.push(serde_json::to_value(&sound)?);
            if opening {
                value["enemy_fire_source"] = sound.source.clone().into();
            } else {
                value["enemy_fire_sound"] = index.into();
            }
            let encoded = serde_json::to_vec(&value)?;
            output.extend_from_slice(&header[..8]);
            output.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
            output.extend(encoded);
            changed += 1;
        } else {
            output.extend_from_slice(raw.get(offset..end).ok_or("Short layer")?);
        }
        offset = end;
        if &header[..8] == b"RCPLAY01" {
            break;
        }
        if depth == 15 {
            return Err("Too many layers".into());
        }
    }
    if changed != 1 {
        return Err("Expected one campaign sound bank".into());
    }
    output.extend_from_slice(&raw[offset..]);
    rc_game::bundle::decode(&output)?;
    fs::write(out.join(format!("{map}.rcplay")), output)?;
    fs::write(
        out.join("enemy-audio-binding.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"map":map,"binding":binding,"selected_pcm":sound.source,"variants":receipts,"scope":"Original BattleDroid fire sound; bounded hitscan feedback, no native projectile/AI parity claim"}),
        )?,
    )?;
    println!("{map}: original BattleDroid fire binding {}", sound.source);
    Ok(())
}
