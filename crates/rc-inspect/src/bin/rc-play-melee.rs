//! Extend the pinned combat bundle with original attack clips and selected damage lists.
use rc_game::{bundle, combat, melee, visual};
use rc_inspect::defaults;
use rc_package::{properties::Value, read_package, skeletal_animation::read_mesh_animation};
use serde_json::Value as Json;
use std::{collections::BTreeMap, env, fs, path::Path};
fn split<'a>(bytes: &'a [u8], magic: &[u8; 8]) -> Result<(&'a [u8], &'a [u8]), String> {
    if !bytes.starts_with(magic) || bytes.len() < 12 {
        return Err("Invalid input bundle layer".into());
    }
    let size = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    Ok((
        bytes.get(12..12 + size).ok_or("Truncated input layer")?,
        bytes.get(12 + size..).ok_or("Missing base layer")?,
    ))
}
fn number(c: &rc_package::defaults::Catalog, class: &str, property: &str) -> Result<f32, String> {
    match c.resolve(class, property, 0)?.value {
        Value::Float(v) => Ok(v),
        Value::Int(v) => Ok(v as f32),
        _ => Err(format!("Not numeric {class}.{property}")),
    }
}
fn reference(
    c: &rc_package::defaults::Catalog,
    class: &str,
    property: &str,
) -> Result<String, String> {
    match c.resolve(class, property, 0)?.value {
        Value::Object { path, .. } => Ok(path),
        _ => Err(format!("Not a reference {class}.{property}")),
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 5 {
        return Err(
            "usage: rc-play-melee GAME_DATA COMBAT_SCENE DAMAGE_REPORT OUTPUT_DIRECTORY".into(),
        );
    }
    let game = Path::new(&args[1]);
    let out = Path::new(&args[4]);
    fs::create_dir_all(out)?;
    let bytes = fs::read(&args[2])?;
    let (metadata, base) = split(&bytes, b"RCPLAY03")?;
    let mut combat: combat::Assets = serde_json::from_slice(metadata)?;
    if combat.melee.is_some() {
        return Err("Input already contains melee rules".into());
    }
    let (metadata, world) = split(base, b"RCPLAY02")?;
    let mut visual: visual::Assets = serde_json::from_slice(metadata)?;
    let original = fs::read(game.join("Animations/geonosian.ukx"))?;
    let pkg = read_package(&original)?;
    let e = pkg
        .exports
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            pkg.object_path(i as i32 + 1)
                .is_ok_and(|p| p == "GeonosianWarriorSet")
                .then_some(e)
        })
        .ok_or("Missing warrior animation")?;
    let animation = read_mesh_animation(&pkg, &original, e)?;
    let mut attacks = vec![];
    for name in ["MeleeAttack", "MeleeAttack2", "Lunge", "Lunge2"] {
        let sequence = animation
            .sequences
            .iter()
            .find(|s| s.name.name == name)
            .ok_or("Missing original attack")?;
        let mut hits = vec![];
        for n in &sequence.notifies {
            if n.object_index > 0
                && pkg.object_path(pkg.exports[n.object_index as usize - 1].class)?
                    == "Engine.AnimNotify_MeleeHit"
            {
                hits.push(n);
            }
        }
        if hits.len() != 1 {
            return Err("Unsupported attack hit count".into());
        }
        attacks.push(melee::AttackClip {
            name: name.into(),
            hit_time: f32::from_bits(hits[0].time_bits),
            notify: hits[0].object_index,
        });
        visual.enemy.clips.push(visual::Clip {
            source: format!("geonosian.GeonosianWarriorSet:{name}"),
            mapping: visual.enemy.clips[0].mapping.clone(),
            sequence: serde_json::from_value(serde_json::to_value(sequence)?)?,
        });
    }
    let report: Json = serde_json::from_slice(&fs::read(&args[3])?)?;
    let classes = report["raw_classes"]
        .as_object()
        .ok_or("Missing class report")?;
    let table: BTreeMap<_, _> = classes.iter().map(|(k, v)| (k.to_lowercase(), v)).collect();
    let ids: BTreeMap<_, _> = table
        .keys()
        .enumerate()
        .map(|(i, k)| (k.clone(), i as u32 + 1))
        .collect();
    let class_id = |name: &str| -> Result<u32, String> {
        ids.get(&name.to_lowercase())
            .copied()
            .ok_or_else(|| format!("Unresolved damage class {name}"))
    };
    let ancestry = |name: &str| -> Result<Vec<u32>, String> {
        let mut result = vec![];
        let mut key = name.to_lowercase();
        while let Some(class) = table.get(&key) {
            let id = class_id(&key)?;
            if result.contains(&id) {
                return Err("Damage ancestry cycle".into());
            }
            result.push(id);
            key = class["parent"]
                .as_str()
                .ok_or("Missing damage parent")?
                .to_lowercase();
        }
        if key != "core.object" {
            return Err("Incomplete damage ancestry".into());
        }
        Ok(result)
    };
    let defense = |name: &str, armor: f32| -> Result<melee::Defense, Box<dyn std::error::Error>> {
        let class = table
            .get(&name.to_lowercase())
            .ok_or("Missing variance class")?;
        let arrays = &class["decoded_arrays"];
        let mut include = vec![];
        for row in arrays["IncludeTypes"]
            .as_array()
            .ok_or("Missing selected include list")?
        {
            let fields: BTreeMap<_, _> = row["values"]
                .as_array()
                .ok_or("Missing include fields")?
                .iter()
                .map(|p| (p["name"].as_str().unwrap_or(""), &p["value"]["value"]))
                .collect();
            let object = fields
                .get("DamageTypeClass")
                .ok_or("Missing include class")?;
            let class = if object["index"].as_i64() == Some(0) {
                None
            } else {
                Some(class_id(
                    object["path"].as_str().ok_or("Missing class path")?,
                )?)
            };
            include.push(melee::DamageRule {
                class,
                multiplier: fields
                    .get("Multiplier")
                    .and_then(|v| v.as_f64())
                    .ok_or("Missing include factor")? as f32,
            });
        }
        let mut exclude = vec![];
        if let Some(rows) = arrays["ExcludeTypes"].as_array() {
            for row in rows {
                exclude.push(if row[0].as_i64() == Some(0) {
                    None
                } else {
                    Some(class_id(row[1].as_str().ok_or("Missing exclusion path")?)?)
                });
            }
        } else {
            // Selected Warrior ancestry has no inherited exclusion payload.
            let mut key = class["parent"]
                .as_str()
                .ok_or("Missing variance parent")?
                .to_lowercase();
            while let Some(parent) = table.get(&key) {
                if parent["decoded_arrays"]["ExcludeTypes"].is_array() {
                    return Err("Inherited exclusions need explicit resolution".into());
                }
                key = parent["parent"]
                    .as_str()
                    .ok_or("Missing variance parent")?
                    .to_lowercase();
            }
        }
        Ok(melee::Defense {
            armor,
            include,
            exclude,
        })
    };
    let catalog = defaults::load(game)?;
    let player = "CTCharacters.CloneCommando";
    let enemy = "CTCharacters.GeonosianWarrior";
    let pike = "CTInventory.ForcePike";
    for class in [player, enemy] {
        if reference(&catalog, class, "ShieldDamageVariance")?.to_lowercase() != "none" {
            return Err("Separate shield variance needs profile support".into());
        }
    }
    let projectile_type = reference(&catalog, "CTInventory.DC17BlasterProj", "MyDamageType")?;
    let melee_type = reference(&catalog, pike, "MeleeDamageType")?;
    let bypass = |class: &str| -> Result<bool, String> {
        match catalog.resolve(class, "bBypassesShields", 0)?.value {
            Value::Bool(v) => Ok(v),
            _ => Err("Invalid bypass flag".into()),
        }
    };
    combat.melee = Some(melee::Rules {
        provenance: serde_json::json!({"class_ids":ids,"damage_report":report,"projectile_type":projectile_type,"melee_type":melee_type,"adapters":"direct approach 65/s, sight 1600, idle turning, 2s attack recovery, alternating two normal melee clips, identity Game.ReduceDamage; jumps/lunges disabled; no native CDO construction"}),
        player_health: number(&catalog, player, "Health")?,
        player_shields: number(&catalog, player, "Shields")?,
        shield_delay: number(&catalog, player, "ShieldRechargeDelay")?,
        shield_rate: number(&catalog, player, "ShieldRechargeRate")?,
        player: defense(
            &reference(&catalog, player, "MyDamageVariance")?,
            number(&catalog, player, "ArmorFactor")?,
        )?,
        enemy: defense(
            &reference(&catalog, enemy, "MyDamageVariance")?,
            number(&catalog, enemy, "ArmorFactor")?,
        )?,
        projectile_type: ancestry(&projectile_type)?,
        projectile_bypass: bypass(&projectile_type)?,
        melee_type: ancestry(&melee_type)?,
        melee_bypass: bypass(&melee_type)?,
        melee_damage: number(&catalog, pike, "MeleeDamage")?,
        melee_range: f64::from(number(&catalog, pike, "MeleeRange")?),
        clips: attacks,
    });
    let animated = bundle::encode_animated(world, &visual)?;
    let result = bundle::encode_combat(&animated, &combat)?;
    bundle::decode(&result)?;
    fs::write(
        out.join("melee-receipt.json"),
        serde_json::to_vec_pretty(&combat.melee)?,
    )?;
    fs::write(out.join("geo_01a-melee.rcplay"), result)?;
    Ok(())
}
