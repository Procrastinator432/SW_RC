use rc_package::{
    read_package,
    skeletal_mesh::read_skeletal_mesh_prefix,
    skeletal_skin::{build_skin_palette, skin_rigid_stream, BindSkinVertex, SkinVertex},
};
use std::{env, fs};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 5 {
        return Err("usage: rc-skin-rigid-check LINKUPS FULL_TICKS REFERENCE_CACHE OUTPUT".into());
    }
    let links: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let poses: serde_json::Value = serde_json::from_slice(&fs::read(&args[2])?)?;
    let inverses: serde_json::Value = serde_json::from_slice(&fs::read(&args[3])?)?;
    let mut objects = vec![];
    let mut matrices = 0;
    let mut vertices = 0;
    let mut points_count = 0;
    for o in poses["objects"].as_array().ok_or("objects")? {
        let index = o["source_index"].as_u64().ok_or("index")? as usize;
        let original = &links["objects"][index];
        let bytes = fs::read(original["file"].as_str().ok_or("file")?)?;
        let package = read_package(&bytes)?;
        let export =
            &package.exports[original["export_index"].as_u64().ok_or("export")? as usize - 1];
        let prefix = read_skeletal_mesh_prefix(&package, &bytes, export)?;
        let inverse = inverses["objects"]
            .as_array()
            .ok_or("inverse objects")?
            .iter()
            .find(|p| p["source_index"] == o["source_index"])
            .ok_or("inverse cache")?;
        let inverse: Vec<[u32; 16]> = serde_json::from_value(inverse["inverse"].clone())?;
        let mut runs = vec![];
        for run in o["runs"].as_array().ok_or("runs")? {
            let mut cases = vec![];
            let mut palette = vec![];
            for case in run["cases"].as_array().ok_or("cases")? {
                let pose: Vec<[u32; 16]> =
                    serde_json::from_value(case["full"]["matrices"].clone())?;
                build_skin_palette(&pose, &inverse, &mut palette)?;
                let input_source = if prefix.points.is_empty() {
                    "ReferenceBonePosition"
                } else {
                    "MeshPoint"
                };
                let source_points: Vec<_> = if prefix.points.is_empty() {
                    prefix.bones.iter().map(|b| b.position).collect()
                } else {
                    prefix.points.clone()
                };
                let count = source_points.len().min(8);
                let mut commands = vec![];
                let mut inputs = vec![];
                let mut indices = vec![];
                for j in 0..count {
                    let point_index = if count <= 1 {
                        0
                    } else {
                        j * (source_points.len() - 1) / (count - 1)
                    };
                    let bone = point_index % palette.len();
                    if bone * 6 > 0xfff {
                        return Err("diagnostic bone does not fit skin command".into());
                    }
                    commands.push((bone * 6) as u32);
                    indices.push(point_index);
                    inputs.push(BindSkinVertex {
                        position: source_points[point_index],
                        packed_normal: [0, 0x3fffffff, 0x1ff7fdff, 0x20000000][j % 4],
                    });
                }
                commands.push(u32::MAX);
                let mut output = vec![SkinVertex::default(); count];
                let written = skin_rigid_stream(&commands, &inputs, &palette, &mut output)?;
                matrices += palette.len();
                vertices += written;
                cases.push(serde_json::json!({"step":case["step"],"palette":palette,"input_source":input_source,"source_indices":indices,"commands":commands,"input":inputs,"output":output,"written":written}));
            }
            runs.push(serde_json::json!({"animation":run["animation"],"entry":run["entry"],"cases":cases}));
        }
        points_count += prefix.points.len();
        objects.push(serde_json::json!({"source_index":index,"points_offset":prefix.points_offset,"points":prefix.points,"bones_offset":prefix.bones_offset,"runs":runs}));
    }
    let mut synthetic = vec![];
    let mut seed = 0x13579bdu32;
    let mut random = || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        ((seed >> 8) as f32 / 16777216.0 * 8.0 - 4.0).to_bits()
    };
    for _ in 0..64 {
        let inverse = std::array::from_fn(|_| random());
        let pose = std::array::from_fn(|_| random());
        let vertex = BindSkinVertex {
            position: std::array::from_fn(|_| random()),
            packed_normal: random(),
        };
        let mut palette = vec![];
        build_skin_palette(&[pose], &[inverse], &mut palette)?;
        let mut output = [SkinVertex::default()];
        skin_rigid_stream(&[0, u32::MAX], &[vertex], &palette, &mut output)?;
        synthetic.push(serde_json::json!({"inverse":inverse,"pose":pose,"vertex":vertex,"palette":palette[0],"output":output[0]}));
    }
    fs::write(
        &args[4],
        serde_json::to_vec_pretty(
            &serde_json::json!({"objects":objects,"synthetic":synthetic,"points":points_count,"palette_matrices":matrices,"rigid_vertices":vertices,
        "scope":"Original legacy mesh point field and verified continuous full-pose matrices/inverse references; native transposed skin palette and top-nibble-zero rigid kernel. Empty legacy point arrays use original reference bone positions as explicitly diagnostic kernel inputs, with diagnostic bone assignments and packed normals. No native LOD vertex/influence/command/normal streams, weighted skinning, triangles, rendering or Android claim."}),
        )?,
    )?;
    println!("{points_count} original points, {matrices} palette matrices, {vertices} diagnostic rigid vertices");
    Ok(())
}
