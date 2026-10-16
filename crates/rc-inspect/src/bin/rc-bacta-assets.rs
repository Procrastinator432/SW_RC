use rc_package::{properties, read_package};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("usage: rc-bacta-assets GAME_DATA OUT".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let cat = rc_inspect::defaults::load(root)?;
    let bytes = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let map = read_package(&bytes)?;
    let props = properties::read(&map, &bytes, &map.exports[735])?;
    let class = map.object_path(map.exports[735].class)?;
    let mut defaults = serde_json::Map::new();
    for field in [
        "DispenserEffectClass",
        "PawnEffectClass",
        "AmbientSound",
        "Location",
        "Rotation",
        "DrawScale",
        "DrawScale3D",
        "PrePivot",
        "CollisionHeight",
    ] {
        defaults.insert(
            field.into(),
            serde_json::to_value(cat.instance(&class, &props, field, 0, "geo_01a"))?,
        );
    }
    let mut resolver = rc_inspect::assets::Assets::new(&[
        &root.join("Textures"),
        &root.join("Animations"),
        &root.join("System"),
        &root.join("Properties"),
    ])?;
    let mesh = rc_inspect::visual_assets::load(
        root,
        "bactadispensers.ukx",
        "BactaDispenserGEO",
        &["Load", "Holster"],
        &mut resolver,
    )?;
    let anim = fs::read(root.join("Animations/bactadispensers.ukx"))?;
    let package = read_package(&anim)?;
    let mut notifies = vec![];
    for clip in &mesh.clips {
        for notify in &clip.sequence.notifies {
            let e = &package.exports[notify.object_index as usize - 1];
            notifies.push(serde_json::json!({"clip":clip.sequence.name,"notify":notify,"class":package.object_path(e.class)?,"properties":properties::read(&package,&anim,e)?}));
        }
    }
    let mut effects = vec![];
    for field in ["DispenserEffectClass", "PawnEffectClass"] {
        if let rc_package::properties::Value::Object { index, path } =
            cat.instance(&class, &props, field, 0, "geo_01a")?.value
        {
            if index != 0 {
                let mut data = serde_json::Map::new();
                for name in ["Emitters", "AmbientSound", "LifeSpan"] {
                    data.insert(
                        name.into(),
                        serde_json::to_value(cat.resolve(&path, name, 0))?,
                    );
                }
                effects.push(serde_json::json!({"field":field,"class":path,"defaults":data}));
            }
        }
    }
    fs::write(out.join("bacta-mesh.json"), serde_json::to_vec(&mesh)?)?;
    fs::write(
        out.join("original-bindings.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"actor":736,"class":class,"defaults":defaults,"notifies":notifies,"effects":effects,"bones":mesh.bones.iter().map(|b|&b.name.name).collect::<Vec<_>>()}),
        )?,
    )?;
    Ok(())
}
