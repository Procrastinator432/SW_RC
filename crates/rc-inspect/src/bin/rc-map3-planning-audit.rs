use rc_package::{properties, read_package};
use std::{fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new("D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData");
    let out = Path::new("analysis/mission21-map3-planning");
    fs::create_dir_all(out)?;
    let data = fs::read(root.join("Maps/geo_01c.ctm"))?;
    let p = read_package(&data)?;
    let mut receipts = vec![];
    for id in [263, 193, 356] {
        let e = &p.exports[id - 1];
        let props = properties::read(&p, &data, e)?;
        let payload = p.payload(&data, e)?;
        let nested: Vec<_> = props.values.iter().filter(|v| v.struct_name.as_deref() == Some("TransitionInfo")).map(|v| {
            serde_json::json!({"property":v.name,"decoded":properties::tagged_struct(&p,&payload[v.payload_offset..v.payload_offset+v.bytes])})
        }).collect();
        receipts.push(serde_json::json!({"original":id,"properties":props,"nested":nested}));
    }
    fs::write(
        out.join("original-planning-receipt.json"),
        serde_json::to_vec_pretty(&receipts)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&receipts[0])?);
    Ok(())
}
