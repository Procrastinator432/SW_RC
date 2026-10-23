//! Real original clip/audio fixtures; ordinary mission validation remains separate.
use rc_game::Game;
use std::{env, fs, path::Path};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn tick(g: &mut Game, velocity: [f64; 3], shot: bool, blocked: bool) -> Result<()> {
    let old = g.state.clone();
    g.state.pawn.body.velocity = velocity;
    g.state.statistics.shots += u32::from(shot);
    let mut visual = g.state.visuals.take().ok_or("Missing visuals")?;
    let events = g.visuals.as_ref().unwrap().tick(
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
fn events(g: &Game) -> &[rc_game::combat::Event] {
    g.state
        .mission
        .as_ref()
        .map(|m| m.events.as_slice())
        .or_else(|| g.state.map.as_ref().map(|m| m.events.as_slice()))
        .unwrap()
}
fn main() -> Result<()> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("usage: rc-weapon-foley-check SCENE OUTPUT".into());
    }
    let raw = fs::read(&args[1])?;
    let mut g = rc_game::bundle::decode(&raw)?;
    let profile = g.dc15.as_ref().unwrap().animation.as_ref().unwrap().clone();
    let foley = profile.foley.as_ref().ok_or("Missing Foley profile")?;
    assert_eq!(foley.notifies.len(), 3);
    assert_eq!(foley.sounds.len(), 9);
    assert_eq!(foley.groups.len(), 1);
    assert_eq!(foley.groups[0].members.len(), 8);
    assert_eq!(foley.notifies.iter().filter(|n| n.clip == "Run").count(), 2);
    assert_eq!(
        foley.notifies.iter().filter(|n| n.clip == "Idle").count(),
        1
    );
    assert!(g.audio_bank().len() <= 128);
    let base = g.audio_bank().len() - foley.sounds.len();
    let armor: Vec<_> = foley.groups[0]
        .members
        .iter()
        .flatten()
        .map(|m| base + m.sound)
        .collect();
    let idle = base
        + foley
            .notifies
            .iter()
            .find(|n| n.clip == "Idle")
            .unwrap()
            .sound;
    // The first crossing includes the tiny positive entry timestamp exactly once.
    tick(&mut g, [226., 0., 0.], false, false)?;
    assert_eq!(events(&g).len(), 1);
    assert!(armor.contains(&events(&g)[0].sound));
    let entry = g.checkpoint("weapon-foley")?;
    let before_render = g.checkpoint("weapon-foley")?;
    g.render_frame(64, 36)?;
    assert_eq!(before_render, g.checkpoint("weapon-foley")?);
    let mut resumed = rc_game::bundle::decode(&raw)?;
    resumed.restore_checkpoint(&entry, "weapon-foley")?;
    for i in 0..100 {
        // Both channels are live: firing must not suppress base-channel notifies.
        tick(&mut g, [226., 0., 0.], i == 4, false)?;
        tick(&mut resumed, [226., 0., 0.], i == 4, false)?;
        assert_eq!(
            g.checkpoint("weapon-foley")?,
            resumed.checkpoint("weapon-foley")?
        );
    }
    assert_eq!(events(&g).len(), 5, "Two armor cues per original Run cycle");
    assert!(events(&g)
        .iter()
        .all(|e| armor.contains(&e.sound) && e.owner == -1));
    let running_events = events(&g).len();
    // Stop/locked state clears running channel. No phantom entry or late cue.
    for _ in 0..60 {
        tick(&mut g, [226., 0., 0.], false, true)?;
    }
    assert_eq!(events(&g).len(), running_events);
    tick(&mut g, [0.; 3], false, false)?;
    for _ in 0..1199 {
        tick(&mut g, [0.; 3], false, false)?;
    }
    assert_eq!(events(&g).len(), running_events);
    let before_idle = g.checkpoint("weapon-foley")?;
    resumed.restore_checkpoint(&before_idle, "weapon-foley")?;
    tick(&mut g, [0.; 3], false, false)?;
    tick(&mut resumed, [0.; 3], false, false)?;
    assert_eq!(events(&g).len(), running_events + 1);
    assert_eq!(events(&g).last().unwrap().sound, idle);
    assert_eq!(
        g.checkpoint("weapon-foley")?,
        resumed.checkpoint("weapon-foley")?
    );
    let after_idle = g.checkpoint("weapon-foley")?;
    resumed.restore_checkpoint(&after_idle, "weapon-foley")?;
    for _ in 0..120 {
        tick(&mut g, [0.; 3], false, false)?;
        tick(&mut resumed, [0.; 3], false, false)?;
    }
    assert_eq!(
        events(&g).len(),
        running_events + 1,
        "Idle cue must not repeat after restore"
    );
    assert_eq!(
        g.checkpoint("weapon-foley")?,
        resumed.checkpoint("weapon-foley")?
    );
    // Imported bindings must agree with exact object identities and timestamp bits.
    let mesh = &g.visuals.as_ref().unwrap().weapon;
    for defect in 0..5 {
        let mut bad = profile.clone();
        let f = bad.foley.as_mut().unwrap();
        match defect {
            0 => f.notifies[0].time += 0.01,
            1 => f.notifies[0].original += 1,
            2 => f.notifies[0].sound = f.sounds.len(),
            3 => {
                f.notifies.pop();
            }
            _ => f.sounds[0].wave[0] = 0,
        }
        assert!(bad.validate_mesh(mesh).is_err());
    }
    // Reject malformed event batches before changing the queue.
    let exact = g.checkpoint("weapon-foley")?;
    assert!(g
        .emit_weapon_foley(&[(0, -1), (foley.sounds.len(), -1)])
        .is_err());
    assert_eq!(exact, g.checkpoint("weapon-foley")?);
    // Even a correctly framed/checksummed save must reject an invalid sound role.
    let invalid_role = g.audio_bank().len();
    if let Some(m) = &mut g.state.mission {
        m.events.last_mut().unwrap().sound = invalid_role;
    } else {
        g.state
            .map
            .as_mut()
            .unwrap()
            .events
            .last_mut()
            .unwrap()
            .sound = invalid_role;
    }
    let corrupt = g.checkpoint("weapon-foley")?;
    g.restore_checkpoint(&exact, "weapon-foley")?;
    assert!(g.restore_checkpoint(&corrupt, "weapon-foley").is_err());
    assert_eq!(exact, g.checkpoint("weapon-foley")?);
    // Corrupt native selection state is rejected atomically, even with a valid save envelope.
    for defect in 0..5 {
        g.restore_checkpoint(&exact, "weapon-foley")?;
        let weapon = g
            .state
            .visuals
            .as_mut()
            .unwrap()
            .weapon_animation
            .as_mut()
            .unwrap();
        match defect {
            0 => weapon.selection = None,
            1 => weapon.selection.as_mut().unwrap().groups.clear(),
            2 => weapon.selection.as_mut().unwrap().groups[0].flags = 128,
            3 => weapon.selection.as_mut().unwrap().groups[0].which = 8,
            _ => weapon.selection.as_mut().unwrap().groups[0].rendered = Some(99),
        }
        let bad = g.checkpoint("weapon-foley")?;
        g.restore_checkpoint(&exact, "weapon-foley")?;
        assert!(g.restore_checkpoint(&bad, "weapon-foley").is_err());
        assert_eq!(exact, g.checkpoint("weapon-foley")?);
    }
    // Drive actual imported notify crossings until every original armor member
    // is selected. This is a controlled animation fixture, not player movement.
    for _ in 0..3000 {
        tick(&mut g, [226., 0., 0.], false, false)?;
    }
    assert!(armor
        .iter()
        .all(|s| events(&g).iter().any(|e| e.sound == *s)));
    let selected: Vec<_> = events(&g)
        .iter()
        .filter(|e| armor.contains(&e.sound))
        .map(|e| e.sound)
        .collect();
    assert!(selected.windows(2).any(|s| s[0] != s[1]));
    g.restart();
    assert!(events(&g).is_empty());
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "passed":true,"sounds":9,"original_armor_variants":8,"all_variants_selected":true,"invalid_selection_checkpoints_atomic":5,"original_notifies":3,"run_events_in_101_ticks":running_events,
            "fire_preserves_base_notifies":true,"blocked_silent":true,"idle_entry_once":true,
            "checkpoint_before_and_after_crossing_exact":true,"render_read_only":true,
        "invalid_profiles_rejected":5,"invalid_event_batch_atomic":true,"invalid_saved_audio_role_atomic":true,"restart_clears_events":true,
            "scope":"Controlled presentation fixtures on real clips and PCM, not an ordinary-input mission or subjective hearing test"
        }))?,
    )?;
    println!("Weapon Foley original bindings, crossings and continuation passed");
    Ok(())
}
