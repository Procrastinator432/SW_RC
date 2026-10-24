//! Audit original work notify objects before binding tools, sounds or effects.
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args_os().collect();
    if a.len() != 4 {
        return Err("usage: rc-work-notify-audit GAME_DATA WORK_CLIPS OUTPUT".into());
    }
    let root = Path::new(&a[1]);
    let clips: serde_json::Value = serde_json::from_slice(&fs::read(&a[2])?)?;
    let data = fs::read(root.join("Animations/Clone.ukx"))?;
    let pkg = rc_package::read_package(&data)?;
    let mut rows = vec![];
    for clip in clips["clips"].as_array().ok_or("Missing clips")? {
        for n in clip["sequence"]["notifies"]
            .as_array()
            .ok_or("Missing notifies")?
        {
            let id = n["object_index"].as_i64().ok_or("Missing notify index")? as i32;
            let e = pkg.exports.get((id - 1) as usize).ok_or("Invalid notify")?;
            let props = rc_package::properties::read(&pkg, &data, e)?;
            let row = serde_json::json!({"clip":clip["source"],"id":id,"time_bits":n["time_bits"],"time":f32::from_bits(n["time_bits"].as_u64().unwrap()as u32),"object":pkg.object_path(id)?,"class":pkg.object_path(e.class)?,"properties":props});
            println!("{}", serde_json::to_string(&row)?);
            rows.push(row);
        }
    }
    let mut classes = vec![];
    for file in ["System/ctmarkers.u", "System/ctinventory.u"] {
        let bytes = fs::read(root.join(file))?;
        let p = rc_package::read_package(&bytes)?;
        for (i, e) in p.exports.iter().enumerate() {
            if e.class != 0 {
                continue;
            }
            let name = p.object_path(i as i32 + 1)?;
            if !["Hack", "Demolition"].iter().any(|q| name.contains(q)) {
                continue;
            }
            let props = rc_package::classes::read(&p, &bytes, e)?.properties;
            println!("CLASS {} {}", name, serde_json::to_string(&props)?);
            classes.push(serde_json::json!({"file":file,"name":name,"properties":props}));
        }
    }
    let out = Path::new(&a[3]);
    if let Some(p) = out.parent() {
        fs::create_dir_all(p)?;
    }
    fs::write(out, serde_json::to_vec_pretty(&rows)?)?;
    fs::write(
        out.with_file_name("work-classes.json"),
        serde_json::to_vec_pretty(&classes)?,
    )?;
    Ok(())
}
