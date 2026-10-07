use rc_package::{
    skeletal_director::*, skeletal_hierarchy::prepare_hierarchy,
    skeletal_matrix_inverse::inverse_matrix, skeletal_mesh::StoredBone,
    skeletal_root_pose::RootTransform,
};
use std::{env, fs};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-world-director-check LOCAL_DIRECTOR_REPORT OUTPUT".into());
    }
    let report: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let stacks: serde_json::Value =
        serde_json::from_slice(&fs::read("analysis/reports/original-channel-stacks.json")?)?;
    let links: serde_json::Value = serde_json::from_slice(&fs::read(
        "analysis/reports/original-skeletal-linkups.json",
    )?)?;
    let transforms = [
        [
            2f32, 0., 0., 0., 0., 4., 0., 0., 0., 0., 0.5, 0., 10., 20., 30., 1.,
        ],
        [
            0., -2., 0., 0., 3., 0., 0., 0., 0.5, 0.25, 1., 0., -7., 11., 5., 1.,
        ],
        [0.; 16],
    ]
    .map(|m| m.map(f32::to_bits));
    let mut cases = vec![];
    let mut count = 0;
    for (si, c) in report["cases"]
        .as_array()
        .ok_or("cases")?
        .iter()
        .enumerate()
    {
        if c["editor"].as_bool().ok_or("editor")? {
            continue;
        }
        let oi = c["source_object"].as_u64().ok_or("object")? as usize;
        let ri = c["run"].as_u64().ok_or("run")? as usize;
        let ti = c["tick"].as_u64().ok_or("tick")? as usize;
        let o = &stacks["objects"][oi];
        let index = o["source_index"].as_u64().ok_or("index")? as usize;
        let bones: Vec<StoredBone> =
            serde_json::from_value(links["objects"][index]["prefix"]["bones"].clone())?;
        let hierarchy = prepare_hierarchy(&bones)?;
        let local: Vec<RootTransform> =
            serde_json::from_value(o["runs"][ri]["ticks"][ti]["local"].clone())?;
        let mut directors: Vec<BoneDirector> = serde_json::from_value(c["directors"].clone())?;
        directors[1].words[19] = 0;
        for (i, &transform) in transforms.iter().enumerate() {
            let mut output = vec![];
            let result = build_directed_pose_with_transform(
                &local,
                &hierarchy,
                false,
                &directors,
                transform,
                &mut output,
            );
            count += output.len();
            cases.push(serde_json::json!({"source_case":si,"transform":i,"result":result,"matrices":output}));
        }
    }
    let mut inverse_cases = vec![];
    for seed in 0..128 {
        let matrix = std::array::from_fn::<_, 16, _>(|i| {
            let v = (((i * 7 + seed * 11) % 17) as f32 - 8.) * 0.03125;
            if i / 4 == i % 4 {
                v + 2.
            } else {
                v
            }
        })
        .map(f32::to_bits);
        inverse_cases.push(serde_json::json!({"matrix":matrix,"inverse":inverse_matrix(matrix)}));
    }
    let mut conversion_cases = vec![];
    for seed in 1..=32 {
        let d =
            std::array::from_fn::<_, 16, _>(|i| (((i * 17 + seed * 11) % 43) as f32 - 21.) * 0.125)
                .map(f32::to_bits);
        let inv =
            std::array::from_fn::<_, 16, _>(|i| (((i * 13 + seed * 7) % 37) as f32 - 18.) * 0.2)
                .map(f32::to_bits);
        conversion_cases.push(
            serde_json::json!({"director":d,"inverse":inv,"converted":director_to_local(d,inv)}),
        );
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"scope":"World-to-local translation/plane-scale directors on original-track-derived game poses; supplied director snapshots and three supplied MeshToWorld matrices (two invertible affine, one singular identity fallback). No original runtime binding or rotation/ancestor correction. Native scalar inverse and reverse-order conversion, finite arithmetic validation without NaN payload/control-register emulation.","transforms":transforms,"inverses":transforms.map(inverse_matrix),"poses":cases.len(),"bone_matrices":count,"cases":cases,"inverse_cases":inverse_cases,"conversion_cases":conversion_cases}),
        )?,
    )?;
    println!(
        "{} world-directed poses, {count} matrices, 128 inverse probes",
        cases.len()
    );
    Ok(())
}
