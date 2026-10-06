//! Audit original Model solid-space state, separately from visible triangles.
use rc_package::{collision, geometry, level, read_package};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("Usage: rc-bsp-check <Maps-directory> <report.json>".into());
    }
    let mut files: Vec<_> = fs::read_dir(&args[1])?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("ctm")))
        .collect();
    files.sort();
    let mut maps = Vec::new();
    let mut failed = 0;
    let mut total = 0;
    for file in files {
        let bytes = fs::read(&file)?;
        let pkg = read_package(&bytes)?;
        let binding = level::world_binding(&pkg, &bytes)?;
        let mut models = Vec::new();
        let mut errors = Vec::new();
        let mut probes = Vec::new();
        for (i, e) in pkg.exports.iter().enumerate() {
            if pkg.object_path(e.class)? != "Engine.Model" {
                continue;
            }
            let result = (|| -> Result<_, String> {
                let bsp = geometry::read_bsp(&pkg, &bytes, e)?;
                collision::read_model_collision(&pkg, &bytes, e, &bsp)
            })();
            match result {
                Ok(solid) => {
                    total += 1;
                    if i == binding.model_export {
                        for start in level::player_starts(&pkg, &bytes)? {
                            let outside = solid.point_outside(start.location)?;
                            let mut lines = Vec::new();
                            for (axis, direction) in [
                                ("+X", [1., 0., 0.]),
                                ("-X", [-1., 0., 0.]),
                                ("+Y", [0., 1., 0.]),
                                ("-Y", [0., -1., 0.]),
                                ("+Z", [0., 0., 1.]),
                                ("-Z", [0., 0., -1.]),
                            ] {
                                let end = std::array::from_fn(|j| {
                                    start.location[j] + direction[j] * 10000.
                                });
                                lines.push(serde_json::json!({"direction":axis,"length":10000,"clear":solid.line_clear(start.location,end)?}));
                            }
                            probes.push(serde_json::json!({"actor":start.actor,"location":start.location,"outside":outside,"lines":lines}));
                        }
                    }
                    models.push(serde_json::json!({"object":pkg.object_path(i as i32+1)?,"nodes":solid.nodes.len(),"root_outside":solid.root_outside,"linked":solid.linked,"zones":solid.zones,"bounds":solid.bounds,"hull_indices":solid.hull_indices,"leaves":solid.leaves,"lights":solid.lights,"decoded_bytes":solid.decoded_bytes,"remaining_bytes":solid.remaining_bytes}));
                }
                Err(error) => errors
                    .push(serde_json::json!({"object":pkg.object_path(i as i32+1)?,"error":error})),
            }
        }
        if !errors.is_empty() {
            failed += 1;
        }
        maps.push(
            serde_json::json!({"map":file,"world_binding":binding,"models":models,"start_probes":probes,"errors":errors}),
        );
    }
    if let Some(parent) = Path::new(&args[2]).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"maps":maps,"models":total,"failed_maps":failed,"scope":"World Model solid-space/zero-extent checks, no StaticMesh or pawn collision"}),
        )?,
    )?;
    println!("{total} Model tails; {failed} maps with errors");
    if failed > 0 {
        return Err("BSP collision audit failed; see report".into());
    }
    Ok(())
}
