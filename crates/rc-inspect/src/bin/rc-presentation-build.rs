//! Bind selected original GEO_01A factories/prop transitions to visible effects.
use rc_package::{
    properties::{self, Value},
    read_package,
};
use std::{env, fs, path::Path};
#[derive(serde::Serialize)]
struct Pixels {
    #[serde(with = "rc_game::pixel_words")]
    pixels: Vec<u32>,
}
fn compact(value: &mut serde_json::Value) -> Result<usize, String> {
    let mut count = 0;
    if let Some(object) = value.as_object_mut() {
        if object.contains_key("uv_scale")
            && object
                .get("source")
                .is_some_and(serde_json::Value::is_string)
        {
            if let Some(pixels) = object.get("pixels").and_then(serde_json::Value::as_array) {
                let words = pixels
                    .iter()
                    .map(|p| {
                        p.as_u64()
                            .filter(|p| *p <= u64::from(u32::MAX))
                            .map(|p| p as u32)
                            .ok_or("Invalid material pixel")
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let encoded =
                    serde_json::to_value(Pixels { pixels: words }).map_err(|e| e.to_string())?;
                object.insert("pixels".into(), encoded["pixels"].clone());
                count += 1;
            }
        }
        for value in object.values_mut() {
            count += compact(value)?;
        }
    } else if let Some(values) = value.as_array_mut() {
        for value in values {
            count += compact(value)?;
        }
    }
    Ok(count)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 4 {
        return Err("usage: rc-presentation-build GAME_DATA SCENE OUT".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    let raw = fs::read(&args[2])?;
    if !raw.starts_with(b"RCPLAY13") {
        return Err("Expected Opening scene".into());
    }
    let game = rc_game::bundle::decode(&raw)?;
    let n = u32::from_le_bytes(raw[8..12].try_into()?) as usize;
    let mut opening: rc_game::opening::Assets = serde_json::from_slice(&raw[12..12 + n])?;
    if !opening.world_effects.is_empty() {
        return Err("Presentation effects already bound".into());
    }
    let cat = rc_inspect::defaults::load(root)?;
    let data = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let map = read_package(&data)?;
    let teleport = &map.exports[2395 - 1];
    let teleport_props = properties::read(&map, &data, teleport)?;
    for (field, expected) in [
        ("ActorToTeleportTag", "PlayerExplosionFactory"),
        ("DestinationTag", "PlayerCommando"),
    ] {
        if !teleport_props
            .values
            .iter()
            .any(|p| p.name == field && matches!(&p.value, Value::Name(name) if name == expected))
        {
            return Err("Original injury teleport target differs".into());
        }
    }
    let program = opening
        .programs
        .iter_mut()
        .find(|p| p.original == 497)
        .ok_or("Missing injury program")?;
    let event = program
        .actions
        .iter()
        .position(|a| a.original == 2396)
        .ok_or("Missing injury effect event")?;
    if !program.actions.iter().any(|a| a.original == 2395) {
        program.actions.insert(
            event,
            rc_game::opening::Action {
                original: 2395,
                op: rc_game::opening::Op::TeleportEffectToPlayer { original: 823 },
            },
        );
    }
    if let Some(receipts) = opening.provenance["scripts"].as_array_mut() {
        for receipt in receipts.iter_mut().filter(|r| r["original"] == 2395) {
            receipt["operation"] =
                serde_json::to_value(rc_game::opening::Op::TeleportEffectToPlayer {
                    original: 823,
                })?;
            receipt["omitted_reason"] = serde_json::Value::Null;
            receipt["bound_by"] = "map-a-presentation".into();
        }
    }
    let mut door_motion = vec![];
    for id in [34, 35] {
        let e = &map.exports[id - 1];
        let class = map.object_path(e.class)?;
        let props = properties::read(&map, &data, e)?;
        let get = |field, index| {
            cat.instance(&class, &props, field, index, "geo_01a")
                .map(|v| v.value)
        };
        let Value::Float(delay) = get("DelayTime", 0)? else {
            return Err("Door delay".into());
        };
        let Value::Float(seconds) = get("MoveTimes", 0)? else {
            return Err("Door move time".into());
        };
        let Value::Vector(offset) = get("KeyPos", 1)? else {
            return Err("Door key position".into());
        };
        let Value::Rotator(rotation) = get("KeyRot", 1)? else {
            return Err("Door key rotation".into());
        };
        let Value::Byte(glide) = get("MoverGlideType", 0)? else {
            return Err("Door glide type".into());
        };
        if rotation != [0; 3] || glide > 1 {
            return Err("Unsupported original rotating door".into());
        }
        door_motion.push(rc_game::spire_runtime::DoorMotion {
            original: id as u32,
            delay: f64::from(delay),
            seconds: f64::from(seconds),
            glide: glide == 1,
            offset,
        });
    }
    let mut resolver =
        rc_inspect::assets::Assets::new(&[&root.join("Textures"), &root.join("System")])?;
    let mut sounds = game
        .mission
        .as_ref()
        .ok_or("Mission bank")?
        .sounds
        .iter()
        .map(|s| s.source.clone())
        .collect::<Vec<_>>();
    let mut receipts = vec![];
    let mut bindings = vec![];
    for (id, disabled) in [
        (211, None),
        (823, None),
        (822, None),
        (617, Some(617)),
        (442, None),
        (950, None),
        (951, None),
        (1135, None),
        (1136, None),
        (1137, None),
    ] {
        let e = &map.exports[id as usize - 1];
        let class = map.object_path(e.class)?;
        let props = properties::read(&map, &data, e)?;
        let get = |field| {
            cat.instance(&class, &props, field, 0, "geo_01a")
                .map(|v| v.value)
        };
        let Value::Vector(location) = get("Location")? else {
            return Err("Factory location".into());
        };
        let Value::Rotator(rotation) = get("Rotation")? else {
            return Err("Factory rotation".into());
        };
        let source = if id == 617 {
            let field = props
                .values
                .iter()
                .find(|p| p.name == "DestroyedState")
                .ok_or("DestroyedState")?;
            let payload = map.payload(&data, e)?;
            let transition = properties::tagged_struct(
                &map,
                &payload[field.payload_offset..field.payload_offset + field.bytes],
            )?;
            let Value::Object { path, .. } = &transition
                .values
                .iter()
                .find(|p| p.name == "TransitionEffect")
                .ok_or("Transition effect")?
                .value
            else {
                return Err("Transition effect type".into());
            };
            path.clone()
        } else {
            let Value::Object { path, index } = get("Prototype")? else {
                return Err("Factory effect prototype".into());
            };
            if index == 0 {
                return Err("Null factory prototype".into());
            }
            path
        };
        let event = if disabled.is_none() {
            let Value::Name(tag) = get("Tag")? else {
                return Err("Factory event tag".into());
            };
            Some(tag)
        } else {
            None
        };
        let mut role = |field: &str| -> Result<Option<usize>, String> {
            let Value::Object { index, path } = cat.resolve(&source, field, 0)?.value else {
                return Err("Effect sound field".into());
            };
            if index == 0 {
                return Ok(None);
            }
            if let Some(i) = sounds.iter().position(|s| s == &path) {
                return Ok(Some(i));
            }
            let (sound, _) = rc_inspect::mission_assets::sound_variant(root, &path, &mut receipts)
                .map_err(|e| e.to_string())?;
            let i = sounds.len();
            sounds.push(path.clone());
            opening.sound_bindings.push(rc_game::opening::SoundBinding {
                role: i,
                source: path,
                pcm: rc_game::replay::fingerprint(&sound.wave),
            });
            opening.sounds.push(sound);
            Ok(Some(i))
        };
        let sound = role("SpawnSound")?;
        let ambient = role("AmbientSound")?;
        let templates = rc_inspect::particle_assets::effect(root, &mut resolver, &source)?;
        receipts.push(serde_json::json!({"original":id,"class":class,"effect_class":source,"location":location,"rotation":rotation,"event":event,"disabled":disabled,"spawn_sound":sound,"ambient":ambient,"templates":templates.iter().map(|t|t.original).collect::<Vec<_>>()}));
        bindings.push(rc_game::world_effects::Binding {
            teleport_action: (id == 823).then_some(2395),
            original: id,
            source,
            event,
            disabled,
            location,
            rotation,
            templates,
            sound,
            ambient,
        });
    }
    opening.world_effects = bindings;
    opening.provenance["world_effects"] = serde_json::json!({"bindings":receipts,"injury":"175 scripted damage remains separate from original PlayerExplosionFactory823; original ACTION_TeleportToPoint2395 captures PlayerCommando location and pawn yaw before triggering factory823; fixed transform persists through checkpoint","barrier617":"PropDestroyed: original transition effect, no replacement mesh/material and no explosion damage => immediate actor removal","boundary":"Selected original SpriteEmitter assets, portable fixed-tick/range renderer; no general Karma, projector, global RNG or native driver parity"});
    let meta = serde_json::to_vec(&opening)?;
    let mut result = b"RCPLAY13".to_vec();
    result.extend((meta.len() as u32).to_le_bytes());
    result.extend(meta);
    let mut at = 12 + n;
    let mut compact_materials = 0;
    for _ in 0..16 {
        let header = raw.get(at..at + 12).ok_or("Truncated scene layer")?;
        if !header.starts_with(b"RCPLAY") {
            return Err("Invalid scene layer".into());
        }
        let size = u32::from_le_bytes(header[8..12].try_into()?) as usize;
        let mut value: serde_json::Value =
            serde_json::from_slice(raw.get(at + 12..at + 12 + size).ok_or("Layer metadata")?)?;
        if &header[..8] == b"RCPLAY09" {
            value["door_motion"] = serde_json::to_value(&door_motion)?;
            value["provenance"]["door_motion_source"] = serde_json::json!({"function":"AMover::physMovingBrush 104edfb0","curve":"MV_MoveByTime: t; MV_GlideByTime: 3*t*t - 2*t*t*t","geometry_and_collision":"same original KeyPos[1] displacement after DelayTime over MoveTimes[0]","limits":"Selected translation-only exit doors; encroachment/pushing/general mover keys are outside this host"});
        }
        compact_materials += compact(&mut value)?;
        let bytes = serde_json::to_vec(&value)?;
        result.extend_from_slice(&header[..8]);
        result.extend((bytes.len() as u32).to_le_bytes());
        result.extend(bytes);
        at += 12 + size;
        if &header[..8] == b"RCPLAY01" {
            result.extend_from_slice(&raw[at..]);
            break;
        }
    }
    if result.len() > 112 * 1024 * 1024 {
        return Err("Corrected scene exceeds native/Android payload budget".into());
    }
    rc_game::bundle::decode(&result)?;
    fs::create_dir_all(out)?;
    fs::write(out.join("geo_01a.rcplay"), &result)?;
    fs::write(
        out.join("presentation.json"),
        serde_json::to_vec_pretty(&opening.provenance["world_effects"])?,
    )?;
    println!(
        "Bound {} original presentation effects, {} bytes; compacted {compact_materials} material arrays losslessly",
        opening.world_effects.len(),
        result.len()
    );
    Ok(())
}
