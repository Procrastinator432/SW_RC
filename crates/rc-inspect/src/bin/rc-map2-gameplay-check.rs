//! Map2 real-input entry regression plus explicitly positioned interaction fixtures.
use rc_game::{Game, Input, Point, STEP};
use std::{fs, path::Path};
fn step(g: &mut Game, n: usize, input: Input) -> Result<(), String> {
    for _ in 0..n {
        g.advance(STEP, input)?;
    }
    Ok(())
}
fn state(g: &Game) -> &rc_game::map_gameplay::State {
    g.state.map.as_ref().unwrap().gameplay.as_ref().unwrap()
}
fn event(g: &Game, e: &str) -> bool {
    state(g).events.contains(e)
}
fn place(g: &mut Game, target: Point) -> Result<(), String> {
    let p = rc_game::mission_motion::ground(
        &g.world,
        [target[0], target[1], target[2] + 200.],
        g.profile.physics.extent,
        2048.,
    )?
    .ok_or("No fixture support")?;
    g.state.pawn.body.position = p;
    g.state.pawn.body.velocity = [0.; 3];
    g.state.pawn.body.grounded = true;
    Ok(())
}
fn face(g: &mut Game, p: Point) {
    let eye = g.eye();
    let d: Point = std::array::from_fn(|i| p[i] - eye[i]);
    g.state.yaw = d[1].atan2(d[0]);
    g.state.pitch = d[2].atan2(d[0].hypot(d[1]));
}
fn approach(g: &mut Game, index: usize) -> Result<(), String> {
    let p = g
        .map
        .as_ref()
        .unwrap()
        .gameplay
        .as_ref()
        .unwrap()
        .assets
        .interactions[index]
        .position;
    for radius in [100., 150., 190.] {
        for angle in 0..24 {
            let a = angle as f64 * std::f64::consts::TAU / 24.;
            if place(g, [p[0] + a.cos() * radius, p[1] + a.sin() * radius, p[2]]).is_err() {
                continue;
            }
            face(g, p);
            let old = state(g).progress[index];
            step(
                g,
                1,
                Input {
                    use_action: true,
                    ..Input::default()
                },
            )?;
            if state(g).progress[index] > old {
                return Ok(());
            }
        }
    }
    Err(format!("Cannot use original interaction {}", index))
}
fn wait_event(g: &mut Game, e: &str) -> Result<(), String> {
    for _ in 0..3600 {
        if event(g, e) {
            return Ok(());
        }
        step(g, 1, Input::default())?;
    }
    Err(format!("Event missing {e}"))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = Path::new("analysis/mission17-map2-gameplay/delivery/check");
    fs::create_dir_all(out)?;
    let bytes = fs::read("analysis/mission17-map2-gameplay/delivery/geo_01b.rcplay")?;
    let mut g = rc_game::bundle::decode(&bytes)?;
    let h = serde_json::from_slice(&fs::read(
        "analysis/mission15-complete/delivery/playthrough/map-handoff.json",
    )?)?;
    g.restore_handoff(&h)?;
    let initial = serde_json::to_vec(&g.state)?;
    // Physical entry and front-door opening use normal controller input only.
    step(
        &mut g,
        240,
        Input {
            forward: 0.5,
            ..Input::default()
        },
    )?;
    assert!(event(&g, "OpenDoor.422"));
    assert!(event(&g, "SpawnEntrance"));
    assert!(g.state.pawn.body.position[0] > -9700.);
    fs::write(
        out.join("physical-entry.json"),
        serde_json::to_vec_pretty(&g.state)?,
    )?;
    let frame = g.render_frame(320, 200)?.gpu_packet(&g.scene);
    assert!(!frame.is_empty());
    g.restart();
    assert_eq!(initial, serde_json::to_vec(&g.state)?);
    // A downed companion carried from map1 remains visible and can be revived.
    let c = g.state.map.as_mut().unwrap().companion.as_mut().unwrap();
    c.health = 0;
    c.down = true;
    let down_position = c.position;
    place(&mut g, down_position)?;
    assert!(g.mission_status().contains("kampfunfaehig"));
    step(
        &mut g,
        179,
        Input {
            use_action: true,
            ..Input::default()
        },
    )?;
    assert!(
        g.state
            .map
            .as_ref()
            .unwrap()
            .companion
            .as_ref()
            .unwrap()
            .down
    );
    step(
        &mut g,
        2,
        Input {
            use_action: true,
            ..Input::default()
        },
    )?;
    assert!(
        !g.state
            .map
            .as_ref()
            .unwrap()
            .companion
            .as_ref()
            .unwrap()
            .down
    );
    assert_eq!(
        g.state
            .map
            .as_ref()
            .unwrap()
            .companion
            .as_ref()
            .unwrap()
            .health,
        100
    );
    g.restart();
    assert_eq!(initial, serde_json::to_vec(&g.state)?);
    // Remaining checks are explicit actor-position fixtures, not a continuous replay.
    g.state.health = 1000;
    assert!(approach(&mut g, 0).is_err());
    assert_eq!(state(&g).progress[0], 0.);
    assert!(!state(&g).used[0]);
    g.restart();
    g.state.health = 1000;
    let dialog = g
        .map
        .as_ref()
        .unwrap()
        .gameplay
        .as_ref()
        .unwrap()
        .assets
        .triggers
        .iter()
        .find(|t| t.original == 654)
        .unwrap()
        .position;
    place(&mut g, dialog)?;
    step(&mut g, 600, Input::default())?;
    assert!(state(&g).objectives[1]);
    let mut receipts = vec![];
    for i in 0..5 {
        let prerequisite = g
            .map
            .as_ref()
            .unwrap()
            .gameplay
            .as_ref()
            .unwrap()
            .assets
            .interactions[i]
            .prerequisite
            .clone();
        if !event(&g, &prerequisite) {
            if i == 1 {
                wait_event(&mut g, &prerequisite)?;
            } else {
                let p = g
                    .map
                    .as_ref()
                    .unwrap()
                    .gameplay
                    .as_ref()
                    .unwrap()
                    .assets
                    .triggers
                    .iter()
                    .find(|t| t.event == prerequisite)
                    .unwrap()
                    .position;
                place(&mut g, p)?;
                step(&mut g, 600, Input::default())?;
            }
        }
        approach(&mut g, i)?;
        step(
            &mut g,
            420,
            Input {
                use_action: true,
                ..Input::default()
            },
        )?;
        if g.map
            .as_ref()
            .unwrap()
            .gameplay
            .as_ref()
            .unwrap()
            .assets
            .interactions[i]
            .demolition
        {
            assert!(state(&g).armed[i]);
            assert!(!state(&g).used[i]);
            let ammo = g.state.ammo;
            step(
                &mut g,
                1,
                Input {
                    fire: true,
                    ..Input::default()
                },
            )?;
            assert!(!state(&g).used[i]);
            assert_eq!(g.state.ammo, ammo);
            let p = g
                .map
                .as_ref()
                .unwrap()
                .gameplay
                .as_ref()
                .unwrap()
                .assets
                .interactions[i]
                .position;
            let mut placed = false;
            for angle in 0..24 {
                let a = angle as f64 * std::f64::consts::TAU / 24.;
                if place(&mut g, [p[0] + a.cos() * 450., p[1] + a.sin() * 450., p[2]]).is_ok() {
                    placed = true;
                    break;
                }
            }
            assert!(placed);
            step(
                &mut g,
                1,
                Input {
                    fire: true,
                    ..Input::default()
                },
            )?;
            assert_eq!(g.state.ammo, ammo);
        }
        assert!(state(&g).used[i]);
        let completed_label = &g
            .map
            .as_ref()
            .unwrap()
            .gameplay
            .as_ref()
            .unwrap()
            .assets
            .interactions[i]
            .label;
        assert!(!g.mission_status().starts_with(completed_label));
        step(&mut g, 900, Input::default())?;
        g.state.health = 1000;
        g.state.phase = rc_game::Phase::Playing;
        if i == 0 {
            let fixer = state(&g).fixer.position;
            face(&mut g, fixer);
            let mut ppm = b"P6\n640 400\n255\n".to_vec();
            for pixel in g.render(640, 400)? {
                ppm.extend([(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8]);
            }
            fs::write(out.join("fixer.ppm"), ppm)?;
        }
        receipts.push(serde_json::json!({"interaction":i,"original":g.map.as_ref().unwrap().gameplay.as_ref().unwrap().assets.interactions[i].original,"used":state(&g).used[i],"objectives":state(&g).objectives,"fixer_joined":state(&g).fixer.joined}));
        fs::write(
            out.join(format!("interaction-{i}.json")),
            serde_json::to_vec_pretty(&g.state)?,
        )?;
    }
    assert!(state(&g).objectives[..7].iter().all(|v| *v));
    assert!(state(&g).fixer.joined);
    let exit = g
        .map
        .as_ref()
        .unwrap()
        .gameplay
        .as_ref()
        .unwrap()
        .assets
        .exit;
    place(&mut g, exit)?;
    step(&mut g, 1, Input::default())?;
    assert_eq!(g.state.phase, rc_game::Phase::Won);
    assert_eq!(
        state(&g).transition.as_deref(),
        Some("GEO_01C.ctm#playerstart")
    );
    fs::write(out.join("exit.json"), serde_json::to_vec_pretty(&g.state)?)?;
    g.restart();
    assert_eq!(initial, serde_json::to_vec(&g.state)?);
    // Each original factory must activate safely; tests do not mark unsupported spawns as passed.
    let factory_events: Vec<String> = g
        .map
        .as_ref()
        .unwrap()
        .gameplay
        .as_ref()
        .unwrap()
        .assets
        .factories
        .iter()
        .map(|f| f.event.clone())
        .collect();
    for e in factory_events {
        g.state
            .map
            .as_mut()
            .unwrap()
            .gameplay
            .as_mut()
            .unwrap()
            .events
            .insert(e);
    }
    g.state.health = 1000;
    step(&mut g, 1, Input::default())?;
    assert_eq!(g.state.enemies.len(), 42);
    assert!(g.state.enemies.iter().all(|e| e.health > 0));
    let deaths: Vec<usize> = state(&g)
        .enemy_factories
        .iter()
        .enumerate()
        .filter(|(_, f)| {
            g.map
                .as_ref()
                .unwrap()
                .gameplay
                .as_ref()
                .unwrap()
                .assets
                .factories[**f]
                .death_event
                == "FinalHallwayDroids"
        })
        .map(|(i, _)| i)
        .collect();
    assert_eq!(deaths.len(), 4);
    for i in deaths {
        g.state.enemies[i].health = 0;
    }
    step(&mut g, 1, Input::default())?;
    assert!(event(&g, "FinalHallwayDroidsDead"));
    step(&mut g, 1, Input::default())?;
    assert_eq!(state(&g).event_counts["FinalHallwayDroids"], 4);
    let bridge = g
        .map
        .as_ref()
        .unwrap()
        .gameplay
        .as_ref()
        .unwrap()
        .assets
        .factories
        .iter()
        .position(|f| f.original == 245)
        .unwrap();
    for _ in 0..6 {
        let ids: Vec<_> = state(&g)
            .enemy_factories
            .iter()
            .enumerate()
            .filter(|(_, f)| **f == bridge)
            .map(|(i, _)| i)
            .collect();
        for i in ids {
            g.state.enemies[i].health = 0;
        }
        step(&mut g, 61, Input::default())?;
    }
    assert_eq!(state(&g).factories[bridge], 6);
    let spawned = g.state.enemies.len();
    step(&mut g, 121, Input::default())?;
    assert_eq!(g.state.enemies.len(), spawned);
    g.render_frame(320, 200)?;
    fs::write(
        out.join("factory-fixture.json"),
        serde_json::to_vec_pretty(&g.state)?,
    )?;
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"physical_entry_inputs":240,"interaction_fixtures":receipts,"factories":42,"restart_exact":true,"exit":"GEO_01C.ctm#playerstart","limitations":["Actor-position fixtures for all five interactions; not a complete physical route replay","Fixture health reset to 1000 between encounters","Factory fixture injects original spawn events; not proof every factory is naturally reached"]}),
        )?,
    )?;
    println!("Map2 physical entry + 5 interaction fixtures + all 42 factory spawns/reset passed");
    Ok(())
}
