//! Preserve original movement-property values and provenance without applying a physics policy.
use std::{env, fs, path::Path};
#[path = "../defaults.rs"]
mod defaults;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("Usage: rc-controller-defaults <GameData> <report.json>".into());
    }
    let catalog = defaults::load(Path::new(&args[1]))?;
    let mut properties = Vec::new();
    let mut unresolved = 0;
    for (class, names) in [
        (
            "CTCharacters.PlayerCommando",
            vec![
                "GroundSpeed",
                "AccelRate",
                "DecelRate",
                "AirControl",
                "JumpZ",
                "MaxFallSpeed",
                "WalkSpeedRatio",
                "BackSpeedRatio",
                "SideSpeedRatio",
            ],
        ),
        (
            "Engine.PhysicsVolume",
            vec!["Gravity", "TerminalVelocity", "GroundFriction"],
        ),
    ] {
        for name in names {
            let result = catalog.resolve(class, name, 0);
            unresolved += usize::from(result.is_err());
            properties.push(serde_json::json!({"class":class,"property":name,"result":result}));
        }
    }
    let output = Path::new(&args[2]);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        output,
        serde_json::to_vec_pretty(
            &serde_json::json!({"properties":properties,"unresolved":unresolved,"scope":"Original package class defaults and Properties overlays with provenance; no map volume selection, config runtime resolution or native controller algorithm parity. Values are not automatically applied to diagnostic controller options."}),
        )?,
    )?;
    println!(
        "{} movement properties audited; {unresolved} unresolved",
        properties.len()
    );
    if unresolved > 0 {
        return Err("Movement defaults audit has explicit unresolved properties".into());
    }
    Ok(())
}
