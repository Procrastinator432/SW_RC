//! Explicit original-geometry squad fixtures, separate from input-only acceptance.
use rc_game::{squad, Game, Input, STEP};
use std::{env, fs, path::Path};
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}
fn input(row: &[u8]) -> Input {
    let f = |i| f32::from_le_bytes(row[i..i + 4].try_into().unwrap()) as f64;
    let b = u32::from_le_bytes(row[16..20].try_into().unwrap());
    Input {
        forward: f(0),
        strafe: f(4),
        look: [f(8), f(12)],
        jump: b & 1 != 0,
        fire: b & 2 != 0,
        reload: b & 4 != 0,
        walking: b & 8 != 0,
        use_action: b & 16 != 0,
        throw_grenade: b & 32 != 0,
        crouch: b & 64 != 0,
    }
}
fn frame(g: &Game, out: &Path, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let pixels = g.render(640, 400)?;
    let mut ppm = b"P6\n640 400\n255\n".to_vec();
    for p in pixels {
        ppm.extend([(p >> 16) as u8, (p >> 8) as u8, p as u8]);
    }
    fs::write(out.join(format!("{name}.ppm")), ppm)?;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    let scene = fs::read(&args[1])?;
    let replay = fs::read(&args[2])?;
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let mut g = rc_game::bundle::decode(&scene)?;
    for row in replay[12..].chunks_exact(20) {
        g.advance(STEP, input(row))?;
        if g.state
            .demolition
            .as_ref()
            .unwrap()
            .flow
            .objective_completed
        {
            break;
        }
    }
    let base = g.state.clone();
    let p = g.state.pawn.body.position;
    let q = g.state.rendezvous.as_ref().unwrap().pawn.body.position;
    let crouched = g.state.squad.as_ref().unwrap().crouched;
    let extent = if crouched {
        [40., 40., 56.]
    } else {
        squad::EXTENT
    };
    let nav = if crouched {
        &g.tactics.as_ref().unwrap().assets.squad.crouch_navigation
    } else {
        &g.tactics.as_ref().unwrap().assets.squad.navigation
    };
    let mut connections = vec![];
    for n in &nav.nodes {
        if distance(q, n.position) < 1500. {
            connections.push(serde_json::json!({"node":n.name,"at":n.position,"distance":distance(q,n.position),"clear":squad::corridor(&g.world,q,n.position,extent)?.is_some()}));
        }
    }
    let target =
        rc_game::mission_motion::ground(&g.world, [p[0], p[1], p[2] + 120.], extent, 300.)?
            .unwrap();
    let mut ends = vec![];
    for n in &nav.nodes {
        if distance(target, n.position) < 1500. {
            ends.push(serde_json::json!({"node":n.name,"distance":distance(target,n.position),"clear":squad::corridor(&g.world,target,n.position,extent)?.is_some()}));
        }
    }
    fs::write(
        out.join("navigation-debug.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"player":p,"scorch":q,"tick":g.state.statistics.ticks,"start_connections":connections,"end_connections":ends,"graph":nav,"plan":squad::plan(nav,&g.world,q,target,extent)?,"direct":squad::corridor(&g.world,q,[p[0],p[1],p[2]-if crouched{28.}else{0.}],extent)?}),
        )?,
    )?;
    fs::write(out.join("baseline.json"), serde_json::to_vec_pretty(&base)?)?;
    frame(&g, out, "baseline")?;

    let restore = |g: &mut Game| {
        g.state = base.clone();
        g.state.health = 100;
        g.state.squad.as_mut().unwrap().order = squad::Order::Hold;
    };
    let mut near = None;
    for offset in [
        [120., 0., 0.],
        [-120., 0., 0.],
        [0., 120., 0.],
        [0., -120., 0.],
    ] {
        let at: [f64; 3] = std::array::from_fn(|j| p[j] + offset[j]);
        if let Some(at) = rc_game::mission_motion::ground(
            &g.world,
            [at[0], at[1], at[2] + 120.],
            squad::EXTENT,
            300.,
        )? {
            if g.wall_fraction(g.eye(), at)? == 1. {
                near = Some(at);
                break;
            }
        }
    }
    let near = near.ok_or("No clear nearby original floor fixture")?;
    let locate = |g: &mut Game| {
        let r = g.state.rendezvous.as_mut().unwrap();
        r.pawn.body.position = near;
        r.pawn.body.velocity = [0.; 3];
        r.pawn.body.grounded = true;
    };
    restore(&mut g);
    locate(&mut g);
    g.aim_at(near);
    g.advance(
        STEP,
        Input {
            use_action: true,
            ..Input::default()
        },
    )?;
    assert_eq!(g.state.squad.as_ref().unwrap().order, squad::Order::Follow);
    for _ in 0..30 {
        g.advance(
            STEP,
            Input {
                use_action: true,
                ..Input::default()
            },
        )?;
    }
    assert_eq!(g.state.squad.as_ref().unwrap().orders, 1);
    g.advance(STEP, Input::default())?;
    g.advance(
        STEP,
        Input {
            use_action: true,
            ..Input::default()
        },
    )?;
    assert_eq!(g.state.squad.as_ref().unwrap().order, squad::Order::Hold);
    let orders = serde_json::to_value(g.state.squad.as_ref().unwrap())?;
    restore(&mut g);
    locate(&mut g);
    g.state.squad.as_mut().unwrap().health = 0;
    for _ in 0..60 {
        g.advance(
            STEP,
            Input {
                use_action: true,
                ..Input::default()
            },
        )?;
    }
    assert!(g.state.squad.as_ref().unwrap().progress > 0.);
    g.advance(STEP, Input::default())?;
    assert_eq!(g.state.squad.as_ref().unwrap().progress, 0.);
    let mut scorch_ramp = vec![];
    for _ in 0..600 {
        g.advance(
            STEP,
            Input {
                use_action: true,
                ..Input::default()
            },
        )?;
        if let Some(elapsed) = g.state.squad.as_ref().unwrap().scorch_recovery {
            scorch_ramp.push((elapsed, g.state.squad.as_ref().unwrap().health));
        }
        if g.state.squad.as_ref().unwrap().scorch_revives > 0 {
            break;
        }
    }
    assert!(scorch_ramp
        .iter()
        .any(|(_, health)| *health > 1 && *health < 200));
    assert!(scorch_ramp.windows(2).all(|w| w[0].1 <= w[1].1));
    assert_eq!(g.state.squad.as_ref().unwrap().clip, "ActionBreathe");
    assert!(g
        .state
        .squad
        .as_ref()
        .unwrap()
        .scorch_recovery_health
        .is_none());
    assert_eq!(g.state.squad.as_ref().unwrap().health, 200);
    assert!(!g.state.squad.as_ref().unwrap().down);
    let scorch_revive = serde_json::to_value(g.state.squad.as_ref().unwrap())?;
    g.aim_at(g.state.rendezvous.as_ref().unwrap().pawn.body.position);
    frame(&g, out, "scorch-revived")?;
    restore(&mut g);
    locate(&mut g);
    g.state.health = 0;
    g.advance(STEP, Input::default())?;
    assert!(g.state.squad.as_ref().unwrap().player_down);
    assert_eq!(g.state.phase, rc_game::Phase::Playing);
    let frozen = g.state.pawn.body.position;
    for _ in 0..30 {
        g.advance(
            STEP,
            Input {
                forward: 1.,
                fire: true,
                ..Input::default()
            },
        )?;
    }
    assert_eq!(g.state.pawn.body.position, frozen);
    assert_eq!(g.state.squad.as_ref().unwrap().progress, 0.);
    let mut player_ramp = vec![];
    for _ in 0..600 {
        g.advance(
            STEP,
            Input {
                use_action: true,
                ..Input::default()
            },
        )?;
        if let Some(elapsed) = g.state.squad.as_ref().unwrap().player_recovery {
            player_ramp.push((elapsed, g.state.health));
        }
        if g.state.squad.as_ref().unwrap().player_revives > 0 {
            break;
        }
    }
    assert!(!g.state.squad.as_ref().unwrap().player_down);
    assert_eq!(
        g.state.health,
        (g.mission.as_ref().unwrap().max_health / 2.) as u32
    );
    assert!(player_ramp
        .iter()
        .any(|(_, health)| *health > 1 && *health < 50));
    assert!(player_ramp.windows(2).all(|w| w[0].1 <= w[1].1));
    // The original Tick has no half-health cap. This long player clip can
    // exceed the EndState value before EndState assigns MaxHealth/2.
    assert!(player_ramp.last().unwrap().1 > g.state.health);
    assert!(g
        .state
        .squad
        .as_ref()
        .unwrap()
        .player_recovery_health
        .is_none());
    fs::write(
        out.join("revival-ramp.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"source":"Engine.Pawn.Reviving BeginState/Tick/EndState","scorch":scorch_ramp,"player":player_ramp,"fractional_accumulation":true,"idle_after_getup":true,"scope":"isolated original-geometry fixture; setup uses actor/health assignment"}),
        )?,
    )?;
    let player_revive = serde_json::to_value(g.state.squad.as_ref().unwrap())?;
    restore(&mut g);
    locate(&mut g);
    g.state.health = 0;
    g.state.squad.as_mut().unwrap().health = 0;
    g.advance(STEP, Input::default())?;
    assert_eq!(g.state.phase, rc_game::Phase::Lost);
    restore(&mut g);
    let start = g.state.rendezvous.as_ref().unwrap().pawn.body.position;
    g.state.squad.as_mut().unwrap().order = squad::Order::Follow;
    for _ in 0..240 {
        g.advance(STEP, Input::default())?;
    }
    let end = g.state.rendezvous.as_ref().unwrap().pawn.body.position;
    assert!(distance(end, p) < distance(start, p) - 200.);
    let follow = serde_json::to_value(g.state.squad.as_ref().unwrap())?;
    // Obtain a real spawned Warrior, then explicitly reposition the isolated combat fixture.
    restore(&mut g);
    for row in replay[12 + base.statistics.ticks as usize * 20..].chunks_exact(20) {
        g.advance(STEP, input(row))?;
        if !g.state.artillery.as_ref().unwrap().warriors.is_empty() {
            break;
        }
    }
    let mut warrior = g
        .state
        .artillery
        .as_ref()
        .unwrap()
        .warriors
        .first()
        .ok_or("No original Warrior spawned")?
        .clone();
    restore(&mut g);
    locate(&mut g);
    let mut combat_at = None;
    for offset in [
        [400., 0., 0.],
        [-400., 0., 0.],
        [0., 400., 0.],
        [0., -400., 0.],
    ] {
        let at = [near[0] + offset[0], near[1] + offset[1], near[2] + 160.];
        if let Some(at) =
            rc_game::mission_motion::ground(&g.world, at, rc_game::warrior_ai::EXTENT, 400.)?
        {
            if g.wall_fraction([near[0], near[1], near[2] + 44.], at)? == 1.
                && distance(p, at) > distance(near, at)
            {
                combat_at = Some(at);
                break;
            }
        }
    }
    let at = combat_at.ok_or("No clear Warrior combat fixture")?;
    warrior.pawn.body.position = at;
    warrior.pawn.body.grounded = true;
    warrior.health = 100;
    warrior.ai = None;
    g.state.artillery.as_mut().unwrap().warriors = vec![warrior];
    g.aim_at(at);
    g.advance(
        STEP,
        Input {
            use_action: true,
            ..Input::default()
        },
    )?;
    assert!(matches!(
        g.state.squad.as_ref().unwrap().order,
        squad::Order::Focus(_)
    ));
    let mut targeted_scorch = false;
    for _ in 0..900 {
        g.advance(STEP, Input::default())?;
        targeted_scorch |= g.state.artillery.as_ref().unwrap().warriors[0]
            .ai
            .as_ref()
            .and_then(|a| a.advanced.as_ref())
            .is_some_and(|a| a.target_scorch);
        if g.state.artillery.as_ref().unwrap().warriors[0].health == 0 {
            break;
        }
    }
    let fighting = g.state.squad.as_ref().unwrap();
    assert!(fighting.shots > 0 && fighting.hits > 0 && fighting.kills == 1);
    assert!(targeted_scorch);
    let combat = serde_json::to_value(fighting)?;
    g.aim_at(g.state.rendezvous.as_ref().unwrap().pawn.body.position);
    frame(&g, out, "squad-combat")?;
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"scope":"Explicit isolated fixtures on original geometry after an input-only demolition prefix; actor repositioning and health assignment are test setup, not a mission playthrough", "orders_edge_only":orders,"interrupted_hold_revive_resets":true,"scorch_revive":scorch_revive,"player_revive":player_revive,"incapacitated_input_blocks_movement_fire":true,"both_incapacitated_loses":true,"follow":follow,"combat":combat,"enemy_selects_scorch":targeted_scorch}),
        )?,
    )?;
    println!(
        "baseline tick{} Scorch{q:?} player{p:?}",
        g.state.statistics.ticks
    );
    Ok(())
}
