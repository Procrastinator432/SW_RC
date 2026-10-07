use rc_package::{
    skeletal_mesh::StoredBone, skeletal_reference_cache::ensure_inverse_reference_cache,
    skeletal_reference_product::compose_reference_matrix,
};
use std::{env, fs};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-reference-cache-check ORIGINAL_LINKUPS OUTPUT".into());
    }
    let source: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let mut objects = vec![];
    let mut count = 0;
    for (source_index, o) in source["objects"]
        .as_array()
        .ok_or("objects")?
        .iter()
        .enumerate()
    {
        if o["status"] == "UnsupportedLegacyPackage" {
            continue;
        }
        let bones: Vec<StoredBone> = serde_json::from_value(o["prefix"]["bones"].clone())?;
        let mut cache = vec![];
        let mut flag = 7;
        let built = ensure_inverse_reference_cache(&bones, &mut cache, &mut flag);
        let after_build = flag;
        let snapshot = cache.clone();
        flag = 9;
        let reused = ensure_inverse_reference_cache(&bones, &mut cache, &mut flag);
        if cache != snapshot {
            return Err("nonempty cache changed".into());
        }
        count += cache.len();
        objects.push(serde_json::json!({"source_index":source_index,"built":built,"flag_after_build":after_build,"reused":reused,"flag_after_reuse":flag,"inverse":cache}));
    }
    let mut products = vec![];
    for seed in 0..64 {
        let a =
            std::array::from_fn::<_, 16, _>(|i| (((i * 13 + seed * 17) % 47) as f32 - 23.) * 0.125)
                .map(f32::to_bits);
        let b =
            std::array::from_fn::<_, 16, _>(|i| (((i * 7 + seed * 11) % 41) as f32 - 20.) * 0.2)
                .map(f32::to_bits);
        products
            .push(serde_json::json!({"local":a,"parent":b,"result":compose_reference_matrix(a,b)}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"scope":"Original reference-bone inverse cache build and native nonempty reuse gate on supplied mesh snapshots. Native reference-parent SSE order and general matrix inverse; no Move detachment, no skinning/render use or complete ApplyAnimation buffer/scene preparation.","meshes":objects.len(),"matrices":count,"objects":objects,"products":products}),
        )?,
    )?;
    println!(
        "{} reference caches / {count} inverse matrices",
        objects.len()
    );
    Ok(())
}
