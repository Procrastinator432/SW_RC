use rc_package::{mesh::read_static_mesh, read_package};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("Usage: rc-mesh-check <StaticMeshes-directory> <report.json>".into());
    }
    let mut files = fs::read_dir(&args[1])?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;
    files.sort();
    let mut results = Vec::new();
    let (mut meshes, mut triangles, mut failures) = (0, 0, 0);
    for path in files
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("usx")))
    {
        let data = fs::read(&path)?;
        let pkg = read_package(&data)?;
        for (i, e) in pkg.exports.iter().enumerate() {
            if pkg.object_path(e.class)? != "Engine.StaticMesh" {
                continue;
            }
            meshes += 1;
            match read_static_mesh(&pkg, &data, e) {
                Ok(m) => {
                    let count = m.triangles()?.len();
                    triangles += count;
                    results.push(serde_json::json!({"file":path,"object":pkg.object_path(i as i32+1)?,"vertices":m.vertices.len(),"triangles":count,"tail_bytes":m.unparsed_tail_bytes}));
                }
                Err(error) => {
                    failures += 1;
                    results.push(serde_json::json!({"file":path,"object":pkg.object_path(i as i32+1)?,"version":pkg.summary.version,"error":error}));
                }
            }
        }
    }
    let report = serde_json::json!({"meshes":meshes,"triangles":triangles,"failures":failures,"results":results});
    if let Some(parent) = Path::new(&args[2]).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args[2], serde_json::to_vec_pretty(&report)?)?;
    println!("{meshes} meshes, {triangles} triangles, {failures} failures");
    if failures != 0 {
        return Err("Mesh validation failed; see report".into());
    }
    Ok(())
}
