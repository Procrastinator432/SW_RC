//! Class inheritance and bounded serialized default-property audit.
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::Path,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() < 3 {
        return Err("Usage: rc-class-check <package-directory> <report.json> [additional-package-directory ...]".into());
    }
    let mut files = Vec::new();
    for directory in std::iter::once(&args[1]).chain(args[3..].iter()) {
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            if path
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("u"))
            {
                files.push(path);
            }
        }
    }
    files.sort();
    let mut classes = HashMap::new();
    let mut defaults_failures = 0;
    let mut default_tags = 0;
    for file in files {
        let data = fs::read(&file)?;
        let pkg = rc_package::read_package(&data)?;
        let package = file
            .file_stem()
            .and_then(|p| p.to_str())
            .ok_or("Invalid package filename")?;
        for (i, e) in pkg.exports.iter().enumerate() {
            if e.class != 0 {
                continue;
            }
            let name = format!("{package}.{}", pkg.object_path(i as i32 + 1)?);
            let parent = if e.super_class == 0 {
                None
            } else {
                let path = pkg.object_path(e.super_class)?;
                Some(if e.super_class > 0 {
                    format!("{package}.{path}")
                } else {
                    path
                })
            };
            let defaults = match rc_package::classes::read(&pkg, &data, e) {
                Ok(value) => {
                    default_tags += value.properties.values.len();
                    serde_json::to_value(value)?
                }
                Err(error) => {
                    defaults_failures += 1;
                    serde_json::json!({"error": error})
                }
            };
            if classes
                .insert(
                    name.to_lowercase(),
                    (
                        name,
                        parent,
                        file.clone(),
                        e.serial_offset,
                        e.serial_size,
                        defaults,
                    ),
                )
                .is_some()
            {
                return Err("Duplicate class path".into());
            }
        }
    }
    let mut keys: Vec<_> = classes.keys().cloned().collect();
    keys.sort();
    let mut report = Vec::new();
    let mut unresolved = 0;
    let mut cycles = 0;
    for key in keys {
        let (name, parent, file, offset, size, defaults) = &classes[&key];
        let mut ancestry = Vec::new();
        let mut seen = HashSet::new();
        let mut next = Some(key.clone());
        let mut error = None;
        while let Some(key) = next {
            if !seen.insert(key.clone()) {
                cycles += 1;
                error = Some("Inheritance cycle");
                break;
            }
            let Some((name, parent, _, _, _, _)) = classes.get(&key) else {
                unresolved += 1;
                error = Some("Parent package/class missing");
                break;
            };
            ancestry.push(name.clone());
            next = parent.as_ref().map(|p| p.to_lowercase());
        }
        let mut critical_defaults = serde_json::Map::new();
        for property_name in [
            "Location",
            "Rotation",
            "bCollideActors",
            "bBlockActors",
            "bBlockPlayers",
            "CollisionRadius",
            "CollisionHeight",
            "BaseEyeHeight",
        ] {
            let mut resolved = serde_json::json!({"status":"unresolved","reason":"No serialized value in ancestry; native initialization not inferred"});
            for declaring in &ancestry {
                let (_, _, _, _, _, class_defaults) = &classes[&declaring.to_lowercase()];
                if let Some(error) = class_defaults.get("error") {
                    resolved = serde_json::json!({"status":"unresolved","declaring_class":declaring,"reason":error});
                    break;
                }
                if let Some(property) =
                    class_defaults["properties"]["values"]
                        .as_array()
                        .and_then(|values| {
                            values
                                .iter()
                                .find(|p| p["name"] == property_name && p["array_index"] == 0)
                        })
                {
                    resolved = serde_json::json!({"status":"serialized","declaring_class":declaring,"source_package":classes[&declaring.to_lowercase()].2,"property":property});
                    break;
                }
            }
            critical_defaults.insert(property_name.into(), resolved);
        }
        report.push(serde_json::json!({"class":name,"parent":parent,"ancestry":ancestry,"source_package":file,"serial_offset":offset,"serial_size":size,"error":error,"default_properties":defaults,"critical_serialized_defaults":critical_defaults}));
    }
    if let Some(parent) = Path::new(&args[2]).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"classes":report,"unresolved_chains":unresolved,"cycles":cycles,"default_tags":default_tags,"defaults_failures":defaults_failures,"scope":"Serialized class default deltas and inheritance metadata; no native constructor, config/localization overrides or bytecode execution"}),
        )?,
    )?;
    println!(
        "{} classes; {unresolved} unresolved chains; {cycles} cycles",
        classes.len()
    );
    println!("{default_tags} serialized default tags; {defaults_failures} default-reader failures");
    if cycles > 0 || defaults_failures > 0 || unresolved > 0 {
        return Err("Class audit failures; see report".into());
    }
    Ok(())
}
