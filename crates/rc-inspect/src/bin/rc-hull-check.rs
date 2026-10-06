//! Audit original BSP collision hulls and diagnostic player-sized world sweeps.
use rc_package::{collision, geometry, hull, level, properties::Value, read_package};
use std::{env, fs, path::Path};
#[path = "../defaults.rs"]
mod defaults;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("Usage: rc-hull-check <GameData> <report.json>".into());
    }
    let root = Path::new(&args[1]);
    let catalog = defaults::load(root)?;
    let radius = catalog.resolve("CTCharacters.PlayerCommando", "CollisionRadius", 0)?;
    let height = catalog.resolve("CTCharacters.PlayerCommando", "CollisionHeight", 0)?;
    let scalar = |v: &Value| match v {
        Value::Float(v) if v.is_finite() && *v > 0.0 => Ok(f64::from(*v)),
        _ => Err("invalid player size"),
    };
    let extent = [
        scalar(&radius.value)?,
        scalar(&radius.value)?,
        scalar(&height.value)?,
    ];
    let mut files: Vec<_> = fs::read_dir(root.join("Maps"))?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<_, _>>()?;
    files.sort();
    let (mut models, mut failures, mut hulls, mut missing, mut blocked, mut free, mut query_errors) =
        (0, 0, 0, 0, 0, 0, 0);
    let mut maps = Vec::new();
    for file in files {
        if !file
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("ctm"))
        {
            continue;
        }
        let data = fs::read(&file)?;
        let pkg = read_package(&data)?;
        let starts =
            level::player_starts_with_defaults(&pkg, &data, &catalog, &file.to_string_lossy())?;
        let binding = level::world_binding(&pkg, &data)?;
        let mut records = Vec::new();
        let mut probes = Vec::new();
        for (i, export) in pkg.exports.iter().enumerate() {
            if pkg.object_path(export.class)? != "Engine.Model" {
                continue;
            }
            models += 1;
            let result = (|| -> Result<_, String> {
                let bsp = geometry::read_bsp(&pkg, &data, export)?;
                let solid = collision::read_model_collision(&pkg, &data, export, &bsp)?;
                hull::HullSet::read(&bsp, &solid)
            })();
            match result {
                Ok(set) => {
                    hulls += set.hulls.len();
                    missing += set.missing_solid_leaves;
                    records.push(serde_json::json!({"object":pkg.object_path(i as i32+1)?,"hulls":set.hulls.len(),"hull_planes":set.hulls.iter().map(|h|h.planes.len()).sum::<usize>(),"missing_solid_leaves":set.missing_solid_leaves,"empty_root_solid":set.empty_root_solid}));
                    if i == binding.model_export {
                        for start in &starts {
                            for axis in 0..3 {
                                for direction in [-1.0, 1.0] {
                                    let a = start.location.map(f64::from);
                                    let mut b = a;
                                    b[axis] += direction * 1000.0;
                                    let query = match set.sweep(a, b, extent) {
                                        Ok(hit) => {
                                            if hit.is_some() {
                                                blocked += 1;
                                            } else {
                                                free += 1;
                                            }
                                            serde_json::json!({"hit":hit,"zero_extent_hit":set.sweep(a,b,[0.0;3])?})
                                        }
                                        Err(error) => {
                                            query_errors += 1;
                                            serde_json::json!({"error":error})
                                        }
                                    };
                                    probes.push(serde_json::json!({"start":start.actor,"axis":axis,"direction":direction,"result":query}));
                                }
                            }
                        }
                    }
                }
                Err(error) => {
                    failures += 1;
                    records.push(
                        serde_json::json!({"object":pkg.object_path(i as i32+1)?,"error":error}),
                    );
                }
            }
        }
        maps.push(serde_json::json!({"map":file,"resolved_start_anchors":starts.len(),"world_binding":binding,"models":records,"probes":probes}));
    }
    if let Some(parent) = Path::new(&args[2]).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"models":models,"hulls":hulls,"failures":failures,"missing_solid_leaves":missing,"blocked_sweeps":blocked,"free_sweeps":free,"query_errors":query_errors,"player_radius":radius,"player_height":height,"extent":extent,"maps":maps,"scope":"Original hull records; mathematical AABB sweeps against Level-referenced world Model solid-leaf hulls at PlayerStart centers. No meshes, spawn selection, native tolerances/backoff/flags parity, movement or Android integration"}),
        )?,
    )?;
    println!("{models} Models, {hulls} solid-leaf hulls, {failures} hull errors; {blocked} blocked/{free} free sweeps, {query_errors} query errors");
    if failures > 0 || query_errors > 0 {
        return Err("BSP hull audit incomplete; see report".into());
    }
    Ok(())
}
