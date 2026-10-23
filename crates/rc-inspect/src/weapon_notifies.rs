//! Original DC-15s HUD notify bindings. Existing grenade host owns throw events.
use rc_game::{
    visual::Mesh,
    weapon_animation::{Foley, SoundNotify},
};
use rc_package::properties::{self, Value};
use std::{fs, path::Path};
pub fn load(
    root: &Path,
    mesh: &Mesh,
) -> Result<(Foley, serde_json::Value), Box<dyn std::error::Error>> {
    if mesh.source != "hudarms.HudArmsDC15s" {
        return Err("Weapon notify importer requires original HudArmsDC15s".into());
    }
    let data = fs::read(root.join("Animations/hudarms.ukx"))?;
    let package = rc_package::read_package(&data)?;
    let mut foley = Foley {
        sounds: vec![],
        groups: vec![],
        notifies: vec![],
    };
    let mut receipts = vec![];
    let mut variants = vec![];
    let catalog = crate::defaults::load(root)?;
    for clip in &mesh.clips {
        let name = &clip.sequence.name.name;
        if !clip.source.starts_with("hudarms.HudArmsDC15sSet:") {
            return Err("Weapon notify animation source differs".into());
        }
        for n in &clip.sequence.notifies {
            let index = n.object_index.checked_sub(1).ok_or("Null notify")? as usize;
            let export = package.exports.get(index).ok_or("Invalid notify")?;
            let class = package.object_path(export.class)?;
            let props = properties::read(&package, &data, export)?;
            let dispatch = if name == "ThrowGrenade" {
                "existing throw_runtime Foley/release host"
            } else {
                if !["Run", "Idle"].contains(&name.as_str()) || class != "Engine.AnimNotify_Sound" {
                    return Err(format!("Unsupported weapon notify {name}: {class}").into());
                }
                let path = props
                    .values
                    .iter()
                    .find_map(|p| match &p.value {
                        Value::Object { path, index } if p.name == "Sound" && *index != 0 => {
                            Some(path)
                        }
                        _ => None,
                    })
                    .ok_or("Missing original notify Sound")?;
                let (sound, group) = bind(root, path, &catalog, &mut foley, &mut variants)?;
                foley.notifies.push(SoundNotify {
                    group,
                    clip: name.clone(),
                    original: n.object_index,
                    time: f32::from_bits(n.time_bits),
                    sound,
                });
                "weapon base-channel Foley"
            };
            receipts.push(serde_json::json!({"clip":clip.source,"time_bits":n.time_bits,"time":f32::from_bits(n.time_bits),"object":package.object_path(n.object_index)?,"original":n.object_index,"class":class,"properties":props,"dispatch":dispatch}));
        }
    }
    let sounds: Vec<_> = foley.sounds.iter().map(|s| serde_json::json!({"source":s.source,"pcm":rc_game::replay::fingerprint(&s.wave),"bytes":s.wave.len()})).collect();
    Ok((
        foley,
        serde_json::json!({"notifies":receipts,"sound_variants":variants,"sounds":sounds,"variant_policy":"Original SelectSound/appSRand on flat unrestricted weapon groups; saved local stream, no global RNG/spatialization parity"}),
    ))
}

fn bind(
    root: &Path,
    path: &str,
    catalog: &rc_package::defaults::Catalog,
    foley: &mut Foley,
    receipts: &mut Vec<serde_json::Value>,
) -> Result<(usize, Option<usize>), Box<dyn std::error::Error>> {
    if let Some(i) = foley.groups.iter().position(|g| g.source == path) {
        return Ok((
            foley.groups[i]
                .members
                .iter()
                .flatten()
                .next()
                .ok_or("Empty weapon group")?
                .sound,
            Some(i),
        ));
    }
    if let Some(i) = foley.sounds.iter().position(|s| s.source == path) {
        return Ok((i, None));
    }
    let (package, object) = path.split_once('.').ok_or("Invalid audio path")?;
    let data = fs::read(root.join("Sounds").join(format!("{package}.uax")))?;
    let p = rc_package::read_package(&data)?;
    let e = p
        .exports
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            p.object_path(i as i32 + 1)
                .is_ok_and(|s| s.eq_ignore_ascii_case(object))
                .then_some(e)
        })
        .ok_or("Missing weapon sound")?;
    let class = p.object_path(e.class)?;
    if class == "Engine.Sound" {
        let sound = crate::mission_assets::sound(root, path)?.0;
        let i = foley.sounds.len();
        foley.sounds.push(sound);
        return Ok((i, None));
    }
    if class != "Engine.SoundMultiple" {
        return Err(format!("Unsupported weapon sound {class}").into());
    }
    let props = properties::read(&p, &data, e)?;
    let first = catalog.instance("Engine.SoundMultiple", &props, "FirstSound", 0, path)?;
    if !matches!(first.value, Value::Object { index: 0, .. }) {
        return Err("Weapon group FirstSound host unsupported".into());
    }
    let (ids, style) =
        rc_package::sound_multiple::read_members(p.payload(&data, e)?, props.native_offset)?;
    if ids.is_empty() {
        return Err("Empty weapon sound group".into());
    }
    let mut group = rc_package::sound_multiple::Group {
        source: path.into(),
        in_order: style == 1,
        first: None,
        members: vec![],
    };
    let mut members = vec![];
    for id in ids {
        if id <= 0 {
            return Err("Weapon group requires local direct sound children".into());
        }
        let child = p
            .exports
            .get(id as usize - 1)
            .ok_or("Invalid weapon sound child")?;
        if p.object_path(child.class)? != "Engine.Sound" {
            return Err("Nested weapon group unsupported".into());
        }
        let source = format!("{package}.{}", p.object_path(id)?);
        let props = properties::read(&p, &data, child)?;
        let mut resolved = serde_json::Map::new();
        let mut values = vec![];
        for name in ["BattleContext", "MinCampaign", "MaxCampaign", "Likelihood"] {
            let r = catalog.instance("Engine.Sound", &props, name, 0, &source)?;
            values.push(r.value.clone());
            resolved.insert(name.into(), serde_json::to_value(r)?);
        }
        let [Value::Byte(battle), Value::Byte(minimum), Value::Byte(maximum), Value::Float(likelihood)] =
            values.as_slice()
        else {
            return Err("Unexpected original sound property types".into());
        };
        if (*battle, *minimum, *maximum) != (0, 0, 255) {
            return Err("Weapon group requires unrestricted original children".into());
        }
        let sound = crate::mission_assets::sound(root, &source)?.0;
        let index = foley.sounds.len();
        foley.sounds.push(sound);
        group.members.push(Some(rc_package::sound_multiple::Member {
            sound: index,
            battle: *battle,
            minimum: *minimum,
            maximum: *maximum,
            likelihood: *likelihood,
        }));
        members.push(
            serde_json::json!({"original":id,"source":source,"sound":index,"properties":resolved}),
        );
    }
    group.validate(foley.sounds.len())?;
    let sound = group.members[0].as_ref().unwrap().sound;
    let i = foley.groups.len();
    receipts.push(
        serde_json::json!({"group":path,"selection_style":style,"first":first,"members":members}),
    );
    foley.groups.push(group);
    Ok((sound, Some(i)))
}
