use rc_package::skeletal_bounds::*;
use std::{env, fs};
#[derive(Default)]
struct Host {
    squared: Vec<u32>,
    published: Vec<PoseBounds>,
    supplied_seed: bool,
}
impl PoseBoundsHost for Host {
    fn reciprocal_sqrt_seed(&mut self, n: f32) -> Result<f32, String> {
        self.squared.push(n.to_bits());
        Ok(if self.supplied_seed {
            0.125
        } else {
            1. / n.sqrt()
        })
    }
    fn publish(&mut self, b: &PoseBounds) -> Result<(), String> {
        self.published.push(b.clone());
        Ok(())
    }
}
fn initial() -> PoseBounds {
    PoseBounds {
        minimum: [(-3f32).to_bits(); 3],
        maximum: [5f32.to_bits(); 3],
        sphere: [77; 4],
        byte_60: 7,
        byte_61: 0,
        byte_179: 9,
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-pose-bounds-check STACK_REPORT OUTPUT".into());
    }
    let source: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let links: serde_json::Value = serde_json::from_slice(&fs::read(
        "analysis/reports/original-skeletal-linkups.json",
    )?)?;
    let padding = BoundsPadding {
        minimum: [0.25f32, 1., 2.].map(f32::to_bits),
        maximum: [2f32, 0.5, 1.].map(f32::to_bits),
        k_one: [1f32.to_bits(); 3],
    };
    let mut cases = vec![];
    for (oi, o) in source["objects"]
        .as_array()
        .ok_or("objects")?
        .iter()
        .enumerate()
    {
        let si = o["source_index"].as_u64().ok_or("source index")? as usize;
        let name = links["objects"][si]["prefix"]["bones"][0]["name"]["name"]
            .as_str()
            .ok_or("root name")?;
        let move_bone = if name.eq_ignore_ascii_case("Move") {
            0
        } else {
            -1
        };
        for (ri, run) in o["runs"].as_array().ok_or("runs")?.iter().enumerate() {
            let mut state = initial();
            for (ti, tick) in run["ticks"].as_array().ok_or("ticks")?.iter().enumerate() {
                // Caller-driven invalidation for these diagnostics, not a reconstructed tick rule.
                state.byte_61 = 0;
                let before = state.clone();
                let matrices: Vec<[u32; 16]> = serde_json::from_value(tick["matrices"].clone())?;
                let mut host = Host::default();
                let result =
                    finish_prepared_bounds(&mut state, &matrices, move_bone, &padding, &mut host);
                cases.push(serde_json::json!({"source_object":oi,"run":ri,"tick":ti,"move_bone":move_bone,"before":before,"after":state,"result":result,"squared":host.squared,"published":host.published}));
            }
        }
    }
    let mut synthetic = vec![];
    for seed in 0..64 {
        let move_bone = if seed % 2 == 0 { 0 } else { -1 };
        let mut matrices = vec![];
        for bone in 0..seed % 5 {
            let mut mat = [0; 16];
            for axis in 0..3 {
                mat[12 + axis] = (((seed * 13 + bone * 7 + axis * 3) % 37) as f32 - 18.).to_bits();
            }
            matrices.push(mat);
        }
        let before = initial();
        let mut after = before.clone();
        let mut host = Host {
            supplied_seed: true,
            ..Host::default()
        };
        let result = finish_prepared_bounds(&mut after, &matrices, move_bone, &padding, &mut host);
        synthetic.push(serde_json::json!({"move_bone":move_bone,"matrices":matrices,"before":before,"after":after,"result":result,"squared":host.squared,"published":host.published}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"scope":"Local point bounds and sphere from 900 original-track stack matrices, with supplied asymmetric diagnostic mesh padding and supplied kOne=(1,1,1). Actor +48 absent: publication is a no-op observer, not world-space bounds emulation. Portable square-root seed for original matrices; fixed .125 seed for 64 synthetic cases. Directors absent, per-tick cache invalidation supplied by caller.","padding":{"minimum":padding.minimum,"maximum":padding.maximum,"k_one":padding.k_one},"cases":cases,"synthetic":synthetic}),
        )?,
    )?;
    println!(
        "{} original-matrix bounds, {} synthetic cases",
        cases.len(),
        synthetic.len()
    );
    Ok(())
}
