//! Rebuild original geometry/actor imports independently of private mission scenes.
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("usage: rc-audio-geometry-assets GAME_DATA OUTPUT_DIRECTORY".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let catalog = rc_inspect::defaults::load(root)?;
    let mut reports = vec![];
    for name in ["geo_01a", "geo_01b", "geo_01c", "geo_03a"] {
        let data = fs::read(root.join("Maps").join(format!("{name}.ctm")))?;
        let pkg = rc_package::read_package(&data)?;
        let owners = if name == "geo_01a" {
            vec![736, 950, 951, 1135, 1136, 1137]
        } else {
            vec![]
        };
        let (assets, receipt) =
            rc_inspect::ambient_geometry::read(&pkg, &data, &catalog, &owners, name)?;
        let encoded = serde_json::to_vec(&assets)?;
        let copy: rc_game::ambient_geometry::Assets = serde_json::from_slice(&encoded)?;
        copy.validate()?;
        fs::write(out.join(format!("{name}.json")), &encoded)?;
        reports.push(serde_json::json!({"map":name,"nodes":assets.bsp.nodes.len(),"zones":assets.connectivity.len(),"receipt":receipt}));
    }
    let report = serde_json::json!({"passed":true,"maps":reports});
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{report}");
    Ok(())
}
