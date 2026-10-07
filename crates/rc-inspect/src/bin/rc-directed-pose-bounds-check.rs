use rc_package::{
    quaternion_animation::{PortableQuaternionMath, QuaternionMath},
    skeletal_bounds::{BoundsPadding, PoseBounds, PoseBoundsHost},
    skeletal_directed_bounds::{evaluate_directed_pose, DirectedPoseInput},
    skeletal_director::BoneDirector,
    skeletal_director_rotation::PortableDirectorRotationHost,
    skeletal_hierarchy::prepare_hierarchy,
    skeletal_mesh::StoredBone,
    skeletal_root_pose::RootTransform,
    skeletal_world_bounds::{publish_world_bounds, WorldPoseBounds},
};
use std::{env, fs};
struct Publication {
    matrix: Option<[u32; 16]>,
    world: WorldPoseBounds,
    snapshot: Option<PoseBounds>,
    math: PortableQuaternionMath,
}
impl PoseBoundsHost for Publication {
    fn reciprocal_sqrt_seed(&mut self, n: f32) -> Result<f32, String> {
        self.math.reciprocal_sqrt_seed(n)
    }
    fn publish(&mut self, b: &PoseBounds) -> Result<(), String> {
        self.snapshot = Some(b.clone());
        if let Some(m) = self.matrix {
            publish_world_bounds(b, m, &mut self.world, &mut self.math)?;
        }
        Ok(())
    }
}
fn fingerprint(matrices: &[[u32; 16]]) -> String {
    let mut h = 0xcbf29ce484222325u64;
    for m in matrices {
        for word in m {
            for b in word.to_le_bytes() {
                h = (h ^ u64::from(b)).wrapping_mul(0x100000001b3);
            }
        }
    }
    format!("{h:016x}")
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-directed-pose-bounds-check DIRECTED_ROTATION_REPORT OUTPUT".into());
    }
    let prior: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let original: serde_json::Value =
        serde_json::from_slice(&fs::read("analysis/reports/director-poses.json")?)?;
    let stacks: serde_json::Value =
        serde_json::from_slice(&fs::read("analysis/reports/original-channel-stacks.json")?)?;
    let links: serde_json::Value = serde_json::from_slice(&fs::read(
        "analysis/reports/original-skeletal-linkups.json",
    )?)?;
    let mesh: [u32; 16] = serde_json::from_value(prior["mesh_to_world"].clone())?;
    let scale: [u32; 3] = serde_json::from_value(prior["actor_scale"].clone())?;
    let transforms = [
        mesh,
        [
            0f32, -2., 0., 0., 3., 0., 0., 0., 0.5, 0.25, 1., 0., -7., 11., 5., 1.,
        ]
        .map(f32::to_bits),
        [0; 16],
    ];
    let padding = BoundsPadding {
        minimum: [0.25f32, 1., 2.].map(f32::to_bits),
        maximum: [2f32, 0.5, 1.].map(f32::to_bits),
        k_one: [1f32.to_bits(); 3],
    };
    let mut cases = vec![];
    let mut count = 0;
    for (index, c) in prior["cases"].as_array().ok_or("cases")?.iter().enumerate() {
        let si = c["source_case"].as_u64().ok_or("source")? as usize;
        let s = &original["cases"][si];
        let oi = s["source_object"].as_u64().ok_or("object")? as usize;
        let o = &stacks["objects"][oi];
        let link = o["source_index"].as_u64().ok_or("link")? as usize;
        let ri = s["run"].as_u64().ok_or("run")? as usize;
        let ti = s["tick"].as_u64().ok_or("tick")? as usize;
        let bones: Vec<StoredBone> =
            serde_json::from_value(links["objects"][link]["prefix"]["bones"].clone())?;
        let hierarchy = prepare_hierarchy(&bones)?;
        let local: Vec<RootTransform> =
            serde_json::from_value(o["runs"][ri]["ticks"][ti]["local"].clone())?;
        let mut directors: Vec<BoneDirector> = serde_json::from_value(c["before"].clone())?;
        let mut matrices = vec![];
        let mut bounds = PoseBounds {
            minimum: [99; 3],
            maximum: [100; 3],
            sphere: [123; 4],
            byte_60: 7,
            byte_61: 8,
            byte_179: 9,
        };
        let scenario = c["scenario"].as_u64().ok_or("scenario")? as usize;
        let mut publication = Publication {
            matrix: if scenario == 0 {
                None
            } else {
                Some(transforms[scenario - 1])
            },
            world: WorldPoseBounds {
                minimum: [77; 3],
                maximum: [88; 3],
                valid: 0,
                sphere: [456; 4],
            },
            snapshot: None,
            math: PortableQuaternionMath,
        };
        let mut host = PortableDirectorRotationHost {
            math: PortableQuaternionMath,
        };
        let result = evaluate_directed_pose(
            DirectedPoseInput {
                local: &local,
                hierarchy: &hierarchy,
                editor: false,
                mesh_to_world: mesh,
                actor_scale: scale,
                padding: &padding,
            },
            &mut directors,
            &mut matrices,
            &mut bounds,
            &mut host,
            &mut publication,
        );
        let expected: Vec<[u32; 16]> = serde_json::from_value(c["matrices"].clone())?;
        if matrices != expected
            || serde_json::to_value(&directors)? != c["after"]
            || result != Ok(1)
        {
            return Err(
                format!("integrated pose differs from previous directed case {index}").into(),
            );
        }
        count += matrices.len();
        cases.push(serde_json::json!({"source_case":index,"matrix_fnv1a64":fingerprint(&matrices),"matrices_and_directors_match_prior":true,"result":result,"local":bounds,"published":publication.snapshot,"world":publication.world}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"scope":"Prepared full directed hierarchy, per-bone local point collection, local sphere, optional world bounds and final flags. Original-track-derived local poses and supplied mutable director/scene snapshots; diagnostic padding. All matrices/directors checked against prior directed report inside CLI, independent matrix fingerprint and bounds checks in recorder. No cache/preparation, live runtime binding, mesh skinning or Android claim.","poses":cases.len(),"bone_matrices":count,"padding":{"minimum":padding.minimum,"maximum":padding.maximum,"k_one":padding.k_one},"world_transforms":transforms,"cases":cases}),
        )?,
    )?;
    println!(
        "{} completed directed poses / {count} matrices",
        cases.len()
    );
    Ok(())
}
