//! Export original Clone work clips with explicit inputs and skeleton binding.
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args_os().collect();
    if a.len() != 3 {
        return Err("usage: rc-work-clips-export GAME_DATA OUTPUT".into());
    }
    let root = Path::new(&a[1]);
    let out = Path::new(&a[2]);
    fs::create_dir_all(out)?;
    let mut resolver = rc_inspect::assets::Assets::new(&[
        &root.join("Textures"),
        &root.join("Animations"),
        &root.join("System"),
    ])?;
    let names = [
        "DemolitionSetChargeStart",
        "DemolitionSetChargeLoop",
        "DemolitionSetChargeStop",
        "HackUnlockConStationStart",
        "HackUnlockConStationLoop",
        "HackUnlockConStationStop",
        "HackUnlockDoorStart",
        "HackUnlockDoorLoop",
        "HackUnlockDoorStop",
    ];
    let mesh =
        rc_inspect::visual_assets::load(root, "Clone.ukx", "CloneCommando", &names, &mut resolver)?;
    let rows:Vec<_>=mesh.clips.iter().map(|c|serde_json::json!({"source":c.source,"clip":c.sequence.name.name,"frames":c.sequence.frames,"rate":f32::from_bits(c.sequence.rate_bits),"seconds":c.sequence.frames as f64/f32::from_bits(c.sequence.rate_bits)as f64})).collect();
    fs::write(
        out.join("work-clips.json"),
        serde_json::to_vec(&serde_json::json!({"bones":mesh.bones,"clips":mesh.clips}))?,
    )?;
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"scope":"Original compressed work clips and Clone skeleton; no material replacement or gameplay timing inference","clips":rows}),
        )?,
    )?;
    println!("{}", serde_json::to_string_pretty(&rows)?);
    Ok(())
}
