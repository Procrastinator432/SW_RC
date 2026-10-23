//! Controlled presentation fixtures using real imported clips; not a mission replay.
use rc_game::Game;
use std::{env, fs, path::Path};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn tick(g: &mut Game, velocity: [f64; 3], shot: bool, blocked: bool) -> Result<()> {
    let old = g.state.clone();
    g.state.pawn.body.velocity = velocity;
    g.state.statistics.shots += u32::from(shot);
    let mut visual = g.state.visuals.take().ok_or("Missing visuals")?;
    let events = g.visuals.as_ref().ok_or("Missing assets")?.tick(
        &g.state,
        &old,
        &mut visual,
        None,
        g.dc15.as_ref().and_then(|a| a.animation.as_ref()),
        blocked,
    )?;
    g.state.visuals = Some(visual);
    g.emit_weapon_foley(&events)?;
    g.state.statistics.ticks += 1;
    Ok(())
}
fn name(g: &Game) -> &str {
    let v = g.state.visuals.as_ref().unwrap();
    &g.visuals.as_ref().unwrap().weapon.mesh.clips[v.weapon.clip]
        .sequence
        .name
        .name
}
fn runtime(g: &Game) -> &rc_game::weapon_animation::State {
    g.state
        .visuals
        .as_ref()
        .unwrap()
        .weapon_animation
        .as_ref()
        .unwrap()
}
fn capture(g: &mut Game, out: &Path, label: &str) -> Result<()> {
    let before = g.checkpoint("weapon-fixture")?;
    let view = g.render_frame(320, 180)?;
    let frame = view.gpu_packet(&g.scene);
    // Render is read-only; a repeated picture must not advance either channel.
    assert_eq!(before, g.checkpoint("weapon-fixture")?);
    assert_eq!(frame, g.render_frame(320, 180)?.gpu_packet(&g.scene));
    g.restore_checkpoint(&before, "weapon-fixture")?;
    assert_eq!(before, g.checkpoint("weapon-fixture")?);
    assert_eq!(frame, g.render_frame(320, 180)?.gpu_packet(&g.scene));
    fs::write(out.join(format!("{label}.rcsave")), before)?;
    fs::write(out.join(format!("{label}.frame")), frame)?;
    let mut ppm = b"P6\n320 180\n255\n".to_vec();
    for p in view.render_cpu(&g.scene)? {
        ppm.extend([(p >> 16) as u8, (p >> 8) as u8, p as u8]);
    }
    fs::write(out.join(format!("{label}.ppm")), ppm)?;
    Ok(())
}
fn corrupt(save: &[u8], path: &[&str], value: serde_json::Value) -> Vec<u8> {
    let mut body: serde_json::Value = serde_json::from_slice(&save[20..]).unwrap();
    let mut at = &mut body;
    for key in path {
        at = &mut at[*key];
    }
    *at = value;
    let data = serde_json::to_vec(&body).unwrap();
    let hash = data.iter().fold(0xcbf29ce484222325_u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    });
    let mut bytes = b"RCSAVE01".to_vec();
    bytes.extend((data.len() as u32).to_le_bytes());
    bytes.extend(hash.to_le_bytes());
    bytes.extend(data);
    bytes
}
fn main() -> Result<()> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("usage: rc-weapon-animation-check SCENE OUTPUT".into());
    }
    let raw = fs::read(&args[1])?;
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let mut g = rc_game::bundle::decode(&raw)?;
    let profile = g
        .dc15
        .as_ref()
        .and_then(|a| a.animation.as_ref())
        .ok_or("Need new weapon animation profile")?
        .clone();
    assert_eq!(profile.idle_delay, 20.);
    assert!(
        g.visuals
            .as_ref()
            .unwrap()
            .state(0)?
            .weapon_animation
            .is_some(),
        "Encounter and factory state rebuilds must retain the weapon module"
    );
    tick(&mut g, [0.; 3], false, false)?;
    for _ in 0..1199 {
        tick(&mut g, [0.; 3], false, false)?;
    }
    assert_eq!(name(&g), "ActionBreathe");
    tick(&mut g, [0.; 3], false, false)?;
    assert_eq!(name(&g), "Idle");
    assert!(runtime(&g).playing_idle);
    capture(&mut g, out, "idle-start")?;
    for _ in 0..8 {
        tick(&mut g, [0.; 3], false, false)?;
    }
    capture(&mut g, out, "idle-mid")?;
    for _ in 0..600 {
        tick(&mut g, [0.; 3], false, false)?;
    }
    assert_eq!(name(&g), "ActionBreathe");
    assert!(!runtime(&g).playing_idle);
    tick(&mut g, [100., 0., 0.], false, false)?;
    assert_eq!(name(&g), "Walk");
    assert!(runtime(&g).idle_started.is_none());
    capture(&mut g, out, "walk")?;
    tick(&mut g, [226., 0., 0.], false, false)?;
    assert_eq!(name(&g), "Run");
    capture(&mut g, out, "run")?;
    tick(&mut g, [226., 0., 0.], true, false)?;
    assert_eq!(name(&g), "Run");
    assert!(runtime(&g).fire.is_some());
    let before = g.state.visuals.as_ref().unwrap().weapon.frame;
    tick(&mut g, [226., 0., 0.], false, false)?;
    assert!(g.state.visuals.as_ref().unwrap().weapon.frame > before);
    capture(&mut g, out, "running-fire")?;
    let save = g.checkpoint("weapon-fixture")?;
    let mut resumed = rc_game::bundle::decode(&raw)?;
    resumed.restore_checkpoint(&save, "weapon-fixture")?;
    for _ in 0..60 {
        tick(&mut g, [226., 0., 0.], false, false)?;
        tick(&mut resumed, [226., 0., 0.], false, false)?;
    }
    assert_eq!(
        g.checkpoint("weapon-fixture")?,
        resumed.checkpoint("weapon-fixture")?
    );
    assert!(runtime(&g).fire.is_none());
    assert_eq!(name(&g), "Run");
    tick(&mut g, [0.; 3], true, false)?;
    let first = runtime(&g).fire.as_ref().unwrap().frame;
    tick(&mut g, [0.; 3], true, false)?;
    assert_eq!(runtime(&g).fire.as_ref().unwrap().frame, first);
    tick(&mut g, [0.; 3], false, true)?;
    assert!(runtime(&g).fire.is_none());
    assert!(runtime(&g).idle_started.is_none());
    assert_eq!(name(&g), "ActionBreathe");
    tick(&mut g, [0.; 3], true, false)?;
    let save = g.checkpoint("weapon-fixture")?;
    for (path, value) in [
        (
            vec!["state", "visuals", "weapon_animation", "idle_started"],
            serde_json::json!(g.state.statistics.ticks + 1),
        ),
        (
            vec!["state", "visuals", "weapon_animation", "fire", "clip"],
            serde_json::json!(0),
        ),
        (
            vec!["state", "visuals", "weapon_animation", "fire", "frame"],
            serde_json::json!(-0.1),
        ),
        (
            vec!["state", "visuals", "weapon_animation", "fire", "finished"],
            serde_json::json!(true),
        ),
        (
            vec!["initial", "visuals", "weapon_animation"],
            serde_json::Value::Null,
        ),
        (
            vec!["state", "visuals", "weapon_animation", "playing_idle"],
            serde_json::json!(true),
        ),
    ] {
        assert!(
            g.restore_checkpoint(&corrupt(&save, &path, value), "weapon-fixture")
                .is_err(),
            "{path:?}"
        );
        assert_eq!(save, g.checkpoint("weapon-fixture")?);
    }
    let mesh = &g.visuals.as_ref().unwrap().weapon;
    let base = mesh.channel(mesh.clip("Walk")?, 0.3)?;
    let fire = mesh.channel(mesh.clip(&profile.fire_anim)?, 0.5)?;
    assert_eq!(
        mesh.geometry_channels(&[base, fire.clone()])?.snapshot()?,
        mesh.geometry(&fire)?.snapshot()?
    );
    g.restart();
    assert!(runtime(&g).fire.is_none());
    assert!(runtime(&g).idle_started.is_none());
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "passed":true,"idle_delay":profile.idle_delay,"fire_anim":profile.fire_anim,
            "idle_once_and_return":true,"velocity_walk_run":true,"independent_base_fire_channels":true,
            "fire_end_and_restart":true,"blocked_clears_fire":true,"checkpoint_continuation_exact":true,
            "render_read_only":true,"corrupt_restores_atomic":6,"full_weight_overlay_matches_fire":true,
            "scope":"Controlled velocity/shot/clock presentation fixtures on real original meshes; separate ordinary-input campaign checks are required."
        }))?,
    )?;
    println!("Original weapon animation, channel composition and checkpoints passed");
    Ok(())
}
