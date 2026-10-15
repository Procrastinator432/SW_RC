//! Capture the actual bunker transition and thermal emitter properties before binding.
use rc_package::{
    classes,
    properties::{self, Value},
    read_package,
};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-gunner-originals GAME_DATA OUTPUT".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let catalog = rc_inspect::defaults::load(root)?;
    let mut rows = vec![];
    for file in [
        "Maps/geo_01a.ctm",
        "Properties/cteffects.u",
        "Properties/physicalmaterials.u",
    ] {
        let bytes = fs::read(root.join(file))?;
        let pkg = read_package(&bytes)?;
        let mut components = std::collections::BTreeSet::new();
        if file.contains("cteffects") {
            for e in &pkg.exports {
                if e.class != 0
                    || !(e.name.to_lowercase().contains("therm")
                        || e.name == "Explosion_Bunker_Lrg")
                {
                    continue;
                }
                let props = classes::read(&pkg, &bytes, e)?.properties;
                if let Some(p) = props.values.iter().find(|p| p.name == "Emitters") {
                    let payload = pkg.payload(&bytes, e)?;
                    for (i, _) in properties::object_array(
                        &pkg,
                        &payload[p.payload_offset..p.payload_offset + p.bytes],
                    )? {
                        components.insert(i as usize);
                    }
                }
            }
        }
        for (i, e) in pkg.exports.iter().enumerate() {
            let name = pkg.object_path(i as i32 + 1)?;
            let props = if e.class == 0 {
                classes::read(&pkg, &bytes, e)?.properties
            } else {
                properties::read(&pkg, &bytes, e)?
            };
            let selected = if file.ends_with("ctm") {
                [850,1093,325,754].contains(&(i+1)) || name.contains("BlockingVolume") || props.values.iter().any(|p| matches!(&p.value,Value::Name(v) if v=="BunkerCollision" || v=="SmallBunkerExplosion"))
            } else {
                name.to_lowercase().contains("therm")
                    || name == "Explosion_Bunker_Lrg"
                    || components.contains(&(i + 1))
            };
            if !selected {
                continue;
            }
            let payload = pkg.payload(&bytes, e)?;
            let nested=props.values.iter().filter(|p|p.struct_name.is_some() && matches!(p.value,Value::Raw{..}) || p.name=="MaterialMap").map(|p|{
                let data=&payload[p.payload_offset..p.payload_offset+p.bytes];
                serde_json::json!({"property":p.name,"slot":p.array_index,"decoded":properties::tagged_struct(&pkg,data),"array":properties::struct_array(&pkg,data),"raw":data})
            }).collect::<Vec<_>>();
            let resolved = if file.ends_with("ctm") {
                ["Tag","StaticMesh","Location","bCollideActors","bBlockPlayers","bBlockZeroExtentTraces","bBlockNonZeroExtentTraces","Brush"].iter().map(|n|serde_json::json!({"property":n,"resolved":catalog.instance(&pkg.object_path(e.class).unwrap(),&props,n,0,file)})).collect::<Vec<_>>()
            } else {
                vec![]
            };
            rows.push(serde_json::json!({"file":file,"original":i+1,"name":name,"class":pkg.object_path(e.class)?,"properties":props,"nested":nested,"resolved":resolved}));
        }
    }
    fs::write(
        out.join("original-bunker-effects.json"),
        serde_json::to_vec_pretty(&rows)?,
    )?;
    println!("Captured {} original bunker/effect objects", rows.len());
    Ok(())
}
