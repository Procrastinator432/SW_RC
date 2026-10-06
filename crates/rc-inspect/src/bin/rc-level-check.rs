//! Audit explicit start anchors; never infer game-selection rules or class defaults.
use rc_package::{level, properties, read_package};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("Usage: rc-level-check <Maps-directory> <report.json>".into());
    }
    let mut files: Vec<_> = fs::read_dir(&args[1])?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("ctm")))
        .collect();
    files.sort();
    let mut maps = Vec::new();
    let mut anchors = 0;
    let mut failures = 0;
    for file in files {
        let audit = (|| -> Result<_, String> {
            let bytes = fs::read(&file).map_err(|e| e.to_string())?;
            let pkg = read_package(&bytes)?;
            let starts = level::player_starts(&pkg, &bytes)?;
            let mut exports = Vec::new();
            for (i, e) in pkg.exports.iter().enumerate() {
                let class = pkg.object_path(e.class)?;
                if class.ends_with("PlayerStart") {
                    let props = properties::read(&pkg, &bytes, e)?;
                    let selected: Vec<_> = props
                        .values
                        .iter()
                        .filter(|p| {
                            [
                                "Location",
                                "Rotation",
                                "bEnabled",
                                "bSinglePlayerStart",
                                "bPrimaryStart",
                                "bCoopStart",
                                "TeamNumber",
                                "Tag",
                                "Event",
                            ]
                            .contains(&p.name.as_str())
                        })
                        .collect();
                    exports.push(serde_json::json!({"actor":pkg.object_path(i as i32+1)?,"class":class,"explicit_properties":selected}));
                }
            }
            Ok((starts, exports))
        })();
        match audit {
            Ok((starts, exports)) => {
                anchors += starts.len();
                maps.push(serde_json::json!({"map":file,"anchors":starts,"start_exports":exports}));
            }
            Err(error) => {
                failures += 1;
                maps.push(serde_json::json!({"map":file,"error":error}));
            }
        }
    }
    if let Some(parent) = Path::new(&args[2]).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"maps":maps,"anchors":anchors,"failures":failures,"scope":"Exact Engine.PlayerStart with explicit Location and Rotation; no defaults, enabled-state inference or game spawn selection"}),
        )?,
    )?;
    println!("{anchors} explicit anchors; {failures} failures");
    if failures != 0 {
        return Err("Level audit failed; see report".into());
    }
    Ok(())
}
