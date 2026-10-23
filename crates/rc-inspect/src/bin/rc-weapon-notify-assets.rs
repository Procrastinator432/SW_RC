//! Inspect every original notify on the installed first-person weapon clips.
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 4 {
        return Err("usage: rc-weapon-notify-assets GAME_DATA SCENE OUTPUT.json".into());
    }
    let game = rc_game::bundle::decode(&fs::read(&args[2])?)?;
    let mesh = &game.visuals.as_ref().ok_or("Missing weapon")?.weapon.mesh;
    let (_, receipts) = rc_inspect::weapon_notifies::load(Path::new(&args[1]), mesh)?;
    fs::write(&args[3], serde_json::to_vec_pretty(&receipts)?)?;
    Ok(())
}
