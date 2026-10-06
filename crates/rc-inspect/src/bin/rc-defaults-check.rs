//! Audit scalar defaults/Properties overlays and all original start transforms.
use rc_package::{level, properties, read_package};
use std::{env, fs, path::Path};
#[path = "../defaults.rs"]
mod defaults;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("Usage: rc-defaults-check <GameData-directory> <report.json>".into());
    }
    let root = Path::new(&args[1]);
    let catalog = defaults::load(root)?;
    let mut classes = Vec::new();
    for (class, names) in [
        (
            "Engine.PlayerStart",
            vec![
                "Location",
                "Rotation",
                "bEnabled",
                "bSinglePlayerStart",
                "bCoopStart",
                "bPrimaryStart",
            ],
        ),
        (
            "Engine.StaticMeshActor",
            vec![
                "Location",
                "Rotation",
                "bCollideActors",
                "bBlockActors",
                "bBlockPlayers",
                "CollisionRadius",
                "CollisionHeight",
            ],
        ),
        (
            "CTCharacters.PlayerCommando",
            vec![
                "CollisionRadius",
                "CollisionHeight",
                "BaseEyeHeight",
                "MaxHealth",
                "ShieldRechargeRate",
                "ShieldRechargeDelay",
            ],
        ),
    ] {
        let mut values = serde_json::Map::new();
        for name in names {
            values.insert(
                name.into(),
                serde_json::to_value(catalog.resolve(class, name, 0)?)?,
            );
        }
        classes.push(serde_json::json!({"class":class,"values":values}));
    }
    let mut maps = Vec::new();
    let mut total = 0;
    let mut explicit = 0;
    let mut failures = 0;
    let mut files: Vec<_> = fs::read_dir(root.join("Maps"))?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<_, _>>()?;
    files.sort();
    for file in files {
        if !file
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("ctm"))
        {
            continue;
        }
        let audit = (|| -> Result<_, String> {
            let bytes = fs::read(&file).map_err(|e| e.to_string())?;
            let pkg = read_package(&bytes)?;
            let original = level::player_starts(&pkg, &bytes)?;
            let starts = level::player_starts_with_defaults(
                &pkg,
                &bytes,
                &catalog,
                &file.to_string_lossy(),
            )?;
            for start in &original {
                if !starts.contains(start) {
                    return Err("explicit start transform changed".into());
                }
            }
            let mut resolved = Vec::new();
            let mut mesh_flags = Vec::new();
            for (i, export) in pkg.exports.iter().enumerate() {
                let class = pkg.object_path(export.class)?;
                if !(class.eq_ignore_ascii_case("Engine.PlayerStart")
                    || file.file_stem().is_some_and(|s| s == "geo_01a")
                        && class.eq_ignore_ascii_case("Engine.StaticMeshActor"))
                {
                    continue;
                }
                let props = properties::read(&pkg, &bytes, export)?;
                let mut values = serde_json::Map::new();
                let fields: &[&str] = if class.eq_ignore_ascii_case("Engine.PlayerStart") {
                    &["Location", "Rotation"]
                } else {
                    &["bCollideActors", "bBlockActors", "bBlockPlayers"]
                };
                for name in fields {
                    values.insert(
                        (*name).into(),
                        serde_json::to_value(catalog.instance(
                            &class,
                            &props,
                            name,
                            0,
                            &file.to_string_lossy(),
                        )?)
                        .map_err(|e| e.to_string())?,
                    );
                }
                let actor =
                    serde_json::json!({"actor":pkg.object_path(i as i32 + 1)?,"values":values});
                if class.eq_ignore_ascii_case("Engine.PlayerStart") {
                    resolved.push(actor);
                } else {
                    mesh_flags.push(actor);
                }
            }
            Ok((starts, original.len(), resolved, mesh_flags))
        })();
        match audit {
            Ok((starts, count, resolved, mesh_flags)) => {
                total += starts.len();
                explicit += count;
                maps.push(serde_json::json!({"map":file,"anchors":starts,"explicit_anchors":count,"resolved_transforms":resolved,"static_mesh_flags":mesh_flags}));
            }
            Err(error) => {
                failures += 1;
                maps.push(serde_json::json!({"map":file,"error":error}));
            }
        }
    }
    if let Some(parent) = Path::new(&args[2]).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"catalog_classes":catalog.len(),"classes":classes,"maps":maps,"anchors":total,"explicit_anchors":explicit,"added_anchors":total-explicit,"failures":failures,"scope":"Scalar zero/inherited/class/Properties/instance values; config/localized instance values rejected; no compound merge or spawn selection"}),
        )?,
    )?;
    println!(
        "{} classes; {total} resolved anchors ({explicit} explicit); {failures} failures",
        catalog.len()
    );
    if failures > 0 {
        return Err("Default/level audit failed".into());
    }
    Ok(())
}
