//! Inspect original damage arrays without silently treating unsupported compound defaults as zero.
use rc_inspect::defaults;
use rc_package::{classes, properties, read_package};
use std::{collections::BTreeMap, env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-damage-assets GAME_DATA OUTPUT".into());
    }
    let game = Path::new(&args[1]);
    let catalog = defaults::load(game)?;
    let mut scalar = BTreeMap::new();
    for class in [
        "CTCharacters.CloneCommando",
        "CTCharacters.GeonosianWarrior",
        "CTInventory.DC17BlasterProj",
        "CTInventory.DC17mBlasterAmmo",
        "Properties.CTDamageMeleeWarrior",
        "Properties.CTDamageEnergyDC17",
    ] {
        let mut values = BTreeMap::new();
        for p in [
            "Health",
            "Shields",
            "MaxShields",
            "ArmorFactor",
            "MyDamageVariance",
            "ShieldDamageVariance",
            "bBypassesShields",
            "DamageScaling",
            "MyDamageType",
            "Damage",
            "ShieldRechargeDelay",
            "ShieldRechargeRate",
        ] {
            values.insert(p, catalog.resolve(class, p, 0));
        }
        scalar.insert(class, values);
    }
    let mut raw = BTreeMap::new();
    for directory in ["System", "Properties"] {
        let mut files = fs::read_dir(game.join(directory))?
            .map(|e| e.map(|e| e.path()))
            .collect::<Result<Vec<_>, _>>()?;
        files.sort();
        for file in files {
            if !file
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("u"))
            {
                continue;
            }
            let data = fs::read(&file)?;
            let pkg = read_package(&data)?;
            let stem = file.file_stem().unwrap().to_string_lossy();
            for (i, export) in pkg.exports.iter().enumerate() {
                if export.class != 0 {
                    continue;
                }
                let name = format!("{stem}.{}", pkg.object_path(i as i32 + 1)?);
                if !catalog
                    .derives_from(&name, "Engine.DamageVariance")
                    .unwrap_or(false)
                    && !catalog
                        .derives_from(&name, "Engine.DamageType")
                        .unwrap_or(false)
                {
                    continue;
                }
                let parent = if export.super_class > 0 {
                    format!("{stem}.{}", pkg.object_path(export.super_class)?)
                } else {
                    pkg.object_path(export.super_class)?
                };
                let defaults = classes::read(&pkg, &data, export)?;
                let payload = pkg.payload(&data, export)?;
                let mut decoded = BTreeMap::new();
                for p in &defaults.properties.values {
                    if p.name == "IncludeTypes" {
                        let mut elements = properties::struct_array(
                            &pkg,
                            &payload[p.payload_offset..p.payload_offset + p.bytes],
                        )?;
                        for element in &mut elements {
                            for field in &mut element.values {
                                if let properties::Value::Object { index, path } = &mut field.value
                                {
                                    if *index > 0 {
                                        *path = format!("{stem}.{path}");
                                    }
                                }
                            }
                        }
                        decoded.insert(p.name.clone(), serde_json::to_value(elements)?);
                    } else if p.name == "ExcludeTypes" {
                        let elements = properties::object_array(
                            &pkg,
                            &payload[p.payload_offset..p.payload_offset + p.bytes],
                        )?
                        .into_iter()
                        .map(|(index, path)| {
                            (
                                index,
                                if index > 0 {
                                    format!("{stem}.{path}")
                                } else {
                                    path
                                },
                            )
                        })
                        .collect::<Vec<_>>();
                        decoded.insert(p.name.clone(), serde_json::to_value(elements)?);
                    }
                }
                raw.insert(name,serde_json::json!({"parent":parent,"source":file,"properties":defaults.properties,"decoded_arrays":decoded}));
            }
        }
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"scalar":scalar,"raw_classes":raw,"scope":"Serialized per-class damage array deltas and ancestry, not inferred compound default merging."}),
        )?,
    )?;
    Ok(())
}
