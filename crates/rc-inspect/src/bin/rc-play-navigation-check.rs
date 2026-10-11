//! Exercise original-map detours, unsupported routes, goal damage and reset.
use rc_game::{bundle, navigation, replay, Input, Phase, STEP};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-play-navigation-check SCENE OUTPUT_DIRECTORY".into());
    }
    let bytes = fs::read(&args[1])?;
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let mut g = bundle::decode(&bytes)?;
    let initial = replay::state_fingerprint(&g)?;
    let mut fixture = None;
    let nav = g.combat.as_ref().unwrap().navigation.as_ref().unwrap();
    for a in &nav.nodes {
        for b in &nav.nodes {
            let d = (a.position[0] - b.position[0]).hypot(a.position[1] - b.position[1]);
            if !(600. ..1800.).contains(&d) || (a.position[2] - b.position[2]).abs() > 1. {
                continue;
            }
            if navigation::walkable(&g.world, a.position, b.position)? {
                continue;
            }
            let route = nav.plan(&g.world, a.position, b.position)?;
            if route.len() > 2 {
                fixture = Some((
                    a.name.clone(),
                    b.name.clone(),
                    a.position,
                    b.position,
                    route,
                ));
                break;
            }
        }
        if fixture.is_some() {
            break;
        }
    }
    let (from, to, start, end, route) =
        fixture.ok_or("No actual original-map wall detour fixture")?;
    g.set_encounter(&[start], g.exit)?;
    g.state.pawn.body.position = [end[0], end[1], end[2] + g.profile.physics.extent[2] - 60.];
    let mut travelled = 0.;
    let mut reached = false;
    let mut first_damage = None;
    for _ in 0..3600 {
        let before = g.state.enemies[0].position;
        g.advance(STEP, Input::default())?;
        let now = g.state.enemies[0].position;
        assert!(navigation::walkable(&g.world, before, now)?);
        travelled += (now[0] - before[0]).hypot(now[1] - before[1]);
        let d = (now[0] - end[0]).hypot(now[1] - end[1]);
        reached |= d <= 350.;
        let melee = g.state.combat.as_ref().unwrap().melee.as_ref().unwrap();
        if melee.player.shields < 100. {
            first_damage = Some(g.state.statistics.ticks);
            break;
        }
        if g.state.phase != Phase::Playing {
            return Err("Encounter ended before detour attack".into());
        }
    }
    assert!(
        reached && first_damage.is_some(),
        "Failed navigation from {from} to {to}: {:?}",
        g.state.combat.as_ref().unwrap().navigation
    );
    let agent = g
        .state
        .combat
        .as_ref()
        .unwrap()
        .navigation
        .as_ref()
        .unwrap()[0]
        .clone();
    assert!(agent.detours > 0 && agent.plans > 1);
    let hit = g
        .state
        .combat
        .as_ref()
        .unwrap()
        .melee
        .as_ref()
        .unwrap()
        .player;
    assert_eq!((hit.health, hit.shields), (100., 37.5));
    // Moving the target invalidates the cached endpoint at the next one-second plan.
    g.state.pawn.body.position = [
        start[0],
        start[1],
        start[2] + g.profile.physics.extent[2] - 60.,
    ];
    for _ in 0..90 {
        g.advance(STEP, Input::default())?;
    }
    let updated = &g
        .state
        .combat
        .as_ref()
        .unwrap()
        .navigation
        .as_ref()
        .unwrap()[0];
    assert!((updated.target[0] - start[0]).abs() < 1.);
    assert!((updated.target[1] - start[1]).abs() < 1.);
    assert!(updated.plans > agent.plans);
    // A dead owner clears the queued attack and path on the next update.
    g.state.enemies[0].health = 0;
    g.advance(STEP, Input::default())?;
    assert!(g
        .state
        .combat
        .as_ref()
        .unwrap()
        .navigation
        .as_ref()
        .unwrap()[0]
        .route
        .is_empty());
    assert!(g
        .state
        .combat
        .as_ref()
        .unwrap()
        .melee
        .as_ref()
        .unwrap()
        .attacks[0]
        .active
        .is_none());
    // Outside supported map floor: no movement and bounded retries, then reset.
    g = bundle::decode(&bytes)?;
    let spawn = g.state.enemies[0].position;
    g.state.pawn.body.position = [1e6, 1e6, 1e6];
    for _ in 0..180 {
        g.advance(STEP, Input::default())?;
    }
    assert_eq!(g.state.enemies[0].position, spawn);
    let waiting = &g
        .state
        .combat
        .as_ref()
        .unwrap()
        .navigation
        .as_ref()
        .unwrap()[0];
    assert!((1..=2).contains(&waiting.plans));
    assert!(waiting.goal.is_none());
    g.restart();
    assert_eq!(initial, replay::state_fingerprint(&g)?);
    // Deterministic combat defeat without player input through the new goal host.
    for _ in 0..3600 {
        g.advance(STEP, Input::default())?;
        if g.state.phase == Phase::Lost {
            break;
        }
    }
    assert_eq!(g.state.phase, Phase::Lost);
    let mut cadences = vec![];
    for frame in [STEP / 2., STEP * 2., STEP * 6.] {
        let mut p = bundle::decode(&bytes)?;
        for _ in 0..(12. / frame).round() as usize {
            p.advance(frame, Input::default())?;
        }
        cadences.push(serde_json::to_vec(&p.state)?);
    }
    assert!(cadences.windows(2).all(|p| p[0] == p[1]));
    let mut p = bundle::decode(&bytes)?;
    for i in 0..p.state.enemies.len() {
        for _ in 0..200 {
            if p.state.enemies[i].health == 0 {
                break;
            }
            p.aim_at(p.state.enemies[i].position);
            p.advance(
                STEP,
                Input {
                    fire: true,
                    ..Input::default()
                },
            )?;
        }
        assert_eq!(p.state.enemies[i].health, 0);
    }
    assert_eq!(p.state.statistics.kills, 3);
    fs::write(
        out.join("navigation-validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"moving_target_replans":true,"cadence_120_30_10_equal":true,"projectile_three_kills":true,"original_map_detour":{"from":from,"to":to,"route":route,"travelled":travelled,"first_damage_tick":first_damage,"agent":agent},"every_motion_collision_and_floor_checked":true,"original_notify_first_hit":[100.,37.5],"dead_owner_clears_route_and_attack":true,"unsupported_target_waits_bounded_retries":true,"restart_exact":true,"default_encounter_defeat":true}),
        )?,
    )?;
    println!("Original-map detour {from} -> {to}; first damage {first_damage:?}, travelled {travelled:.2}; wait/death/reset/defeat passed");
    Ok(())
}
