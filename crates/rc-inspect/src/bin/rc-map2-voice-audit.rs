//! Resolve selected B dialog policies from actual action/default/audio bindings.
use rc_package::{
    properties::{self, Value},
    read_package,
};
use std::{collections::BTreeMap, env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-map2-voice-audit GAME_DATA SCENE OUTPUT".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let game = rc_game::bundle::decode(&fs::read(&args[2])?)?;
    let map = game.map.as_ref().ok_or("Missing map")?;
    let gameplay = &map.gameplay.as_ref().ok_or("Missing B gameplay")?.assets;
    let data = fs::read(root.join("Maps/geo_01b.ctm"))?;
    let pkg = read_package(&data)?;
    let cat = rc_inspect::defaults::load(root)?;
    let mut resolver =
        rc_inspect::assets::Assets::new(&[&root.join("Sounds"), &root.join("System")])?;
    let mut tags = vec![];
    let mut pawns = vec![];
    for (i, e) in pkg.exports.iter().enumerate() {
        let class = pkg.object_path(e.class)?;
        if !cat.derives_from(&class, "Engine.Actor").unwrap_or(false) {
            continue;
        }
        let props = properties::read(&pkg, &data, e)?;
        if let Value::Name(tag) = cat.instance(&class, &props, "Tag", 0, "geo_01b")?.value {
            tags.push((i as i32 + 1, tag));
        }
        if cat.derives_from(&class, "Engine.Pawn").unwrap_or(false) {
            if let Value::Name(tag) = cat
                .instance(&class, &props, "AIScriptTag", 0, "geo_01b")?
                .value
            {
                pawns.push((i as i32 + 1, tag));
            }
        }
    }
    let resolve = |tag: &str| -> Result<i32, String> {
        let matches: Vec<_> = tags
            .iter()
            .filter(|(_, t)| t.eq_ignore_ascii_case(tag))
            .collect();
        if matches.len() != 1 {
            return Err(format!(
                "Ambiguous/missing B audio tag {tag}: {}",
                matches.len()
            ));
        }
        Ok(matches[0].0)
    };
    let mut policies = BTreeMap::new();
    let mut receipts = vec![];
    for script in &gameplay.scripts {
        for action in script.actions.iter().filter(|a| a.op == "sound") {
            let e = pkg
                .exports
                .get(action.original as usize - 1)
                .ok_or("Action export")?;
            let class = pkg.object_path(e.class)?;
            if class != "Gameplay.ACTION_PlaySound" {
                return Err("Wrong action class".into());
            }
            let props = properties::read(&pkg, &data, e)?;
            let val = |field| {
                cat.instance(&class, &props, field, 0, "geo_01b")
                    .map(|r| r.value)
            };
            let sound = map.assets.sounds.get(action.number).ok_or("Bad role")?;
            if !matches!(val("Sound")?,Value::Object{ref path,..}if path.eq_ignore_ascii_case(&sound.source))
            {
                return Err("Original B cue changed".into());
            }
            let (category, anchor) =
                rc_inspect::opening_audio::category(root, &mut resolver, &cat, &sound.source)?;
            if category != 1 {
                return Err(
                    format!("Selected dialog {} is category {category}", action.original).into(),
                );
            }
            let Value::Name(tag) = val("SoundSourceTag")? else {
                return Err("Source tag".into());
            };
            let Value::Bool(no_override) = val("bDontOverrideOtherSounds")? else {
                return Err("Protection flag".into());
            };
            let Value::Bool(disabled) = val("bDisabled")? else {
                return Err("Disabled flag".into());
            };
            if disabled {
                return Err("Selected disabled action".into());
            }
            let selected = if !anchor.is_empty() && !anchor.eq_ignore_ascii_case("None") {
                &anchor
            } else {
                &tag
            };
            let owner = if !selected.is_empty() && !selected.eq_ignore_ascii_case("None") {
                resolve(selected)?
            } else {
                let se = pkg
                    .exports
                    .get(script.original as usize - 1)
                    .ok_or("Script export")?;
                let sc = pkg.object_path(se.class)?;
                if cat.derives_from(&sc, "Gameplay.ScriptedTrigger")? {
                    script.original as i32
                } else {
                    let script_tag = &tags
                        .iter()
                        .find(|(id, _)| *id == script.original as i32)
                        .ok_or("Script tag")?
                        .1;
                    let found: Vec<_> = pawns
                        .iter()
                        .filter(|(_, tag)| tag.eq_ignore_ascii_case(script_tag))
                        .collect();
                    if found.len() != 1 {
                        return Err(format!(
                            "B script {} has {} pawn controllers",
                            script.original,
                            found.len()
                        )
                        .into());
                    }
                    found[0].0
                }
            };
            let policy = rc_game::opening::SoundPolicy {
                role: action.number,
                owner,
                category,
                no_override,
            };
            if let Some(old) = policies.insert(action.number, policy) {
                if serde_json::to_value(old)? != serde_json::to_value(policy)? {
                    return Err("B cue needs action-specific owner".into());
                }
            }
            receipts.push(serde_json::json!({"script":script.original,"action":action.original,"source":sound.source,"source_tag":tag,"anchor_tag":anchor,"policy":policy,"wait":val("bWaitForFinish")?,"script_seconds_unchanged":action.seconds}));
        }
    }
    let policies: Vec<_> = policies.into_values().collect();
    fs::write(
        out.join("sound-policies.json"),
        serde_json::to_vec(&policies)?,
    )?;
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"actions":receipts,"policies":policies,"scope":"Selected B action source identity, original category/actor anchor/protection; no change to script times or PCM"}),
        )?,
    )?;
    println!("{} B dialog policies", policies.len());
    Ok(())
}
