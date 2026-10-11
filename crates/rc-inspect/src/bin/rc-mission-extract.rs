use rc_package::{properties, read_package};
use std::{env, fs};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-mission-extract MAP OUTPUT_JSON".into());
    }
    let data = fs::read(&args[1])?;
    let pkg = read_package(&data)?;
    let mut objects = Vec::new();
    for (i, e) in pkg.exports.iter().enumerate().filter(|(_, e)| e.class != 0) {
        let class = pkg.object_path(e.class)?;
        let props = properties::read(&pkg, &data, e)?;
        let mut arrays = serde_json::Map::new();
        for p in &props.values {
            let payload = pkg.payload(&data, e)?;
            let bytes = payload
                .get(p.payload_offset..p.payload_offset + p.bytes)
                .ok_or("Property range")?;
            if p.name == "Actions" {
                arrays.insert(
                    p.name.clone(),
                    serde_json::to_value(properties::object_array(&pkg, bytes)?)?,
                );
            } else if p.name == "MissionObj" {
                arrays.insert(
                    p.name.clone(),
                    serde_json::to_value(properties::struct_array(&pkg, bytes)?)?,
                );
            }
        }
        objects.push(serde_json::json!({"index":i+1,"name":pkg.object_path(i as i32+1)?,"class":class,"properties":props,"arrays":arrays}));
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(&serde_json::json!({"map":args[1],"objects":objects}))?,
    )?;
    Ok(())
}
