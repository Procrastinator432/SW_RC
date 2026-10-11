//! Resolve Geonosian inventory and attack parameters from original package defaults.
use rc_inspect::defaults;
use rc_package::{properties, read_package, skeletal_animation::read_mesh_animation};
use std::{collections::BTreeMap, env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-geonosian-assets GAME_DATA OUTPUT".into());
    }
    let game = Path::new(&args[1]);
    let catalog = defaults::load(game)?;
    let mut classes = BTreeMap::new();
    let mut names: Vec<_> = catalog
        .names()
        .filter(|n| {
            n.to_ascii_lowercase().contains("geonosian")
                || [
                    "CTInventory.ForcePike",
                    "CTInventory.ForcePikeAmmo",
                    "Properties.CTDamageMeleeWarrior",
                    "CTGame.GOAL_MeleeAttack",
                ]
                .iter()
                .any(|selected| n.eq_ignore_ascii_case(selected))
        })
        .collect();
    names.sort();
    for class in names {
        let mut values = BTreeMap::new();
        for property in [
            "ControllerClass",
            "Health",
            "GroundSpeed",
            "SightRadius",
            "PeripheralVision",
            "LungeDist",
            "JumpDist",
            "OffensiveJumpProbability",
            "DefensiveJumpProbability",
            "DefensiveJumpProbabilityForPlayer",
            "MeleeRange",
            "MeleeDamage",
            "MeleeDamageType",
            "AmmoName",
            "FireAnim",
            "ShotDelay",
            "Damage",
            "ProjectileClass",
            "MeleeAnim",
            "MeleeAttackAnim",
            "bMeleeWeapon",
            "ReadyDistance",
            "SafeDistance",
            "ArmorFactor",
            "Shields",
            "DamageScaling",
            "ShieldDamageVariance",
            "MyDamageVariance",
            "bBypassesShields",
            "KDamageImpulse",
            "DamageTypeModifiers",
        ] {
            values.insert(property.to_string(), catalog.resolve(class, property, 0));
        }
        for i in 0..16 {
            values.insert(
                format!("RequiredEquipment[{i}]"),
                catalog.resolve(class, "RequiredEquipment", i),
            );
        }
        classes.insert(class, values);
    }
    let bytes = fs::read(game.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&bytes)?;
    let mut actors = vec![];
    for (i, e) in pkg.exports.iter().enumerate() {
        let class = pkg.object_path(e.class)?;
        if !catalog
            .derives_from(&class, "CTCharacters.GeonosianWarrior")
            .unwrap_or(false)
        {
            continue;
        }
        let props = properties::read(&pkg, &bytes, e)?;
        let mut values = BTreeMap::new();
        for i in 0..16 {
            values.insert(
                format!("RequiredEquipment[{i}]"),
                catalog.instance(&class, &props, "RequiredEquipment", i, "geo_01a.ctm"),
            );
        }
        actors.push(serde_json::json!({"actor":pkg.object_path(i as i32+1)?,"class":class,"inventory":values}));
    }
    let bytes = fs::read(game.join("Animations/geonosian.ukx"))?;
    let pkg = read_package(&bytes)?;
    let mut sequences = vec![];
    let mut notifies = vec![];
    for (i, e) in pkg.exports.iter().enumerate() {
        let path = pkg.object_path(i as i32 + 1)?;
        if path.eq_ignore_ascii_case("GeonosianWarriorSet") {
            let animation = read_mesh_animation(&pkg, &bytes, e)?;
            for s in animation.sequences {
                if ["MeleeAttack", "MeleeAttack2", "Lunge", "Lunge2", "Fire"]
                    .contains(&s.name.name.as_str())
                {
                    sequences.push(serde_json::json!({"name":s.name,"frames":s.frames,"rate_bits":s.rate_bits,"notifies":s.notifies}));
                }
            }
        }
        let class = pkg.object_path(e.class)?;
        if class.contains("AnimNotify") {
            notifies.push(serde_json::json!({"index":i+1,"path":path,"class":class,"properties":properties::read(&pkg,&bytes,e)?}));
        }
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"classes":classes,"map_actors":actors,"sequences":sequences,"notifies":notifies,"scope":"Original resolved defaults, map inventory overrides and attack animation references; unresolved properties retain Err rather than guessed values."}),
        )?,
    )?;
    Ok(())
}
