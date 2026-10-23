//! Export direct original Sound.Params, for checking an audio host's bindings.
use std::{env, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() < 3 {
        return Err("usage: rc-audio-params GAME_DATA SOUND_PATH...".into());
    }
    let root = Path::new(&args[1]);
    let catalog = rc_inspect::defaults::load(root)?;
    for source in &args[2..] {
        let (params, receipt) = rc_inspect::audio_params::load(root, &catalog, source)?;
        println!("{}", serde_json::json!({"params":params,"receipt":receipt}));
    }
    Ok(())
}
