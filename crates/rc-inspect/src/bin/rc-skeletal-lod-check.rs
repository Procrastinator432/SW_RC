use rc_package::{
    read_package,
    skeletal_lod::{inspect_skin_program, read_skeletal_lods},
    skeletal_skin::{build_skin_palette, skin_rigid_vertex},
};
use std::{env, fs};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 5 {
        return Err(
            "usage: rc-skeletal-lod-check LINKUPS FULL_TICKS REFERENCE_CACHE OUTPUT".into(),
        );
    }
    let links: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let poses: serde_json::Value = serde_json::from_slice(&fs::read(&args[2])?)?;
    let inverses: serde_json::Value = serde_json::from_slice(&fs::read(&args[3])?)?;
    let mut objects = vec![];
    let (mut lod_count, mut bind_count, mut outputs, mut rigid_count) = (0, 0, 0, 0);
    for (index, original) in links["objects"]
        .as_array()
        .ok_or("objects")?
        .iter()
        .enumerate()
    {
        if original["status"] == "UnsupportedLegacyPackage" {
            continue;
        }
        let bytes = fs::read(original["file"].as_str().ok_or("file")?)?;
        let package = read_package(&bytes)?;
        let export =
            &package.exports[original["export_index"].as_u64().ok_or("export")? as usize - 1];
        let mesh = read_skeletal_lods(&package, &bytes, export)
            .map_err(|e| format!("{}: {e}", original["object"]))?;
        let mut programs = vec![];
        for (li, lod) in mesh.lods.iter().enumerate() {
            lod_count += 1;
            bind_count += lod.bind_vertices.len();
            if lod.commands.is_empty() {
                if !lod.bind_vertices.is_empty() {
                    return Err("empty skin program with nonempty bind vertices".into());
                }
                programs.push(None);
                continue;
            }
            let program =
                inspect_skin_program(&lod.commands, lod.bind_vertices.len()).map_err(|e| {
                    format!(
                        "{} LOD {li} ({} commands, {} binds): {e}",
                        original["object"],
                        lod.commands.len(),
                        lod.bind_vertices.len()
                    )
                })?;
            if program.consumed_bind != lod.bind_vertices.len()
                || program.command_words != lod.commands.len()
            {
                return Err("LOD skin program has unused bind vertices or command words".into());
            }
            outputs += program.outputs;
            programs.push(Some(program));
        }
        let prior = poses["objects"]
            .as_array()
            .ok_or("pose objects")?
            .iter()
            .find(|o| o["source_index"] == index);
        let mut runs = vec![];
        if let Some(prior) = prior {
            let inverse = inverses["objects"]
                .as_array()
                .ok_or("inverse objects")?
                .iter()
                .find(|o| o["source_index"] == index)
                .ok_or("inverse")?;
            let inverse: Vec<[u32; 16]> = serde_json::from_value(inverse["inverse"].clone())?;
            for run in prior["runs"].as_array().ok_or("runs")? {
                let mut cases = vec![];
                for case in run["cases"].as_array().ok_or("cases")? {
                    let pose: Vec<[u32; 16]> =
                        serde_json::from_value(case["full"]["matrices"].clone())?;
                    let mut palette = vec![];
                    build_skin_palette(&pose, &inverse, &mut palette)?;
                    let mut samples = vec![];
                    for (lod_index, (lod, program)) in mesh.lods.iter().zip(&programs).enumerate() {
                        let Some(program) = program else { continue };
                        let n = program.rigid.len().min(8);
                        for j in 0..n {
                            let selection = if n < 2 {
                                0
                            } else {
                                j * (program.rigid.len() - 1) / (n - 1)
                            };
                            let entry = &program.rigid[selection];
                            let command = lod.commands[entry.command_index];
                            let vertex = lod.bind_vertices[entry.bind_index];
                            let output = skin_rigid_vertex(command, vertex, &palette)?;
                            samples.push(serde_json::json!({"lod":lod_index,"rigid_index":selection,"output":output}));
                            rigid_count += 1;
                        }
                    }
                    cases.push(serde_json::json!({"step":case["step"],"samples":samples}));
                }
                runs.push(serde_json::json!({"animation":run["animation"],"entry":run["entry"],"cases":cases}));
            }
        }
        objects.push(
            serde_json::json!({"source_index":index,"mesh":mesh,"programs":programs,"runs":runs}),
        );
    }
    fs::write(
        &args[4],
        serde_json::to_vec(&serde_json::json!({"objects":objects,
        "lods":lod_count,"bind_vertices":bind_count,"outputs":outputs,"rigid_samples":rigid_count,
        "scope":"Original persistent LOD model v1 archives, full UV/influence command boundaries, and sampled top-nibble-zero rigid vertices with real bind positions, packed normals and bone commands under prior verified full animation poses. Weighted/cache-copy execution, full mesh output, triangles/material rendering and Android verification remain open."}))?,
    )?;
    println!("{lod_count} LODs, {bind_count} bind vertices, {outputs} outputs, {rigid_count} original rigid samples");
    Ok(())
}
