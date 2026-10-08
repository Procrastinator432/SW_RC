use rc_package::{
    read_package,
    skeletal_lod::{inspect_skin_program, read_skeletal_lods},
    skeletal_mesh::read_skeletal_mesh_prefix,
    skeletal_reference_product::compose_reference_matrix,
    skeletal_root_pose::{quaternion_translation_matrix, RootTransform},
    skeletal_skin::{build_skin_palette, skin_lod_stream, BindSkinVertex, SkinStreamVertex},
};
use std::{env, fs};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 5 {
        return Err("usage: rc-skin-stream-check LINKUPS FULL_TICKS REFERENCE_CACHE OUTPUT".into());
    }
    let links: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let poses: serde_json::Value = serde_json::from_slice(&fs::read(&args[2])?)?;
    let inverses: serde_json::Value = serde_json::from_slice(&fs::read(&args[3])?)?;
    let mut objects = vec![];
    let (mut streams, mut vertices) = (0, 0);
    for prior in poses["objects"].as_array().ok_or("objects")? {
        let index = prior["source_index"].as_u64().ok_or("source index")? as usize;
        let original = &links["objects"][index];
        let data = fs::read(original["file"].as_str().ok_or("file")?)?;
        let pkg = read_package(&data)?;
        let export = &pkg.exports[original["export_index"].as_u64().ok_or("export")? as usize - 1];
        let mesh = read_skeletal_lods(&pkg, &data, export)?;
        let inverse = inverses["objects"]
            .as_array()
            .ok_or("inverse objects")?
            .iter()
            .find(|p| p["source_index"] == index)
            .ok_or("inverse cache")?;
        let inverse: Vec<[u32; 16]> = serde_json::from_value(inverse["inverse"].clone())?;
        let run = &prior["runs"][0];
        let case = &run["cases"][0];
        let has_animation = !prior["runs"].as_array().ok_or("runs")?.is_empty();
        let matrices: Vec<[u32; 16]> = if has_animation {
            serde_json::from_value(case["full"]["matrices"].clone())?
        } else {
            let prefix = read_skeletal_mesh_prefix(&pkg, &data, export)?;
            let mut reference = vec![];
            for (i, bone) in prefix.bones.iter().enumerate() {
                let mut matrix = quaternion_translation_matrix(RootTransform {
                    rotation: bone.rotation,
                    position: bone.position,
                });
                if i > 0 {
                    matrix = compose_reference_matrix(matrix, reference[bone.word_34 as usize]);
                }
                reference.push(matrix);
            }
            reference
        };
        let mut palette = vec![];
        build_skin_palette(&matrices, &inverse, &mut palette)?;
        let mut lods = vec![];
        for (li, lod) in mesh.lods.iter().enumerate() {
            if lod.commands.is_empty() {
                lods.push(serde_json::Value::Null);
                continue;
            }
            let expected = inspect_skin_program(&lod.commands, lod.bind_vertices.len())?;
            let mut output = vec![SkinStreamVertex::default(); expected.outputs];
            let mut cache = vec![0xdeadbeef]; // reset must not retain prior-frame cache data
            let result = skin_lod_stream(
                &lod.commands,
                &lod.bind_vertices,
                &palette,
                &mut output,
                &mut cache,
            )
            .map_err(|e| format!("{} LOD {li}: {e}", original["object"]))?;
            if result.written != expected.outputs
                || result.consumed_bind != lod.bind_vertices.len()
                || result.command_words != lod.commands.len()
                || result.kinds != expected.kinds
            {
                return Err("skin stream consumption differs from inspected archive".into());
            }
            streams += 1;
            vertices += result.written;
            lods.push(serde_json::json!({"result":result,"output":output,"cache":cache}));
        }
        objects.push(serde_json::json!({"source_index":index,"pose_source":if has_animation {"AnimatedFullPose"} else {"OriginalReferencePose"},"animation":run["animation"],"entry":run["entry"],"step":case["step"],"lods":lods}));
    }
    let mut synthetic = vec![];
    let mut seed = 0x17a39bcdu32;
    let mut random = || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        seed
    };
    for _ in 0..64 {
        let palette: Vec<[u32; 16]> = (0..3)
            .map(|_| {
                std::array::from_fn(|_| ((random() >> 8) as f32 / 16777216.0 * 8.0 - 4.0).to_bits())
            })
            .collect();
        let mut input = vec![];
        let mut commands = vec![];
        let mut cache_count = 0;
        for kind in 0..15u32 {
            input.push(BindSkinVertex {
                position: std::array::from_fn(|_| {
                    ((random() >> 8) as f32 / 16777216.0 * 32.0 - 16.0).to_bits()
                }),
                packed_normal: random(),
            });
            for j in 0..((kind & 7) + 1) {
                commands.push(
                    (if j == 0 { kind << 28 } else { 0 })
                        | (random() & 0x0ffff000)
                        | (((kind + j) % 3) * 6),
                );
            }
            commands.extend([random(), random()]);
            if kind & 8 != 0 {
                // Reuse each cached result immediately with different UV bits.
                commands.extend([0xf0000000 + cache_count * 6 + 1, random(), random()]);
                cache_count += 1;
            }
        }
        commands.push(u32::MAX);
        let expected = inspect_skin_program(&commands, input.len())?;
        let mut output = vec![SkinStreamVertex::default(); expected.outputs];
        let mut cache = vec![];
        let result = skin_lod_stream(&commands, &input, &palette, &mut output, &mut cache)?;
        synthetic.push(serde_json::json!({"palette":palette,"input":input,"commands":commands,"output":output,"cache":cache,"result":result}));
    }
    fs::write(
        &args[4],
        serde_json::to_vec(
            &serde_json::json!({"objects":objects,"synthetic":synthetic,"streams":streams,"vertices":vertices,
        "scope":"All nonempty original persistent LOD skin streams under one verified animated full pose per mesh where available (125 meshes), otherwise original reference pose (5 meshes), including weighted/cache/copy/UV output; synthetic streams cover every influence count 1..8. Native allocation/profiling/instance post-copy and triangle/material rendering remain external. No Android verification."}),
        )?,
    )?;
    println!("{streams} original streams, {vertices} original output vertices, 64 synthetic all-kind streams");
    Ok(())
}
