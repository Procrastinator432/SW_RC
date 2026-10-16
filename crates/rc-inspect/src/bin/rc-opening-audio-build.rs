use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 4 {
        return Err("usage: rc-opening-audio-build GAME_DATA SCENE OUT".into());
    }
    let raw = fs::read(&args[2])?;
    if !raw.starts_with(b"RCPLAY13") {
        return Err("Expected final opening scene".into());
    }
    let game = rc_game::bundle::decode(&raw)?;
    let size = u32::from_le_bytes(raw[8..12].try_into()?) as usize;
    let mut assets: rc_game::opening::Assets = serde_json::from_slice(&raw[12..12 + size])?;
    rc_inspect::opening_audio::configure(
        Path::new(&args[1]),
        &mut assets,
        &game.mission.as_ref().ok_or("Missing mission bank")?.sounds,
    )?;
    let meta = serde_json::to_vec(&assets)?;
    let mut output = b"RCPLAY13".to_vec();
    output.extend_from_slice(&(meta.len() as u32).to_le_bytes());
    output.extend(meta);
    output.extend_from_slice(&raw[12 + size..]);
    rc_game::bundle::decode(&output)?;
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    fs::write(
        out.join("owned-audio.json"),
        serde_json::to_vec_pretty(&assets.provenance["owned_audio"])?,
    )?;
    fs::write(out.join("geo_01a.rcplay"), output)?;
    println!(
        "Bound {} original opening sound source policies",
        assets.sound_policies.len()
    );
    Ok(())
}
