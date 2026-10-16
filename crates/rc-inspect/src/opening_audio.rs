//! Selected ACTION_PlaySound source semantics, checked against the game's scalar VM/audio path.
use crate::{assets::Assets, defaults, mission_assets};
use rc_game::opening::{self, Op, SoundPolicy};
use rc_package::{
    defaults::Catalog,
    properties::{self, Value},
    read_package,
};
use std::{fs, path::Path};

fn category(
    root: &Path,
    resolver: &mut Assets,
    catalog: &Catalog,
    source: &str,
) -> Result<(u8, String), Box<dyn std::error::Error>> {
    let mut variants = vec![];
    mission_assets::sound_variant(root, source, &mut variants)?;
    let selected = variants
        .last()
        .and_then(|v| v["children"][0].as_str())
        .unwrap_or(source);
    let (class, props) = resolver.material_properties(selected)?;
    let params = catalog.instance(&class, &props, "Params", 0, selected)?;
    let (value, tag) = match params.value {
        Value::Object { index: 0, .. } => (
            catalog.resolve("Engine.AudioParams", "Category", 0)?.value,
            catalog.resolve("Engine.AudioParams", "ActorTag", 0)?.value,
        ),
        Value::Object { index, path } => {
            let package = selected.split_once('.').ok_or("Invalid sound path")?.0;
            let path = if index > 0 && params.stage == "instance_delta" {
                format!("{package}.{path}")
            } else {
                path
            };
            let (class, props) = resolver.material_properties(&path)?;
            (
                catalog
                    .instance(&class, &props, "Category", 0, &path)?
                    .value,
                catalog
                    .instance(&class, &props, "ActorTag", 0, &path)?
                    .value,
            )
        }
        _ => return Err("Invalid original sound Params".into()),
    };
    let Value::Name(tag) = tag else {
        return Err("Invalid original audio anchor tag".into());
    };
    match value {
        Value::Byte(c) if c <= 2 => Ok((c, tag)),
        _ => Err("Invalid original sound category".into()),
    }
}

pub fn configure(
    root: &Path,
    assets: &mut opening::Assets,
    bank: &[rc_game::combat::Sound],
) -> Result<(), Box<dyn std::error::Error>> {
    let data = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&data)?;
    let catalog = defaults::load(root)?;
    let mut resolver = Assets::new(&[&root.join("Sounds"), &root.join("System")])?;
    let mut tags = vec![];
    for (i, e) in pkg.exports.iter().enumerate() {
        let class = pkg.object_path(e.class)?;
        if !catalog
            .derives_from(&class, "Engine.Actor")
            .unwrap_or(false)
        {
            continue;
        }
        let props = properties::read(&pkg, &data, e)?;
        if let Value::Name(tag) = catalog.instance(&class, &props, "Tag", 0, "geo_01a")?.value {
            tags.push((i as i32 + 1, tag));
        }
    }
    assets.sound_policies.clear();
    let mut receipts = vec![];
    for program in &mut assets.programs {
        for action in &mut program.actions {
            let Op::Sound {
                role,
                seconds,
                caption_seconds,
                ..
            } = &mut action.op
            else {
                continue;
            };
            let e = pkg
                .exports
                .get(action.original as usize - 1)
                .ok_or("Invalid original sound action")?;
            let class = pkg.object_path(e.class)?;
            if class != "Gameplay.ACTION_PlaySound" {
                return Err("Opening action class changed".into());
            }
            let props = properties::read(&pkg, &data, e)?;
            let value = |field| {
                catalog
                    .instance(&class, &props, field, 0, "geo_01a")
                    .map(|r| r.value)
            };
            let Value::Name(tag) = value("SoundSourceTag")? else {
                return Err("Invalid sound source tag".into());
            };
            let mut owner = if tag.is_empty() || tag.eq_ignore_ascii_case("None") {
                program.original as i32
            } else {
                tags.iter()
                    .find(|(_, t)| t.eq_ignore_ascii_case(&tag))
                    .map(|(id, _)| *id)
                    .ok_or("Missing original tagged sound source")?
            };
            let Value::Bool(no_override) = value("bDontOverrideOtherSounds")? else {
                return Err("Invalid no-override flag".into());
            };
            let Value::Bool(wait) = value("bWaitForFinish")? else {
                return Err("Invalid sound wait flag".into());
            };
            let source = &bank.get(*role).ok_or("Invalid opening sound role")?.source;
            let Value::Object {
                index,
                path: expected,
            } = value("Sound")?
            else {
                return Err("Invalid original sound reference".into());
            };
            if index == 0 || !source.eq_ignore_ascii_case(&expected) {
                return Err(format!("Opening action {} selects {source}, expected {expected}; rebuild cue identities first", action.original).into());
            }
            let (category, anchor_tag) = category(root, &mut resolver, &catalog, source)?;
            if !anchor_tag.is_empty() && !anchor_tag.eq_ignore_ascii_case("None") {
                owner = tags
                    .iter()
                    .find(|(_, t)| t.eq_ignore_ascii_case(&anchor_tag))
                    .map(|(id, _)| *id)
                    .ok_or("Missing original AudioParams actor anchor")?;
            }
            // ACTION_PlaySound.InitActionFor: C.GetSoundDuration(Sound) + 0.1f.
            *seconds = if wait {
                f64::from((*caption_seconds as f32) + 0.1f32)
            } else {
                0.
            };
            let policy = SoundPolicy {
                role: *role,
                owner,
                category,
                no_override,
            };
            if assets.sound_policies.iter().any(|p| {
                p.role == policy.role
                    && (p.owner != policy.owner
                        || p.no_override != policy.no_override
                        || p.category != policy.category)
            }) {
                return Err("Reused cue requires action-specific sound ownership".into());
            }
            if !assets
                .sound_policies
                .iter()
                .any(|p| p.role == policy.role && p.owner == policy.owner)
            {
                assets.sound_policies.push(policy);
            }
            receipts.push(serde_json::json!({"action":action.original,"script":program.original,"source":source,"source_tag":tag,"audio_anchor_tag":anchor_tag,"policy":policy,"pcm_seconds":caption_seconds,"script_wait":seconds,"original_slot":value("Slot")?,"original_volume":value("Volume")?}));
        }
    }
    assets.provenance["owned_audio"] = serde_json::json!({"actions":receipts,"proof":"ACTION_PlaySound +0.1; execPlayOwnedSound104ea5a0 ignores slot/volume, inverts bNoOverride into0x2000; PlayUSound103d5950 derives ID from owner/category; FindSource103d0d10 protects EXISTING id bit0; SFX/music get unique IDs","boundary":"Selected original opening script sources. Spatial attenuation, randomized pitch/volume, streaming and full SoundMultiple selection remain bounded adapters."});
    Ok(())
}
