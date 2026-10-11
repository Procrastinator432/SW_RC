//! Resolve the DC15s ThrowGrenade sequence and the original release notify.
use rc_package::{properties, read_package};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-grenade-release-assets GAME_DATA OUTPUT_DIR".into());
    }
    let game = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let mut resolver = rc_inspect::assets::Assets::new(&[
        &game.join("Textures"),
        &game.join("Animations"),
        &game.join("System"),
    ])?;
    let mesh = rc_inspect::visual_assets::load(
        game,
        "hudarms.ukx",
        "HudArmsDC15s",
        &["ActionBreathe", "Walk", "Fire", "Reload", "ThrowGrenade"],
        &mut resolver,
    )?;
    let clip_index = mesh
        .clips
        .iter()
        .position(|c| c.sequence.name.name == "ThrowGrenade")
        .ok_or("Missing throw clip")?;
    let clip = &mesh.clips[clip_index];
    let animation = clip
        .source
        .split(':')
        .next()
        .ok_or("Missing animation source")?;
    let package = animation
        .split('.')
        .next()
        .ok_or("Missing animation package")?;
    let bytes = fs::read(game.join("Animations").join(format!("{package}.ukx")))?;
    let pkg = read_package(&bytes)?;
    let mut notifies = vec![];
    let mut releases = vec![];
    for notify in &clip.sequence.notifies {
        if notify.object_index <= 0 {
            return Err("Throw notify is not a local object".into());
        }
        let e = &pkg.exports[notify.object_index as usize - 1];
        let class = pkg.object_path(e.class)?;
        if class.ends_with(".AnimNotify_ReleaseGrenade") {
            releases.push(f32::from_bits(notify.time_bits));
        }
        notifies.push(serde_json::json!({"time":f32::from_bits(notify.time_bits),
            "time_bits":notify.time_bits,"name":notify.name,"object":pkg.object_path(notify.object_index)?,
            "class":class,"properties":properties::read(&pkg,&bytes,e)?}));
    }
    if releases.len() != 1 {
        return Err(format!(
            "Expected exactly one release notify, found {}",
            releases.len()
        )
        .into());
    }
    let rules = rc_game::grenade_throw::Assets {
        frames: clip.sequence.frames,
        rate: f32::from_bits(clip.sequence.rate_bits),
        release_frame: releases[0],
    };
    rules.validate()?;
    let clip_source = clip.source.clone();
    let prepared = mesh.prepare()?;
    for frame in [0., releases[0], 0.75] {
        if prepared
            .scene(&prepared.channel(clip_index, frame)?)?
            .triangles
            .is_empty()
        {
            return Err("Empty original throwing geometry".into());
        }
    }
    fs::write(
        out.join("throw-visual-assets.json"),
        serde_json::to_vec_pretty(&prepared.mesh)?,
    )?;
    fs::write(
        out.join("throw-assets.json"),
        serde_json::to_vec_pretty(&rules)?,
    )?;
    fs::write(
        out.join("throw-original.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "mesh":prepared.mesh.source,"clip":clip_source,"notifies":notifies,
            "frames":rules.frames,"rate":rules.rate,"release_frame":rules.release_frame,
            "evaluated_geometry_frames":3,"scope":"Original clip and notify preparation, not live weapon integration"
        }))?,
    )?;
    println!(
        "{clip_source}: {} frames, rate {}, release {}",
        rules.frames, rules.rate, rules.release_frame
    );
    Ok(())
}
