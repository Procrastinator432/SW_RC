//! Check the original base event without claiming runtime handler resolution.
use rc_package::notify_wall::{notify_hit_wall, EmptyBaseNotifyWall, NotifyWallHandler};
use std::{env, fs};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("Usage: rc-notify-wall-probe <engine.u> <report.json>".into());
    }
    let data = fs::read(&args[1])?;
    let pkg = rc_package::read_package(&data)?;
    let mut verified = None;
    for (i, export) in pkg.exports.iter().enumerate() {
        let path = pkg.object_path(i as i32 + 1)?;
        if path == "Controller.NotifyHitWall" {
            let function = rc_package::script::read_function(&pkg, &data, export)?;
            verified = Some(EmptyBaseNotifyWall::verify(&path, &function)?);
        }
    }
    let base = verified.ok_or("missing base NotifyHitWall")?;
    // No state frame: native guard bypasses the name index entirely.
    let result = notify_hit_wall(0, None, NotifyWallHandler::EmptyBase(base))?;
    assert!(!result.handled);
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(&serde_json::json!({
            "package":args[1],"base_export":"Controller.NotifyHitWall","result":result,
            "scope":"Original empty base event verified. No state frame supplied. Runtime overrides and resolved FName index remain unknown; AI map diagnostics retain their explicit supplied snapshots."
        }))?,
    )?;
    Ok(())
}
