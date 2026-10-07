use rc_package::{
    quaternion_animation::{PortableQuaternionMath, QuaternionMath},
    quaternion_matrix::quaternion_from_matrix,
};
use std::{env, fs};
struct FixedSeed;
impl QuaternionMath for FixedSeed {
    fn reciprocal_sqrt_seed(&mut self, _: f32) -> Result<f32, String> {
        Ok(0.125)
    }
    fn spherical_weights(&mut self, _: f32, _: f32) -> Result<[f32; 2], String> {
        Err("unused spherical boundary".into())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-matrix-quaternion-check WORLD_DIRECTOR_REPORT OUTPUT".into());
    }
    let input: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut cases = vec![];
    for (i, c) in input["cases"].as_array().ok_or("cases")?.iter().enumerate() {
        // One transform per source pose; includes inherited original bone rotations.
        if c["transform"] != 0 {
            continue;
        }
        let matrices: Vec<[u32; 16]> = serde_json::from_value(c["matrices"].clone())?;
        for (bone, m) in matrices.into_iter().enumerate() {
            let q = quaternion_from_matrix(m, &mut PortableQuaternionMath)?;
            cases.push(serde_json::json!({"source_case":i,"bone":bone,"quaternion":q}));
        }
    }
    let mut probes = vec![];
    for seed in 0..256 {
        let mut m = std::array::from_fn::<_, 16, _>(|i| {
            ((((seed * 17 + i * 11) % 47) as f32) - 23.) * 0.25
        });
        // Force negative trace and each strict/tie diagonal branch in addition to general matrices.
        match seed % 8 {
            0 => {
                m[0] = 1.;
                m[5] = 1.;
                m[10] = 1.;
            }
            1 => {
                m[0] = 1.;
                m[5] = -2.;
                m[10] = -3.;
            }
            2 => {
                m[0] = -2.;
                m[5] = 1.;
                m[10] = -3.;
            }
            3 => {
                m[0] = -3.;
                m[5] = -2.;
                m[10] = 1.;
            }
            4 => {
                m[0] = -1.;
                m[5] = -1.;
                m[10] = -1.;
            }
            5 => {
                m[0] = 0.;
                m[5] = 0.;
                m[10] = -2.;
            }
            _ => {}
        }
        let m = m.map(f32::to_bits);
        probes.push(
            serde_json::json!({"matrix":m,"quaternion":quaternion_from_matrix(m,&mut FixedSeed)?}),
        );
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"scope":"Native FQuat(FMatrix) helper on original-track-derived directed bone matrices. Portable RSQRT policy for source matrices, fixed seed 0.125 for 256 general/branch probes. No director rotation/history application or x86 seed/NaN-payload emulation.","matrices":cases.len(),"cases":cases,"probes":probes}),
        )?,
    )?;
    println!("{} source bone matrices, 256 branch probes", cases.len());
    Ok(())
}
