//! Read and probe original audio BSP metadata; does not alter playable scenes.
use rc_package::{audio_geometry, geometry, level, read_package};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("usage: rc-audio-geometry-check GAME_DATA OUTPUT_DIRECTORY".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let mut reports = vec![];
    for name in ["geo_01a", "geo_01b", "geo_01c", "geo_03a"] {
        let path = root.join("Maps").join(format!("{name}.ctm"));
        let data = fs::read(&path)?;
        let pkg = read_package(&data)?;
        let binding = level::world_binding(&pkg, &data)?;
        let export = &pkg.exports[binding.model_export];
        let bsp = audio_geometry::read_model(&pkg, &data, export)?;
        let geometry = geometry::read_bsp(&pkg, &data, export)?;
        let copy: audio_geometry::Bsp = serde_json::from_slice(&serde_json::to_vec(&bsp)?)?;
        let mut positions: Vec<[f32; 3]> = level::player_starts(&pkg, &data)?
            .into_iter()
            .map(|s| s.location)
            .collect();
        positions.extend(
            geometry
                .points
                .iter()
                .step_by((geometry.points.len() / 40).max(1))
                .copied()
                .take(40),
        );
        let mut queries = vec![];
        for (i, &start) in positions.iter().enumerate() {
            for direction in [
                [0., 0., 0.],
                [200., 0., 0.],
                [-200., 0., 0.],
                [0., 0., 200.],
                [0., 0., -200.],
            ] {
                let end = std::array::from_fn(|axis| start[axis] + direction[axis]);
                let clear = bsp.clear(start, end)?;
                let region = bsp.region(start, 999)?;
                assert_eq!(copy.clear(start, end)?, clear);
                assert_eq!(copy.region(start, 999)?, region);
                queries.push(serde_json::json!({"start":start,"end":end,"clear":clear,"region":region,"anchor":i}));
            }
        }
        let payload_end = export.serial_offset as usize + export.serial_size as usize;
        assert!(audio_geometry::read_model(&pkg, &data[..payload_end - 1], export).is_err());
        let filename = format!("{name}.json");
        fs::write(
            out.join(&filename),
            serde_json::to_vec(&serde_json::json!({"map":name,"bsp":bsp,"queries":queries}))?,
        )?;
        reports.push(serde_json::json!({"map":name,"nodes":bsp.nodes.len(),"zones":bsp.zone_actors.len(),"queries":queries.len(),"blocked":queries.iter().filter(|v|v["clear"]==false).count(),"roundtrip_exact":true,"truncation_rejected":true,"file":filename}));
    }
    let report = serde_json::json!({"passed":true,"maps":reports,"scope":"Original audio BSP metadata and queries; not connected to scene/runtime audio"});
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{report}");
    Ok(())
}
