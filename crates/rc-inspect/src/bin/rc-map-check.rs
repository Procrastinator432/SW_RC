use serde_json::json;
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("Usage: rc-map-check <Maps-directory> <report.json>".into());
    }
    let mut files: Vec<_> = fs::read_dir(&args[1])?.collect::<Result<_, _>>()?;
    files.sort_by_key(|e| e.file_name());
    let mut reports = Vec::new();
    let mut failed = 0;
    for f in files {
        let path = f.path();
        if path.extension().and_then(|e| e.to_str()) != Some("ctm") {
            continue;
        }
        let data = fs::read(&path)?;
        let pkg = rc_package::read_package(&data)?;
        let mut errors = Vec::new();
        let mut properties = 0;
        let mut models = 0;
        let mut triangles = 0;
        for (i, e) in pkg.exports.iter().enumerate() {
            if e.class == 0 {
                continue;
            }
            let object = pkg.object_path(i as i32 + 1)?;
            let class = pkg.object_path(e.class)?;
            match rc_package::properties::read(&pkg, &data, e) {
                Ok(_) => properties += 1,
                Err(error) => {
                    errors.push(json!({"object":object,"phase":"properties","error":error}))
                }
            }
            if class == "Engine.Model" {
                match rc_package::geometry::read_bsp(&pkg, &data, e) {
                    Ok(bsp) => {
                        models += 1;
                        triangles += bsp.triangles()?.len();
                    }
                    Err(error) => errors.push(json!({"object":object,"phase":"bsp","error":error})),
                }
            }
        }
        if !errors.is_empty() {
            failed += 1;
        }
        reports.push(json!({"map":f.file_name().to_string_lossy(),"version":pkg.summary.version,"exports":pkg.exports.len(),"property_lists":properties,"bsp_models":models,"bsp_triangles":triangles,"errors":errors}));
    }
    let report = json!({"maps":reports,"failed_maps":failed});
    let destination = Path::new(&args[2]);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(destination, serde_json::to_vec_pretty(&report)?)?;
    println!(
        "Checked {} original maps; {} maps with errors",
        reports.len(),
        failed
    );
    if failed > 0 {
        return Err("Map validation failed; see report".into());
    }
    Ok(())
}
