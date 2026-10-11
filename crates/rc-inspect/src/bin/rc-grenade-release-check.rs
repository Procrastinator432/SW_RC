//! Drive the prepared throw gate with actual original DC15s channel frames.
use rc_game::{
    grenade_throw::{Assets, Effect, State},
    visual::Mesh,
};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 2 {
        return Err("usage: rc-grenade-release-check ASSET_DIR".into());
    }
    let out = Path::new(&args[1]);
    let assets: Assets = serde_json::from_slice(&fs::read(out.join("throw-assets.json"))?)?;
    assets.validate()?;
    let mesh: Mesh = serde_json::from_slice(&fs::read(out.join("throw-visual-assets.json"))?)?;
    let mesh = mesh.prepare()?;
    let index = mesh.clip("ThrowGrenade")?;
    let sequence = &mesh.mesh.clips[index].sequence;
    if assets.frames != sequence.frames
        || assets.rate.to_bits() != sequence.rate_bits
        || sequence
            .notifies
            .iter()
            .filter(|n| n.time_bits == assets.release_frame.to_bits())
            .count()
            != 1
    {
        return Err("Throw gate is not bound to original sequence".into());
    }
    let mut ammo = 5;
    let mut traces = vec![];
    for attempt in 0..5 {
        let mut state = State::default();
        if state.begin(ammo) != [Effect::PlayThrowAnimation] {
            return Err("Throw refused".into());
        }
        let before = ammo;
        let mut release_tick = None;
        let mut end_tick = None;
        for tick in 1..=120 {
            if !state.blocks_weapon() {
                return Err("Premature weapon unlock".into());
            }
            if !state.begin(ammo).is_empty() {
                return Err("Reentrant throw".into());
            }
            let mut channel = mesh.channel(index, state.frame)?;
            rc_package::skeletal_animation_tick::advance_channel_frame(
                &mut channel,
                assets.frames,
                assets.rate,
                rc_game::STEP as f32,
            );
            let next = f32::from_bits(channel.words[7]);
            let last = 1. - 1. / assets.frames as f32;
            let finished = next >= last;
            let effects = state.animation(&assets, next.min(last), finished, &mut ammo)?;
            if effects.contains(&Effect::ReleaseProjectile) && release_tick.replace(tick).is_some()
            {
                return Err("Duplicate release".into());
            }
            if release_tick.is_none() && ammo != before {
                return Err("Premature ammo consumption".into());
            }
            if effects.contains(&Effect::ResumeWeapon) {
                end_tick = Some(tick);
                break;
            }
        }
        let released = release_tick.ok_or("No original release notify")?;
        let ended = end_tick.ok_or("No animation end")?;
        if ended <= released || state.blocks_weapon() || ammo + 1 != before {
            return Err("Invalid throw lifecycle".into());
        }
        traces.push(serde_json::json!({"attempt":attempt+1,"release_tick":released,"end_tick":ended,"remaining":ammo}));
    }
    let mut empty = State::default();
    if empty.begin(ammo) != [Effect::EmptySound] || empty.blocks_weapon() {
        return Err("Empty throw accepted".into());
    }
    fs::write(
        out.join("release-validation.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "passed":true,"original_clip":mesh.mesh.clips[index].source,"original_release_frame":assets.release_frame,
            "five_notify_consumptions":true,"empty_sound":true,"reentrant_throw_ignored":true,
            "weapon_gate_until_animation_end":true,"traces":traces,
            "scope":"Real original channel frames driving selected throw state. No Game input, projectile world trajectory, JNI or Android integration claim"
        }))?,
    )?;
    println!("Five original DC15s throw animations released once each; empty inventory rejected");
    Ok(())
}
