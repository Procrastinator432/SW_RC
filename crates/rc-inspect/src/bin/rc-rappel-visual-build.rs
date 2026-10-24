//! Opt in to a player-synchronized rope pose using existing original assets.
//! This is an explicit port visual adapter; no original receiver is inferred.
use std::{env, fs};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args().collect();
    if a.len() != 3 {
        return Err("usage: rc-rappel-visual-build INPUT.rcplay OUTPUT.rcplay".into());
    }
    let b = fs::read(&a[1])?;
    if !b.starts_with(b"RCPLAY13") {
        return Err("Expected opening scene".into());
    }
    let n = u32::from_le_bytes(b[8..12].try_into()?) as usize;
    let mut opening: rc_game::opening::Assets = serde_json::from_slice(&b[12..12 + n])?;
    opening
        .rappel
        .as_mut()
        .ok_or("Missing rappel")?
        .synchronize_rope = true;
    opening.provenance["rope_visual_adapter"] = serde_json::json!({"policy":"Use existing CloneEntrance600 rope clip at the player's saved EntranceRope600 channel frame; retain original actor transform and destruction event.","original_receiver_proven":false,"original_actor_trigger_clip":"CloneEntrance1760","camera":"unchanged","body":"unchanged"});
    let meta = serde_json::to_vec(&opening)?;
    let result = [
        b"RCPLAY13".as_slice(),
        &(meta.len() as u32).to_le_bytes(),
        &meta,
        &b[12 + n..],
    ]
    .concat();
    rc_game::bundle::decode(&result)?;
    fs::write(&a[2], result)?;
    Ok(())
}
