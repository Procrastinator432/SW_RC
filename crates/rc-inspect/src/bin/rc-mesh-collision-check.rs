use rc_package::{collision, geometry, mesh, mesh_collision, read_package};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("Usage: rc-mesh-collision-check <StaticMeshes-directory> <report.json>".into());
    }
    let mut files: Vec<_> = fs::read_dir(&args[1])?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<_, _>>()?;
    files.sort();
    let mut results = Vec::new();
    let (
        mut meshes,
        mut triangles,
        mut nodes,
        mut simple_models,
        mut simple_model_failures,
        mut failures,
    ) = (0, 0, 0, 0, 0, 0);
    for file in files {
        if !file
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("usx"))
        {
            continue;
        }
        let data = fs::read(&file)?;
        let pkg = read_package(&data)?;
        for (i, export) in pkg.exports.iter().enumerate() {
            if pkg.object_path(export.class)? != "Engine.StaticMesh" {
                continue;
            }
            meshes += 1;
            let result = (|| -> Result<_, String> {
                let mesh = mesh::read_static_mesh(&pkg, &data, export)?;
                let collision = mesh_collision::read(&pkg, &data, export, &mesh)?;
                let simple = if collision.simple_model > 0 {
                    let parsed = (|| -> Result<_, String> {
                        let model = &pkg.exports[collision.simple_model as usize - 1];
                        let bsp = geometry::read_bsp(&pkg, &data, model)?;
                        let solid = collision::read_model_collision(&pkg, &data, model, &bsp)?;
                        Ok(
                            serde_json::json!({"nodes":solid.nodes.len(),"triangles":bsp.triangles()?.len(),"root_outside":solid.root_outside,"decoded_bytes":solid.decoded_bytes}),
                        )
                    })();
                    Some(match parsed {
                        Ok(value) => value,
                        Err(error) => {
                            serde_json::json!({"error":error,"version":pkg.summary.version})
                        }
                    })
                } else {
                    None
                };
                Ok((mesh, collision, simple))
            })();
            match result {
                Ok((mesh, collision, simple)) => {
                    triangles += collision.triangles.len();
                    nodes += collision.nodes.len();
                    simple_models += usize::from(simple.is_some());
                    simple_model_failures +=
                        usize::from(simple.as_ref().is_some_and(|v| v.get("error").is_some()));
                    results.push(serde_json::json!({"file":file,"object":pkg.object_path(i as i32+1)?,"render_triangles":mesh.triangles()?.len(),"collision_triangles":collision.triangles.len(),"collision_nodes":collision.nodes.len(),"simple_model":collision.simple_model_path,"simple_model_data":simple,"bounds_scale":collision.bounds_scale,"serialized_shadow_only":collision.serialized_shadow_only,"use_simple_line":collision.use_simple_line,"use_simple_box":collision.use_simple_box,"use_simple_karma":collision.use_simple_karma,"decoded_bytes":collision.decoded_bytes,"remaining_bytes":collision.remaining_bytes,"material_max":collision.triangles.iter().map(|t|t.material).max(),"materials":mesh.materials.len(),"root_bounds":if collision.nodes.is_empty(){None}else{Some(collision.expanded_bounds(0)?)} }));
                }
                Err(error) => {
                    failures += 1;
                    results.push(serde_json::json!({"file":file,"object":pkg.object_path(i as i32+1)?,"error":error}));
                }
            }
        }
    }
    if let Some(parent) = Path::new(&args[2]).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"meshes":meshes,"collision_triangles":triangles,"collision_nodes":nodes,"simple_models":simple_models,"simple_model_failures":simple_model_failures,"failures":failures,"results":results,"scope":"Original mesh collision prefix and local Model BSP structure; no UStaticMesh LineCheck dispatch, sweeps or player physics"}),
        )?,
    )?;
    println!("{meshes} meshes; {triangles} collision triangles; {nodes} nodes; {simple_models} simple Models ({simple_model_failures} unreadable); {failures} mesh failures");
    if failures != 0 {
        return Err("Mesh collision audit rejected data; see report".into());
    }
    Ok(())
}
