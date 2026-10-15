//! Mission-input AI acceptance, plus explicitly isolated original-geometry probes.
use rc_game::{Game, Input, Phase, STEP};
use std::{env, fs, path::Path};
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
fn record(out: &mut Vec<u8>, i: Input) {
    for f in [i.forward, i.strafe, i.look[0], i.look[1]] {
        out.extend((f as f32).to_le_bytes());
    }
    let bits = u32::from(i.jump)
        | (u32::from(i.fire) * 2)
        | (u32::from(i.reload) * 4)
        | (u32::from(i.walking) * 8)
        | (u32::from(i.use_action) * 16)
        | (u32::from(i.throw_grenade) * 32)
        | (u32::from(i.crouch) * 64);
    out.extend(bits.to_le_bytes());
}
fn aim(g: &Game, p: [f64; 3]) -> [f64; 2] {
    let eye = g.eye();
    let d: [f64; 3] = std::array::from_fn(|i| p[i] - eye[i]);
    let yaw = d[1].atan2(d[0]);
    let pitch = d[2].atan2(d[0].hypot(d[1]));
    [
        (g.state.yaw - yaw + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
            - std::f64::consts::PI,
        g.state.pitch - pitch,
    ]
}
fn advance(g: &mut Game, out: &mut Vec<u8>, i: Input) -> Result<(), String> {
    g.advance(STEP, i)?;
    record(out, i);
    Ok(())
}
fn save(out: &Path, name: &str, rows: &[u8]) -> Result<(), std::io::Error> {
    let mut v = b"RCINPUT1".to_vec();
    v.extend((rows.len() as u32 / 20).to_le_bytes());
    v.extend(rows);
    fs::write(out.join(name), v)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-warrior-ai-check SCENE NORMAL_INPUT OUT".into());
    }
    let scene = fs::read(&args[1])?;
    let replay = fs::read(&args[2])?;
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let mut g = rc_game::bundle::decode(&scene)?;
    let initial = serde_json::to_vec(&g.state)?;
    let mut prefix = vec![];
    for row in replay[12..].chunks_exact(20) {
        let mut command = input(row);
        if g.state
            .demolition
            .as_ref()
            .unwrap()
            .flow
            .objective_completed
            && g.state
                .artillery
                .as_ref()
                .unwrap()
                .warriors
                .iter()
                .any(|w| w.health > 0)
        {
            command.fire = false;
        }
        advance(&mut g, &mut prefix, command)?;
        if g.state
            .artillery
            .as_ref()
            .unwrap()
            .warriors
            .iter()
            .any(|w| {
                w.health > 30
                    && w.ai.as_ref().is_some_and(|a| {
                        a.last_seen.is_some()
                            && (!a.navigation.route.is_empty() || a.attack.is_some())
                    })
            })
        {
            break;
        }
    }
    assert!(g.state.phase == Phase::Playing);
    assert!(g
        .state
        .artillery
        .as_ref()
        .unwrap()
        .warriors
        .iter()
        .any(|w| w.health > 30));
    let health = g.state.health;
    let mut attack = None;
    for _ in 0..1800 {
        advance(&mut g, &mut prefix, Input::default())?;
        if let Some(i) = g
            .state
            .artillery
            .as_ref()
            .unwrap()
            .warriors
            .iter()
            .position(|w| w.ai.as_ref().is_some_and(|a| a.attack.is_some()))
        {
            attack = Some(i);
            break;
        }
        assert_eq!(g.state.phase, Phase::Playing);
    }
    if attack.is_none() {
        fs::write(
            out.join("failure-state.json"),
            serde_json::to_vec_pretty(&g.state)?,
        )?;
    }
    let i = attack.ok_or("Warrior never reached the visible stationary player")?;
    assert_eq!(
        g.state.health, health,
        "Damage occurred before animation notify"
    );
    let attack_tick = g.state.statistics.ticks;
    let attack_prefix = prefix.clone();
    for _ in 0..300 {
        advance(&mut g, &mut prefix, Input::default())?;
        if g.state.phase == Phase::Lost {
            break;
        }
    }
    assert_eq!(g.state.phase, Phase::Lost);
    assert!(g.state.statistics.ticks > attack_tick);
    assert_eq!(
        g.state.artillery.as_ref().unwrap().warriors[i]
            .ai
            .as_ref()
            .unwrap()
            .hits,
        1
    );
    save(out, "ai-defeat-input.bin", &prefix)?;
    let defeat = serde_json::json!({"tick":g.state.statistics.ticks,"attack_tick":attack_tick,"state":g.state,"normal_inputs_only":true});
    let restore = || -> Result<Game, String> {
        let mut g = rc_game::bundle::decode(&scene)?;
        for row in attack_prefix.chunks_exact(20) {
            g.advance(STEP, input(row))?;
        }
        Ok(g)
    };
    let mut dodged = restore()?;
    let mut dodge_rows = attack_prefix.clone();
    let enemy = dodged.state.artillery.as_ref().unwrap().warriors[i]
        .pawn
        .body
        .position;
    let look = aim(&dodged, enemy);
    for t in 0..120 {
        advance(
            &mut dodged,
            &mut dodge_rows,
            Input {
                forward: -1.,
                look: if t == 0 { look } else { [0.; 2] },
                ..Input::default()
            },
        )?;
        if dodged.state.artillery.as_ref().unwrap().warriors[i]
            .ai
            .as_ref()
            .unwrap()
            .misses
            > 0
        {
            break;
        }
    }
    assert_eq!(dodged.state.health, health);
    assert!(
        dodged.state.artillery.as_ref().unwrap().warriors[i]
            .ai
            .as_ref()
            .unwrap()
            .misses
            > 0,
        "Retreat did not evade notify"
    );
    save(out, "ai-dodge-input.bin", &dodge_rows)?;
    let mut interrupted = restore()?;
    let mut interrupt_rows = attack_prefix.clone();
    for _ in 0..60 {
        let at = interrupted.state.artillery.as_ref().unwrap().warriors[i]
            .pawn
            .body
            .position;
        let look = aim(&interrupted, at);
        advance(
            &mut interrupted,
            &mut interrupt_rows,
            Input {
                look,
                fire: true,
                ..Input::default()
            },
        )?;
        if interrupted.state.artillery.as_ref().unwrap().warriors[i]
            .ai
            .as_ref()
            .unwrap()
            .interruptions
            > 0
        {
            break;
        }
    }
    let ai = interrupted.state.artillery.as_ref().unwrap().warriors[i]
        .ai
        .as_ref()
        .unwrap();
    assert!(ai.interruptions > 0 && ai.attack.is_none());
    assert_eq!(ai.hits, 0);
    assert_eq!(interrupted.state.health, health);
    save(out, "ai-interrupt-input.bin", &interrupt_rows)?;
    let nav = &g.warrior_ai.as_ref().unwrap().navigation;
    let mut detour = None;
    'search: for from in &nav.nodes {
        for to in &nav.nodes {
            if rc_game::navigation::walkable_for(
                &g.world,
                from.position,
                to.position,
                rc_game::warrior_ai::EXTENT,
            )? {
                continue;
            }
            let route = nav.plan_for(
                &g.world,
                from.position,
                to.position,
                rc_game::warrior_ai::EXTENT,
            )?;
            if route.len() > 1 {
                let mut at = from.position;
                for p in &route {
                    assert!(rc_game::navigation::walkable_for(
                        &g.world,
                        at,
                        *p,
                        rc_game::warrior_ai::EXTENT
                    )?);
                    at = *p;
                }
                detour = Some(
                    serde_json::json!({"from":from.name,"to":to.name,"route":route,"isolated_original_geometry_probe":true}),
                );
                break 'search;
            }
        }
    }
    assert!(detour.is_some(), "No real obstacle detour found");
    interrupted.restart();
    assert_eq!(serde_json::to_vec(&interrupted.state)?, initial);
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"defeat":defeat,"dodge_state":dodged.state,"interrupt_before_restart":true,"detour":detour,"restart_exact":true,"attack_damage_delayed_to_original_notify":true}),
        )?,
    )?;
    println!("Warrior AI: normal-input pursuit, delayed notify defeat, retreat dodge, shot interruption, original obstacle detour and reset passed");
    Ok(())
}
