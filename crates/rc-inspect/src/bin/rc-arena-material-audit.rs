//! Inspect original arena layered materials before extending the playable area.
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-arena-material-audit GAME_DATA SCENE_INVENTORY OUTPUT".into());
    }
    let root = Path::new(&args[1]);
    let inventory: serde_json::Value = serde_json::from_slice(&fs::read(&args[2])?)?;
    let mut assets =
        rc_inspect::assets::Assets::new(&[&root.join("Textures"), &root.join("System")])?;
    let mut rows = vec![];
    for row in inventory["materials"]
        .as_array()
        .ok_or("No material inventory")?
    {
        let Some(path) = row["material"].as_str() else {
            continue;
        };
        if !path.starts_with("Arena_Textures.") {
            continue;
        }
        let (class, properties) = assets.material_properties(path)?;
        if class != "Engine.Combiner" {
            continue;
        }
        let mut inputs = vec![];
        for name in ["Material1", "Material2", "Mask"] {
            if let Some(rc_package::properties::Value::Object { index, path: input }) = properties
                .values
                .iter()
                .find(|p| p.name == name)
                .map(|p| &p.value)
            {
                if *index == 0 {
                    continue;
                }
                let input = assets.qualify(path.split('.').next().unwrap(), input);
                let (input_class, input_properties) = assets.material_properties(&input)?;
                inputs.push(serde_json::json!({"field":name,"path":input,"class":input_class,"properties":input_properties}));
            }
        }
        rows.push(serde_json::json!({"material":path,"class":class,"properties":properties,"inputs":inputs,"previous_diffuse":row["texture"]}));
    }
    fs::write(&args[3], serde_json::to_vec_pretty(&rows)?)?;
    println!("Audited {} original arena combiner materials", rows.len());
    Ok(())
}
