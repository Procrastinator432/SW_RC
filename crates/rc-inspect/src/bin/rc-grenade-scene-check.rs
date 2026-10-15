//! Input-driven contiguous mission check; combat behavior remains a port adapter.
use rc_game::{bundle, Input, Phase, Point, STEP};
use std::{env, fs, path::Path};
fn dist(a: Point, b: Point) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}
fn image(g: &rc_game::Game, out: &Path, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let pixels = g.render(640, 400)?;
    let mut bytes = Vec::with_capacity(pixels.len() * 4);
    for pixel in pixels {
        bytes.extend_from_slice(&pixel.to_le_bytes());
    }
    fs::write(out.join(format!("{name}.argb")), bytes)?;
    Ok(())
}
fn steer(g: &rc_game::Game, target: Point, forward: f64, fire: bool) -> Input {
    let p = g.state.pawn.body.position;
    let wanted = (target[1] - p[1]).atan2(target[0] - p[0]);
    Input {
        forward,
        fire,
        look: [
            (g.state.yaw - wanted + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI,
            g.state.pitch,
        ],
        ..Input::default()
    }
}
fn walking(
    g: &mut rc_game::Game,
    route: &[Point],
    trace: &mut Vec<serde_json::Value>,
) -> Result<(), String> {
    for (i, target) in route.iter().enumerate() {
        let mut reached = false;
        let mut stalled = 0;
        let mut best = f64::INFINITY;
        for _ in 0..1800 {
            let d = dist(g.state.pawn.body.position, *target);
            if d < 25. {
                reached = true;
                break;
            }
            if g.state
                .mission
                .as_ref()
                .and_then(|m| m.grenades.as_ref())
                .is_some_and(|s| s.barrier_started && !s.barrier_open)
            {
                g.advance(STEP, Input::default())?;
                stalled = 0;
                continue;
            }
            let input = steer(g, *target, (d / 90.).min(1.), false);
            g.advance(STEP, input)?;
            if d < best - 1. {
                best = d;
                stalled = 0;
            } else {
                stalled += 1;
            }
            if stalled > 120 {
                break;
            }
            if g.state.phase != Phase::Playing {
                return Err(format!("Phase {:?} on route point{i}", g.state.phase));
            }
        }
        if !reached {
            return Err(format!(
                "Stuck {:?} -> route{i} {target:?}",
                g.state.pawn.body.position
            ));
        }
        trace.push(serde_json::json!({"waypoint":i,"position":g.state.pawn.body.position,"health":g.state.health,"enemies":g.state.enemies.len(),"dynamic":g.state.dynamic}));
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-grenade-scene-check SCENE OUTPUT".into());
    }
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let bytes = fs::read(&args[1])?;
    let mut g = bundle::decode(&bytes)?;
    let initial = serde_json::to_vec(&g.state)?;
    assert!(g.state.dynamic.as_ref().unwrap().iter().all(|v| !*v));
    assert!(g.state.enemies.is_empty());
    let closed_frame = g.render_frame(320, 200)?.gpu_packet(&g.scene);
    let closed_triangles = g.render_frame(320, 200)?.triangles.len();
    let static_packet = rc_render::gpu::world(&g.scene)?;
    let route = g.mission.as_ref().unwrap().route.clone();
    let mut trace = vec![];
    walking(&mut g, &route, &mut trace)?;
    g.aim_at([-635.5656, -999.2758, -2286.]);
    image(&g, out, "barricade-closed")?;
    g.aim_at(g.mission.as_ref().unwrap().station);
    for _ in 0..240 {
        g.advance(
            STEP,
            Input {
                use_action: true,
                ..Input::default()
            },
        )?;
        if g.state.mission.as_ref().unwrap().completed {
            break;
        }
    }
    assert_eq!(g.state.health, 100);
    assert_eq!(g.state.phase, Phase::Playing);
    for _ in 0..400 {
        g.advance(STEP, Input::default())?;
    }
    let state = g.state.mission.as_ref().unwrap().crash.as_ref().unwrap();
    assert_eq!(state.flow.stage, rc_game::crash::Stage::Approach);
    assert_eq!(
        &g.state.dynamic.as_ref().unwrap()[..8],
        &[true, false, true, false, false, false, false, false]
    );
    assert_eq!(rc_render::gpu::world(&g.scene)?, static_packet);
    let open_triangles = g.render_frame(320, 200)?.triangles.len();
    assert!(open_triangles < closed_triangles);
    g.aim_at([-635.5656, -999.2758, -2286.]);
    image(&g, out, "barricade-open")?;
    let route = g
        .mission
        .as_ref()
        .unwrap()
        .crash
        .as_ref()
        .unwrap()
        .route
        .clone();
    walking(&mut g, &route, &mut trace)?;
    assert_eq!(g.state.enemies.len(), 5);
    assert!(
        g.state
            .mission
            .as_ref()
            .unwrap()
            .crash
            .as_ref()
            .unwrap()
            .flow
            .vulnerable
    );
    let target = g.state.enemies[0].position;
    g.aim_at(target);
    image(&g, out, "crash-site")?;
    let mut combat = vec![];
    for _ in 0..3600 {
        if g.state.phase != Phase::Playing || g.state.statistics.kills == 5 {
            break;
        }
        let target = g
            .state
            .enemies
            .iter()
            .filter(|e| e.health > 0)
            .filter(|e| {
                g.wall_fraction(g.eye(), [e.position[0], e.position[1], e.position[2] + 15.])
                    .is_ok_and(|f| f >= 1.)
            })
            .min_by(|a, b| {
                dist(a.position, g.state.pawn.body.position)
                    .total_cmp(&dist(b.position, g.state.pawn.body.position))
            })
            .map(|e| e.position);
        if let Some(mut target) = target {
            target[2] += 15.;
            g.aim_at(target);
        }
        g.advance(
            STEP,
            Input {
                fire: target.is_some(),
                reload: g.state.ammo == 0,
                ..Input::default()
            },
        )?;
        if g.state.statistics.ticks % 30 == 0 {
            combat.push(serde_json::json!({"tick":g.state.statistics.ticks,"health":g.state.health,"enemy_health":g.state.enemies.iter().map(|e|e.health).collect::<Vec<_>>(),"counter":g.state.mission.as_ref().unwrap().crash.as_ref().unwrap().flow.counter_remaining}));
        }
    }
    fs::write(
        out.join("combat-trace.json"),
        serde_json::to_vec_pretty(&combat)?,
    )?;
    assert_eq!(g.state.phase, Phase::Playing);
    assert_eq!(g.state.statistics.kills, 5);
    assert_eq!(
        g.state
            .mission
            .as_ref()
            .unwrap()
            .crash
            .as_ref()
            .unwrap()
            .flow
            .counter_remaining,
        0
    );
    for _ in 0..65 {
        g.advance(STEP, Input::default())?;
    }
    assert_eq!(
        &g.state.dynamic.as_ref().unwrap()[..8],
        &[true, true, true, true, true, true, true, false]
    );
    image(&g, out, "walls-released")?;
    let route = g
        .mission
        .as_ref()
        .unwrap()
        .grenades
        .as_ref()
        .unwrap()
        .route
        .clone();
    walking(&mut g, &route[..route.len() - 1], &mut trace)?;
    g.aim_at(
        g.mission
            .as_ref()
            .unwrap()
            .grenades
            .as_ref()
            .unwrap()
            .flow
            .pickup
            .position,
    );
    image(&g, out, "pickup-before")?;
    walking(&mut g, &route[route.len() - 1..], &mut trace)?;
    image(&g, out, "grenades-collected")?;
    for _ in 0..1200 {
        let ready = g
            .state
            .mission
            .as_ref()
            .unwrap()
            .grenades
            .as_ref()
            .unwrap()
            .flow
            .pickup_script
            == rc_game::grenade_pickup::PickupStage::Done;
        if g.state.phase == Phase::Won || g.thermal.is_some() && ready {
            break;
        }
        g.advance(STEP, Input::default())?;
    }
    if g.thermal.is_some() {
        return live_throw_check(g, initial, closed_frame, static_packet, trace, out);
    }
    assert_eq!(g.state.phase, Phase::Won);
    let picked = g.state.mission.as_ref().unwrap().grenades.as_ref().unwrap();
    assert_eq!(picked.flow.ammunition, 5);
    assert!(picked.flow.objective_completed);
    assert!(picked.barrier_open && picked.walls_released);
    assert_eq!(g.state.dynamic.as_ref().unwrap(), &[true; 8]);
    let events = &g.state.mission.as_ref().unwrap().events;
    let mut counts = std::collections::BTreeMap::new();
    for e in events {
        *counts.entry(e.sound).or_insert(0u64) += 1;
    }
    for i in 0..13 {
        assert!(counts.contains_key(&i), "Missing sound {i}");
    }
    assert_eq!(counts[&4], u64::from(g.state.statistics.shots));
    let audio_counts = counts.clone();
    let complete_state = serde_json::to_value(&g.state)?;
    image(&g, out, "complete")?;
    let won_tick = g.state.statistics.ticks;
    g.restart();
    assert_eq!(serde_json::to_vec(&g.state)?, initial);
    assert_eq!(g.render_frame(320, 200)?.gpu_packet(&g.scene), closed_frame);
    assert_eq!(rc_render::gpu::world(&g.scene)?, static_packet);
    let before = serde_json::to_vec(&g.state)?;
    assert!(g.disable_dynamic(u32::MAX).is_err());
    assert_eq!(serde_json::to_vec(&g.state)?, before);
    fs::write(
        out.join("walking-trace.json"),
        serde_json::to_vec_pretty(&trace)?,
    )?;
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"input_driven_all_routes":true,"grenades":5,"objective2_completed":true,"connecting_barrier_completed":true,"thirteen_sound_roles":audio_counts,"weapon_events_match_shots":true,"complete_state":complete_state,"five_original_factory_instances":true,"kills":5,"won_tick":won_tick,
        "bacta_and_crash_continue_without_victory":true,"original_counter_completes":true,"restart_state_and_frame_exact":true,"static_gpu_packet_immutable":true,"transient_barrier_removed":true,"unknown_actor_unchanged":true,
        "scope":"Contiguous original objective0/1/2 with selected script/actor data; bounded resting wall pose and selected connecting dialogue/barrier effects, no general Karma or trooper staging. Provisional hitscan, enemy approach/shooting and cylinder combat; no original BattleDroid AI/DC15s parity or Android acceptance."}),
        )?,
    )?;
    println!("Contiguous Bacta/crash/grenade section: physical routes, five kills, five grenades, all thirteen audio roles and restart passed at tick{won_tick}");
    Ok(())
}

fn live_throw_check(
    mut g: rc_game::Game,
    initial: Vec<u8>,
    closed_frame: Vec<u8>,
    static_packet: Vec<u8>,
    mut walking_trace: Vec<serde_json::Value>,
    out: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(g.state.phase, Phase::Playing);
    assert_eq!(
        g.state
            .mission
            .as_ref()
            .unwrap()
            .grenades
            .as_ref()
            .unwrap()
            .flow
            .ammunition,
        5
    );
    let mut prior_sounds = std::collections::BTreeMap::<usize, u32>::new();
    for e in &g.state.mission.as_ref().unwrap().events {
        *prior_sounds.entry(e.sound).or_default() += 1;
    }
    for i in 0..13 {
        assert!(prior_sounds.contains_key(&i));
    }
    // The actual audio host drains events. Keep this longer diagnostic below the
    // bounded queue cap so early Foley is not evicted by the prior combat sounds.
    g.state.mission.as_mut().unwrap().events.clear();
    let route = g.thermal.as_ref().unwrap().assets.route.clone();
    walking(&mut g, &route, &mut walking_trace)?;
    for _ in 0..60 {
        g.advance(STEP, Input::default())?;
    }
    // Approach the last support at walking pace, then let real braking finish.
    // The earlier coarse route stops its input with substantial residual velocity.
    for _ in 0..900 {
        let target = *route.last().unwrap();
        if dist(g.state.pawn.body.position, target) < 3. {
            break;
        }
        let input = steer(&g, target, 0.03, false);
        g.advance(STEP, input)?;
    }
    for _ in 0..60 {
        g.advance(STEP, Input::default())?;
    }
    walking_trace.push(serde_json::json!({"throw_stand":g.state.pawn}));
    let target = g.thermal.as_ref().unwrap().assets.gun.contact.position;
    g.aim_at(target);
    g.state.pitch += if g.bunker.is_some() { 0.2 } else { 0.4 };
    if g.bunker.is_some() {
        image(&g, out, "bunker-intact")?;
    }
    let shots = g.state.statistics.shots;
    let reloads = g.state.statistics.reloads;
    // Queue a short press with no simulation tick, then release before the tick.
    assert_eq!(
        g.advance(
            0.,
            Input {
                throw_grenade: true,
                ..Input::default()
            }
        )?,
        0
    );
    let mut release_tick = None;
    let mut end_tick = None;
    let mut trace = vec![];
    let mut visible_pixels = 0;
    for tick in 1..=200 {
        let active = g.state.thermal.as_ref().unwrap().gate.active || tick == 1;
        g.advance(
            STEP,
            Input {
                throw_grenade: tick > 1,
                fire: active,
                reload: active,
                forward: if g.bunker.is_some() && tick <= 35 {
                    -1.
                } else if g.bunker.is_some() && tick < 155 {
                    1.
                } else {
                    0.
                },
                look: if g.bunker.is_some() && tick == 36 {
                    [std::f64::consts::PI, 0.]
                } else {
                    [0., 0.]
                },
                ..Input::default()
            },
        )?;
        if let Some(b) = &g.state.bunker {
            let name = if b.destruction_tick == Some(g.state.statistics.ticks) {
                Some("bunker-explosion")
            } else if b.explosions.iter().any(|e| !e.bunker && e.age <= STEP) {
                Some("thermal-explosion")
            } else {
                None
            };
            if let Some(name) = name {
                let camera = (g.state.yaw, g.state.pitch);
                g.aim_at(target);
                image(&g, out, name)?;
                let pixels = g.render(640, 400)?;
                let effects = std::mem::take(&mut g.state.bunker.as_mut().unwrap().explosions);
                let baseline = g.render(640, 400)?;
                g.state.bunker.as_mut().unwrap().explosions = effects;
                let visible = pixels
                    .iter()
                    .zip(baseline)
                    .filter(|(a, b)| **a != *b)
                    .count();
                assert!(visible > 0, "Original explosion sprite invisible: {name}");
                fs::write(
                    out.join(format!("{name}-visibility.json")),
                    serde_json::to_vec_pretty(
                        &serde_json::json!({"passed":true,"visible_pixels":visible}),
                    )?,
                )?;
                g.state.yaw = camera.0;
                g.state.pitch = camera.1;
            }
        }
        let s = g.state.thermal.as_ref().unwrap().clone();
        if s.released == 1 && release_tick.is_none() {
            release_tick = Some(tick);
            assert_eq!(
                g.state
                    .mission
                    .as_ref()
                    .unwrap()
                    .grenades
                    .as_ref()
                    .unwrap()
                    .flow
                    .ammunition,
                4
            );
            assert_eq!(s.projectiles[0].projectile.age, 0.);
            let frame = g.render_frame(640, 400)?;
            let projectiles = std::mem::take(&mut g.state.thermal.as_mut().unwrap().projectiles);
            let baseline = g.render_frame(640, 400)?;
            g.state.thermal.as_mut().unwrap().projectiles = projectiles;
            assert_eq!(frame.weapon_start, baseline.weapon_start + 352);
            visible_pixels = frame
                .render_cpu(&g.scene)?
                .iter()
                .zip(baseline.render_cpu(&g.scene)?)
                .filter(|(a, b)| **a != *b)
                .count();
            assert!(visible_pixels > 0, "Live grenade is invisible at release");
            fs::write(out.join("release.rcframe"), frame.gpu_packet(&g.scene))?;
            image(&g, out, "release")?;
            let before = serde_json::to_vec(&g.state)?;
            let volumes = std::mem::take(&mut g.thermal.as_mut().unwrap().assets.flight.volumes);
            assert!(g
                .advance(
                    STEP,
                    Input {
                        look: [0.1, 0.1],
                        ..Input::default()
                    }
                )
                .is_err());
            assert_eq!(
                serde_json::to_vec(&g.state)?,
                before,
                "Flight error partially committed game tick"
            );
            g.thermal.as_mut().unwrap().assets.flight.volumes = volumes;
        }
        if tick == 42 {
            image(&g, out, "flight")?;
            fs::write(
                out.join("flight.rcframe"),
                g.render_frame(640, 400)?.gpu_packet(&g.scene),
            )?;
        }
        if !s.gate.active && end_tick.is_none() {
            end_tick = Some(tick);
        }
        if active {
            assert_eq!(g.state.statistics.shots, shots);
            assert_eq!(g.state.statistics.reloads, reloads);
        }
        trace.push(serde_json::json!({"tick":tick,"pawn":g.state.pawn,"yaw":g.state.yaw,"pitch":g.state.pitch,"thermal":g.state.thermal,"ammunition":g.state.mission.as_ref().unwrap().grenades.as_ref().unwrap().flow.ammunition}));
    }
    fs::write(
        out.join("live-trace.json"),
        serde_json::to_vec_pretty(&trace)?,
    )?;
    fs::write(
        out.join("walking-trace.json"),
        serde_json::to_vec_pretty(&walking_trace)?,
    )?;
    if g.bunker.is_some() {
        return gunner_complete_check(
            g,
            initial,
            closed_frame,
            static_packet,
            walking_trace,
            trace,
            out,
        );
    }
    assert_eq!(release_tick, Some(35));
    assert_eq!(end_tick, Some(57));
    assert_eq!(
        g.state.thermal.as_ref().unwrap().released,
        1,
        "Held input repeated throw"
    );
    assert!(
        g.state.thermal.as_ref().unwrap().gun.objective_completed,
        "Live projectile missed gun trigger"
    );
    assert_eq!(g.state.thermal.as_ref().unwrap().detonated, 1);
    assert!(g.state.thermal.as_ref().unwrap().projectiles.is_empty());
    assert_eq!(
        g.state.phase,
        Phase::Playing,
        "Unbound bunker must not report victory"
    );
    let first_audio = g.state.mission.as_ref().unwrap().events.clone();
    // Pause clears the queued next throw, without resetting the ongoing animation.
    g.advance(STEP, Input::default())?;
    g.advance(
        0.,
        Input {
            throw_grenade: true,
            ..Input::default()
        },
    )?;
    g.suspend();
    g.advance(STEP, Input::default())?;
    assert_eq!(g.state.thermal.as_ref().unwrap().released, 1);
    g.aim_at(target); // Aim other throws into the same bounded world.
    for expected in 2..=5 {
        g.advance(
            STEP,
            Input {
                throw_grenade: true,
                ..Input::default()
            },
        )?;
        for _ in 0..200 {
            g.advance(STEP, Input::default())?;
        }
        assert_eq!(g.state.thermal.as_ref().unwrap().released, expected);
    }
    assert_eq!(
        g.state
            .mission
            .as_ref()
            .unwrap()
            .grenades
            .as_ref()
            .unwrap()
            .flow
            .ammunition,
        0
    );
    g.advance(
        STEP,
        Input {
            throw_grenade: true,
            ..Input::default()
        },
    )?;
    assert!(!g.state.thermal.as_ref().unwrap().gate.active);
    assert_eq!(g.state.thermal.as_ref().unwrap().released, 5);
    let m = g.state.mission.as_ref().unwrap();
    let mut sounds = std::collections::BTreeMap::<usize, u32>::new();
    for e in first_audio.iter().chain(&m.events) {
        *sounds.entry(e.sound).or_default() += 1;
    }
    assert_eq!(sounds[&13], 5, "throw Foley counts {sounds:?}");
    assert_eq!(sounds[&14], 5, "grab Foley counts {sounds:?}");
    assert!(sounds[&15] > 0);
    assert_eq!(sounds[&16], 1);
    let complete = serde_json::to_value(&g.state)?;
    assert_eq!(rc_render::gpu::world(&g.scene)?, static_packet);
    g.restart();
    assert_eq!(serde_json::to_vec(&g.state)?, initial);
    assert_eq!(g.render_frame(320, 200)?.gpu_packet(&g.scene), closed_frame);
    fs::write(
        out.join("live-trace.json"),
        serde_json::to_vec_pretty(&trace)?,
    )?;
    fs::write(
        out.join("walking-trace.json"),
        serde_json::to_vec_pretty(&walking_trace)?,
    )?;
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "passed":true,"input_driven_bacta_crash_pickup_gun_route":true,"release_tick":release_tick,"end_tick":end_tick,
            "queued_short_press":true,"held_button_no_repeat":true,"fire_reload_blocked":true,"five_consumptions_then_empty":true,
            "live_world_contact621":true,"first_fuse_once":true,"suspend_clears_pending":true,"restart_state_frame_exact":true,"flight_error_atomic":true,"release_visible_pixels":visible_pixels,"grenade_world_pass_faces":352,
            "sound_counts":sounds,"prior_sound_counts":prior_sounds,"static_gpu_packet_immutable":true,"complete_state":complete,
            "scope":"Live throw and contact only. Bunker destruction/collision transition, radius damage, thermal explosion FX/audio, hint/confirmation dispatch and Android acceptance remain open. No mission victory at trigger contact."
        }))?,
    )?;
    println!("Contiguous live throw passed: notify35/end57, five releases, held/empty, world trigger621, weapon gate, audio, pause and exact restart");
    Ok(())
}

fn gunner_complete_check(
    mut g: rc_game::Game,
    initial: Vec<u8>,
    closed_frame: Vec<u8>,
    static_packet: Vec<u8>,
    walking_trace: Vec<serde_json::Value>,
    mut trace: Vec<serde_json::Value>,
    out: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    g.aim_at(g.bunker.as_ref().unwrap().assets.bunker_position);
    fs::write(
        out.join("pre-complete.json"),
        serde_json::to_vec_pretty(&g.state)?,
    )?;
    for _ in 0..400 {
        if g.state.phase != Phase::Playing {
            break;
        }
        g.advance(STEP, Input::default())?;
        if !g.state.bunker.as_ref().unwrap().explosions.is_empty() {
            image(&g, out, "explosion")?;
        }
        trace.push(serde_json::json!({"tick":g.state.statistics.ticks,"state":g.state}));
    }
    fs::write(
        out.join("gunner-trace.json"),
        serde_json::to_vec_pretty(&trace)?,
    )?;
    assert_eq!(
        g.state.phase,
        Phase::Won,
        "{}",
        serde_json::to_string(&g.state)?
    );
    let b = g.state.bunker.as_ref().unwrap();
    let contact = b.contact_tick.unwrap();
    assert_eq!(b.destruction_tick.unwrap() - contact, 60);
    assert_eq!(b.confirmation_tick.unwrap() - contact, 120);
    assert_eq!(b.complete_tick.unwrap() - contact, 322);
    assert_eq!(b.health, 0);
    assert_eq!(g.state.dynamic.as_ref().unwrap(), &[true; 10]);
    let bunker = g
        .dynamic
        .as_ref()
        .unwrap()
        .objects
        .iter()
        .find(|o| o.original == 850)
        .unwrap();
    assert_eq!(bunker.released_triangles.len(), 1356);
    if let rc_package::world_collision::BodyShape::Hulls(h) = &g.world.actors[bunker.actor].shape {
        assert!(std::sync::Arc::ptr_eq(
            h,
            bunker.released_hulls.as_ref().unwrap()
        ));
        assert_eq!(h.hulls.len(), 35);
    } else {
        panic!("Destroyed bunker lost its original collision");
    }
    let hatch = g
        .dynamic
        .as_ref()
        .unwrap()
        .objects
        .iter()
        .find(|o| o.original == 1093)
        .unwrap();
    assert!(matches!(
        g.world.actors[hatch.actor].shape,
        rc_package::world_collision::BodyShape::Clear
    ));
    let mut sounds = std::collections::BTreeMap::<usize, u32>::new();
    for e in &g.state.mission.as_ref().unwrap().events {
        *sounds.entry(e.sound).or_default() += 1;
    }
    for role in [13, 14, 17, 18, 19] {
        assert_eq!(sounds.get(&role), Some(&1), "audio {sounds:?}");
    }
    let complete = serde_json::to_value(&g.state)?;
    image(&g, out, "complete")?;
    assert_eq!(rc_render::gpu::world(&g.scene)?, static_packet);
    let tick = g.state.statistics.ticks;
    g.advance(STEP, Input::default())?;
    assert_eq!(tick, g.state.statistics.ticks);
    g.restart();
    assert_eq!(serde_json::to_vec(&g.state)?, initial);
    assert_eq!(g.render_frame(320, 200)?.gpu_packet(&g.scene), closed_frame);
    fs::write(
        out.join("walking-trace.json"),
        serde_json::to_vec_pretty(&walking_trace)?,
    )?;
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"input_driven_objectives":[0,1,2,3],"sound_counts":sounds,"complete_state":complete,"destruction_delay_ticks":60,"confirmation_delay_ticks":120,"completion_delay_ticks":322,"exact_restart":true,"static_packet_immutable":true,"scope":"Selected original objectives 0-3, bounded combat and sprite adapters; next original mission section remains outside this milestone"}),
        )?,
    )?;
    println!("Gun-position milestone completed at tick{tick}: original objectives0-3, bunker mesh/collision, explosion/audio, delayed confirmation, victory and restart");
    Ok(())
}
