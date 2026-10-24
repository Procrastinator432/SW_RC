//! Original DC15 BringUp/PlaySelect clip and notify evidence.
use rc_package::{
    properties::{self, Value},
    read_package,
};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args().collect();
    if a.len() != 3 {
        return Err("usage: rc-weapon-raise-probe GAME_DATA OUT.json".into());
    }
    let root = Path::new(&a[1]);
    let cat = rc_inspect::defaults::load(root)?;
    let mut fields = vec![];
    for field in ["LoadAnim", "WeaponType", "WeaponIndex"] {
        fields.push(
            serde_json::json!({"field":field,"resolved":cat.resolve("CTInventory.DC15s",field,0)?}),
        );
    }
    let Value::Name(name) = cat.resolve("CTInventory.DC15s", "LoadAnim", 0)?.value else {
        return Err("LoadAnim name".into());
    };
    let mut assets =
        rc_inspect::assets::Assets::new(&[&root.join("Textures"), &root.join("System")])?;
    let mesh = rc_inspect::visual_assets::load_from_animation(
        root,
        "hudarms.ukx",
        "HudArmsDC15s",
        &[&name],
        &mut assets,
        Some("hudarms.HudArmsDC15sSet"),
    )?;
    let clip = &mesh.clips[0];
    let raw = fs::read(root.join("Animations/hudarms.ukx"))?;
    let pkg = read_package(&raw)?;
    let mut notifies = vec![];
    for n in &clip.sequence.notifies {
        let e = &pkg.exports[n.object_index as usize - 1];
        notifies.push(serde_json::json!({"original":n.object_index,"time":f32::from_bits(n.time_bits),"class":pkg.object_path(e.class)?,"properties":properties::read(&pkg,&raw,e)?}));
    }
    fs::write(
        &a[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"fields":fields,"source":clip.source,"frames":clip.sequence.frames,"rate":f32::from_bits(clip.sequence.rate_bits),"notifies":notifies}),
        )?,
    )?;
    println!(
        "Load clip {name}: {} frames, {} notifies",
        clip.sequence.frames,
        notifies.len()
    );
    Ok(())
}
