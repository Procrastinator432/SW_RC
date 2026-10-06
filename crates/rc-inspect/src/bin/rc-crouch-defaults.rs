use std::{env, fs, path::Path};
#[path = "../defaults.rs"]
mod defaults;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("Usage: rc-crouch-defaults <GameData> <report.json>".into());
    }
    let profile = rc_package::crouch::CrouchProfile::read(&defaults::load(Path::new(&args[1]))?)?;
    fs::write(&args[2], serde_json::to_vec_pretty(&profile)?)?;
    Ok(())
}
