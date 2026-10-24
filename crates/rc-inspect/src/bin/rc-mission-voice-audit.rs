//! Recover unambiguous original ACTION_PlaySound owners for selected scene cues.
use rc_inspect::{assets::Assets, defaults, opening_audio};
use rc_package::{
    properties::{self, Value},
    read_package,
};
use std::{collections::BTreeMap, env, fs, path::Path};

// These are the selected host's authored action boundaries, not every script in
// the original level. A future general script VM needs action-specific policies.
fn selected_actions(game: &rc_game::Game, role: usize, source: &str) -> Vec<u32> {
    let mut ids = vec![];
    if let Some(opening) = &game.opening {
        for program in &opening.assets.programs {
            for action in &program.actions {
                if matches!(action.op, rc_game::opening::Op::Sound { role: r, .. } if r == role) {
                    ids.push(action.original);
                }
            }
        }
        if let Some(intro) = opening.intro.as_ref().map(|p| &p.assets) {
            for (r, action) in intro.sounds[..2].iter().zip([1259, 1260]) {
                if *r == role {
                    ids.push(action);
                }
            }
        }
    }
    if let Some(r) = &game.rendezvous {
        for program in &r.assets.flow.programs {
            for action in &program.actions {
                if matches!(&action.operation, rc_game::rendezvous::Operation::Sound { source: s, .. } if s == source)
                {
                    ids.push(action.original);
                }
            }
        }
    }
    if let Some(d) = &game.demolition {
        for program in &d.assets.flow.programs {
            for action in &program.actions {
                if matches!(&action.operation, rc_game::demolition::Operation::Sound { source: s, .. } if s == source)
                {
                    ids.push(action.original);
                }
            }
        }
    }
    if let Some(g) = game.mission.as_ref().and_then(|m| m.grenades.as_ref()) {
        for (cue, action) in g.flow.cues[..3].iter().zip([2955, 2956, 2963]) {
            if cue.source == source {
                ids.push(action);
            }
        }
    }
    ids.sort_unstable();
    ids.dedup();
    ids
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if !(4..=5).contains(&args.len()) {
        return Err(
            "usage: rc-mission-voice-audit GAME_DATA SCENE OUT.json [REFRESHED_SCENE]".into(),
        );
    }
    let root = Path::new(&args[1]);
    let game = rc_game::bundle::decode(&fs::read(&args[2])?)?;
    let data = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&data)?;
    let cat = defaults::load(root)?;
    let mut resolver = Assets::new(&[&root.join("Sounds"), &root.join("System")])?;
    let mut tags = vec![];
    let mut scripts = BTreeMap::new();
    let mut pawns = vec![];
    let mut spawn_receipts = vec![];
    for (i, e) in pkg.exports.iter().enumerate() {
        let class = pkg.object_path(e.class)?;
        if !cat.derives_from(&class, "Engine.Actor").unwrap_or(false) {
            continue;
        }
        let props = properties::read(&pkg, &data, e)?;
        if let Value::Name(tag) = cat.instance(&class, &props, "Tag", 0, "geo_01a")?.value {
            tags.push((i as i32 + 1, tag));
        }
        if cat.derives_from(&class, "Engine.Pawn").unwrap_or(false) {
            let value = cat.instance(&class, &props, "AIScriptTag", 0, "geo_01a")?;
            if let Value::Name(tag) = value.value {
                if !tag.is_empty() && !tag.eq_ignore_ascii_case("None") {
                    pawns.push((i as i32 + 1, tag));
                }
            }
        }
        for field in props.values.iter().filter(|p| p.name == "Actions") {
            let raw = pkg.payload(&data, e)?;
            for (id, _) in properties::object_array(
                &pkg,
                &raw[field.payload_offset..field.payload_offset + field.bytes],
            )? {
                scripts.insert(
                    id,
                    (
                        i as i32 + 1,
                        cat.derives_from(&class, "Gameplay.ScriptedTrigger")
                            .unwrap_or(false),
                    ),
                );
            }
        }
    }
    // A spawned companion has no map export index. Keep a stable port identity
    // in a namespace disjoint from map actor IDs; this is not an original pointer.
    if let Some(r) = &game.rendezvous {
        let factory = r.assets.provenance["factory"]
            .as_u64()
            .ok_or("Missing selected companion factory")?;
        let expected = r.assets.provenance["spawn_class"]
            .as_str()
            .ok_or("Missing selected companion class")?;
        if factory == 0 || factory > 0x3fff_ffff {
            return Err("Invalid companion factory identity".into());
        }
        let e = pkg
            .exports
            .get(factory as usize - 1)
            .ok_or("Companion factory export")?;
        let class = pkg.object_path(e.class)?;
        if !cat.derives_from(&class, "CTGame.PawnFactory")? {
            return Err("Companion factory class differs".into());
        }
        let props = properties::read(&pkg, &data, e)?;
        let val = |field| {
            cat.instance(&class, &props, field, 0, "geo_01a")
                .map(|v| v.value)
        };
        let Value::Object {
            path: prototype, ..
        } = val("Prototype")?
        else {
            return Err("Companion prototype".into());
        };
        let Value::Name(tag) = val("PawnTagFromFactory")? else {
            return Err("Companion factory tag".into());
        };
        if !prototype.eq_ignore_ascii_case(expected)
            || !matches!(val("Capacity")?, Value::Int(1))
            || tag.is_empty()
            || tag.eq_ignore_ascii_case("None")
        {
            return Err("Selected companion factory contract differs".into());
        }
        if tags.iter().any(|(_, t)| t.eq_ignore_ascii_case(&tag)) {
            return Err("Companion tag conflicts with placed actor".into());
        }
        let owner = (0x4000_0000 | factory as u32) as i32;
        tags.push((owner, tag.clone()));
        spawn_receipts.push(serde_json::json!({"factory":factory,"prototype":prototype,"pawn_tag":tag,"capacity":1,"port_owner":owner,"boundary":"Stable identity for the selected persistent companion; not a native UObject pointer or general factory lifecycle."}));
    }
    let mut rows = vec![];
    for (role, sound) in game
        .mission
        .as_ref()
        .ok_or("Mission")?
        .sounds
        .iter()
        .enumerate()
    {
        let (category, anchor) = opening_audio::category(root, &mut resolver, &cat, &sound.source)?;
        if category != 1 {
            continue;
        }
        let mut actions = vec![];
        let selected_ids = selected_actions(&game, role, &sound.source);
        for (i, e) in pkg.exports.iter().enumerate() {
            let class = pkg.object_path(e.class)?;
            if class != "Gameplay.ACTION_PlaySound" {
                continue;
            }
            let props = properties::read(&pkg, &data, e)?;
            let val = |field| {
                cat.instance(&class, &props, field, 0, "geo_01a")
                    .map(|v| v.value)
            };
            let Value::Object { path, .. } = val("Sound")? else {
                continue;
            };
            if !path.eq_ignore_ascii_case(&sound.source) {
                continue;
            }
            let Value::Name(tag) = val("SoundSourceTag")? else {
                return Err("Sound tag".into());
            };
            let Value::Bool(protected) = val("bDontOverrideOtherSounds")? else {
                return Err("Sound flag".into());
            };
            let Value::Bool(disabled) = val("bDisabled")? else {
                return Err("Disabled flag".into());
            };
            let script = scripts.get(&(i as i32 + 1)).copied();
            let selected = if !anchor.is_empty() && !anchor.eq_ignore_ascii_case("None") {
                &anchor
            } else {
                &tag
            };
            let owner = if selected.is_empty() || selected.eq_ignore_ascii_case("None") {
                script.and_then(|(id, trigger)| {
                    if trigger {
                        return Some(id);
                    }
                    let tag = &tags.iter().find(|(actor, _)| *actor == id)?.1;
                    let matches: Vec<_> = pawns
                        .iter()
                        .filter(|(_, t)| t.eq_ignore_ascii_case(tag))
                        .collect();
                    (matches.len() == 1).then(|| matches[0].0)
                })
            } else {
                tags.iter()
                    .find(|(_, t)| t.eq_ignore_ascii_case(selected))
                    .map(|(id, _)| *id)
            };
            actions.push(serde_json::json!({"action":i+1,"script":script,"source_tag":tag,"anchor":anchor,"owner":owner,"no_override":protected,"disabled":disabled,"selected":selected_ids.contains(&(i as u32+1)),"script_tag":script.and_then(|(id,_)|tags.iter().find(|(actor,_)|*actor==id).map(|(_,t)|t))}));
        }
        if actions.is_empty() {
            if let Some(intro) = game
                .opening
                .as_ref()
                .and_then(|o| o.intro.as_ref().map(|p| &p.assets))
            {
                if intro.sounds[3] == role && intro.actors.iter().any(|a| a.original == 530) {
                    let raw = fs::read(root.join("Animations/Geonosian.ukx"))?;
                    let p = read_package(&raw)?;
                    let notify = p.exports.get(190).ok_or("Warrior voice notify")?;
                    let class = p.object_path(notify.class)?;
                    let props = properties::read(&p, &raw, notify)?;
                    let value = cat.instance(&class, &props, "Sound", 0, "Geonosian")?.value;
                    if class != "Engine.AnimNotify_Sound"
                        || !matches!(value,Value::Object {ref path,..}if path.eq_ignore_ascii_case(&sound.source))
                    {
                        return Err("Selected warrior voice notify differs".into());
                    }
                    actions.push(serde_json::json!({"animation_notify":191,"package":"Geonosian.ukx","owner":530,"no_override":false,"disabled":false,"selected":true}));
                }
            }
        }
        if selected_ids.iter().any(|id| {
            !actions
                .iter()
                .any(|a| a["action"].as_u64() == Some(u64::from(*id)) && a["disabled"] == false)
        }) {
            return Err(format!(
                "Selected speech context does not match enabled original action: {source}",
                source = sound.source
            )
            .into());
        }
        rows.push(serde_json::json!({"role":role,"source":sound.source,"category":category,"actions":actions,"selected_actions":selected_ids,"spawned_owners":spawn_receipts}));
    }
    fs::write(&args[3], serde_json::to_vec_pretty(&rows)?)?;
    if args.len() == 5 {
        let bytes = fs::read(&args[2])?;
        if !bytes.starts_with(b"RCPLAY13") {
            return Err("Expected opening wrapper".into());
        }
        let n = u32::from_le_bytes(bytes[8..12].try_into()?) as usize;
        let mut opening: rc_game::opening::Assets = serde_json::from_slice(&bytes[12..12 + n])?;
        let bank = &game.mission.as_ref().ok_or("Mission")?.sounds;
        let mut added = vec![];
        for row in &rows {
            let role = row["role"].as_u64().ok_or("Role")? as usize;
            if opening.sound_policies.iter().any(|p| p.role == role) {
                continue;
            }
            let all = row["actions"].as_array().ok_or("Actions")?;
            let selected: Vec<_> = all.iter().filter(|a| a["selected"] == true).collect();
            let actions: Vec<_> = if selected.is_empty() {
                all.iter().collect()
            } else {
                selected
            };
            if actions.is_empty() || actions.iter().any(|a| a["owner"].is_null()) {
                continue;
            }
            let owner = actions[0]["owner"].as_i64().ok_or("Owner")? as i32;
            let protected = actions[0]["no_override"].as_bool().ok_or("Flag")?;
            if actions.iter().any(|a| {
                a["owner"].as_i64() != Some(i64::from(owner))
                    || a["no_override"].as_bool() != Some(protected)
            }) {
                continue;
            }
            opening.sound_policies.push(rc_game::opening::SoundPolicy {
                role,
                owner,
                category: 1,
                no_override: protected,
            });
            if !opening.sound_bindings.iter().any(|b| b.role == role) {
                opening.sound_bindings.push(rc_game::opening::SoundBinding {
                    role,
                    source: bank[role].source.clone(),
                    pcm: rc_game::replay::fingerprint(&bank[role].wave),
                });
            }
            added.push(row.clone());
        }
        if !added.is_empty() {
            // Preserve earlier provenance and describe this refinement separately.
            opening.provenance["selected_voice_contexts"] = serde_json::json!({"added":added,"rule":"Selected host action contexts are verified against original ACTION_PlaySound data; unselected uses remain in the audit. The persistent factory companion uses an explicit stable port owner namespace.","spawned_owners":spawn_receipts});
        }
        let meta = serde_json::to_vec(&opening)?;
        let result = [
            b"RCPLAY13".as_slice(),
            &(meta.len() as u32).to_le_bytes(),
            &meta,
            &bytes[12 + n..],
        ]
        .concat();
        rc_game::bundle::decode(&result)?;
        fs::write(&args[4], result)?;
        println!("{} original voice owner bindings added", added.len());
    }
    println!("{} speech cues audited", rows.len());
    Ok(())
}
