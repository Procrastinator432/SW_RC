use super::*;
use rc_package::{
    controller::{ControllerOptions, MovementRatios},
    geometry::Triangle,
    hull::{Hull, HullSet},
    physics::PhysicsOptions,
    volumes::VolumeSettings,
    world_collision::{BodyActor, BodyShape},
};
use std::{collections::BTreeMap, sync::Arc};
fn dc15_profile() -> dc15::Assets {
    dc15::Assets {
        provenance: serde_json::Value::Null,
        maximum: 5,
        recharge_rate: 1.5,
        cooldown_time: 0.,
        wait_for_recharge: false,
        damage: 55,
        shot_delay: 0.2,
        fire_mode: None,
        animation: None,
    }
}
fn semi_auto_game() -> Game {
    let mut g = game(false);
    let mut a = dc15_profile();
    a.fire_mode = Some(dc15::FireMode::SemiAuto);
    g.attach_dc15(a).unwrap();
    g
}
fn trigger(g: &mut Game, down: bool, elapsed: f64) {
    g.advance(
        elapsed,
        Input {
            fire: down,
            ..Default::default()
        },
    )
    .unwrap();
}
#[test]
fn dc15_semi_auto_hold_release_cooldown_and_empty_press_are_not_queued() {
    let mut g = semi_auto_game();
    for _ in 0..120 {
        trigger(&mut g, true, STEP);
    }
    assert_eq!(g.state.statistics.shots, 1);
    trigger(&mut g, false, STEP);
    trigger(&mut g, true, STEP);
    assert_eq!(g.state.statistics.shots, 2);
    // Too-early second press is consumed, not fired when the deadline expires.
    trigger(&mut g, false, STEP);
    for _ in 0..60 {
        trigger(&mut g, true, STEP);
    }
    assert_eq!(g.state.statistics.shots, 2);
    g.state.dc15.as_mut().unwrap().charge = 0.;
    g.state.dc15.as_mut().unwrap().ticking = true;
    g.sync_dc15();
    trigger(&mut g, false, STEP);
    for _ in 0..120 {
        trigger(&mut g, true, STEP);
    }
    assert_eq!(g.state.statistics.shots, 2);
    trigger(&mut g, false, STEP);
    trigger(&mut g, true, STEP);
    assert_eq!(g.state.statistics.shots, 3);
}
#[test]
fn dc15_semi_auto_frame_rates_and_subtick_release_checkpoint_are_exact() {
    for fps in [20, 60, 144] {
        let mut g = semi_auto_game();
        for _ in 0..fps {
            trigger(&mut g, true, 1. / f64::from(fps));
        }
        assert_eq!(g.state.statistics.shots, 1, "{fps} FPS");
    }
    let mut a = semi_auto_game();
    for _ in 0..30 {
        trigger(&mut a, true, STEP);
    }
    trigger(&mut a, false, STEP * 0.2);
    trigger(&mut a, true, STEP * 0.2);
    let save = a.checkpoint("semi-auto").unwrap();
    let mut b = semi_auto_game();
    b.restore_checkpoint(&save, "semi-auto").unwrap();
    for _ in 0..60 {
        trigger(&mut a, true, STEP);
        trigger(&mut b, true, STEP);
    }
    assert_eq!(a.state.statistics.shots, 2);
    assert_eq!(
        a.checkpoint("semi-auto").unwrap(),
        b.checkpoint("semi-auto").unwrap()
    );
    a.restart();
    trigger(&mut a, true, STEP);
    assert_eq!(a.state.statistics.shots, 1);
}
#[test]
fn dc15_semi_auto_handoff_retains_held_trigger_and_automatic_keeps_repeating() {
    let mut a = semi_auto_game();
    trigger(&mut a, true, STEP);
    let player = a.dc15_player(serde_json::json!({}));
    let mut b = semi_auto_game();
    let (energy, remaining) = b
        .dc15_handoff(&player, a.state.ammo, a.state.reserve)
        .unwrap();
    b.state.dc15 = energy;
    b.state.fire_remaining = remaining;
    b.sync_dc15();
    for _ in 0..60 {
        trigger(&mut b, true, STEP);
    }
    assert_eq!(b.state.statistics.shots, 0);
    trigger(&mut b, false, STEP);
    trigger(&mut b, true, STEP);
    assert_eq!(b.state.statistics.shots, 1);
    let mut auto = game(false);
    let mut profile = dc15_profile();
    profile.fire_mode = Some(dc15::FireMode::Automatic);
    auto.attach_dc15(profile).unwrap();
    for _ in 0..60 {
        trigger(&mut auto, true, STEP);
    }
    assert!(auto.state.statistics.shots >= 4);
}
#[test]
fn dc15_original_damage_energy_cadence_and_reload_input() {
    let mut g = game(false);
    g.attach_dc15(dc15_profile()).unwrap();
    g.state.enemies[0].position[2] = g.eye()[2];
    let health = g.state.enemies[0].health;
    g.advance(
        STEP,
        Input {
            fire: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(g.state.enemies[0].health, health - 55);
    assert_eq!((g.state.ammo, g.state.reserve), (4, 0));
    g.advance(
        STEP,
        Input {
            reload: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(g.state.statistics.reloads, 0);
    assert_eq!(g.state.reload_remaining, 0.);
    let mut last = 0;
    for _ in 0..600 {
        let before = g.state.statistics.shots;
        g.advance(
            STEP,
            Input {
                fire: true,
                reload: true,
                ..Default::default()
            },
        )
        .unwrap();
        if g.state.statistics.shots > before {
            assert!(g.state.statistics.ticks - last >= 12);
            last = g.state.statistics.ticks;
        }
        dc15::validate_state(g.dc15.as_ref(), &g.state).unwrap();
    }
    assert!((18..=21).contains(&g.state.statistics.shots));
    assert_eq!(g.state.reserve, 0);
    assert_eq!(g.state.statistics.reloads, 0);
}
#[test]
fn dc15_recharges_and_expires_shot_deadline_during_locked_opening() {
    let mut g = semi_auto_game();
    g.advance(
        STEP,
        Input {
            fire: true,
            ..Default::default()
        },
    )
    .unwrap();
    let charge = g.state.dc15.as_ref().unwrap().charge;
    let position = g.state.pawn.body.position;
    // Isolate the host's locked-opening early return from authored assets.
    g.state.opening = Some(
        serde_json::from_value(serde_json::json!({
            "intro":null,"rappel":null,"clock":0.,"lanes":[],"done":false,
            "locked":true,"injured":false,"gunship_idle":null,"captions":[],
            "subtitle":"","subtitle_until":0.,"prompt":"","prompt_until":0.,
            "blink_until":0.,"events":[],"ready_tick":null
        }))
        .unwrap(),
    );
    for _ in 0..60 {
        g.advance(
            STEP,
            Input {
                forward: 1.,
                fire: true,
                ..Default::default()
            },
        )
        .unwrap();
    }
    assert!(g.state.dc15.as_ref().unwrap().charge > charge);
    assert_eq!(g.state.dc15.as_ref().unwrap().charge, 5.);
    assert_eq!(g.state.fire_remaining, 0.);
    assert_eq!(g.state.pawn.body.position, position);
    assert_eq!(g.state.statistics.shots, 1);
    g.state.opening = None;
    trigger(&mut g, true, STEP);
    assert_eq!(
        g.state.statistics.shots, 1,
        "held opening press must not fire on unlock"
    );
    trigger(&mut g, false, STEP);
    trigger(&mut g, true, STEP);
    assert_eq!(g.state.statistics.shots, 2);
}
#[test]
fn dc15_checkpoint_fractional_input_continuation_and_restart_are_exact() {
    let mut a = game(false);
    a.attach_dc15(dc15_profile()).unwrap();
    for _ in 0..38 {
        a.advance(
            STEP,
            Input {
                fire: true,
                ..Default::default()
            },
        )
        .unwrap();
    }
    a.advance(
        STEP * 0.4,
        Input {
            fire: true,
            ..Default::default()
        },
    )
    .unwrap();
    let save = a.checkpoint("dc15-test").unwrap();
    let mut b = game(false);
    b.attach_dc15(dc15_profile()).unwrap();
    b.restore_checkpoint(&save, "dc15-test").unwrap();
    assert_eq!(save, b.checkpoint("dc15-test").unwrap());
    for _ in 0..180 {
        let input = Input {
            fire: true,
            ..Default::default()
        };
        a.advance(STEP, input).unwrap();
        b.advance(STEP, input).unwrap();
    }
    assert_eq!(
        a.checkpoint("dc15-test").unwrap(),
        b.checkpoint("dc15-test").unwrap()
    );
    a.restart();
    assert_eq!(a.state.dc15, Some(dc15_profile().initial()));
    assert_eq!((a.state.ammo, a.state.reserve), (5, 0));
}
#[test]
fn dc15_bad_checkpoint_states_are_rechecksummed_and_rejected_atomically() {
    let mut g = game(false);
    g.attach_dc15(dc15_profile()).unwrap();
    g.advance(
        STEP,
        Input {
            fire: true,
            ..Default::default()
        },
    )
    .unwrap();
    let save = g.checkpoint("dc15-test").unwrap();
    let value: serde_json::Value = serde_json::from_slice(&save[20..]).unwrap();
    for path in ["missing", "charge", "timer", "ammo", "reserve", "sleep"] {
        let mut v = value.clone();
        match path {
            "missing" => {
                v["state"].as_object_mut().unwrap().remove("dc15");
            }
            "charge" => v["state"]["dc15"]["charge"] = 6.into(),
            "timer" => v["state"]["dc15"]["cooldown_started"] = 999.into(),
            "ammo" => v["state"]["ammo"] = 0.into(),
            "reserve" => v["state"]["reserve"] = 1.into(),
            _ => v["state"]["dc15"]["ticking"] = false.into(),
        }
        let body = serde_json::to_vec(&v).unwrap();
        let hash = body.iter().fold(0xcbf29ce484222325_u64, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
        });
        let mut broken = b"RCSAVE01".to_vec();
        broken.extend((body.len() as u32).to_le_bytes());
        broken.extend(hash.to_le_bytes());
        broken.extend(body);
        assert!(
            g.restore_checkpoint(&broken, "dc15-test").is_err(),
            "{path}"
        );
        assert_eq!(save, g.checkpoint("dc15-test").unwrap());
    }
}
#[test]
fn dc15_handoff_preserves_fractional_charge_and_shot_timer_and_accepts_legacy() {
    let mut a = game(false);
    a.attach_dc15(dc15_profile()).unwrap();
    for _ in 0..16 {
        a.advance(
            STEP,
            Input {
                fire: true,
                ..Default::default()
            },
        )
        .unwrap();
    }
    let receipt = a.dc15_player(serde_json::json!({}));
    let (s, remaining) = a
        .dc15_handoff(&receipt, a.state.ammo, a.state.reserve)
        .unwrap();
    assert_eq!(s, a.state.dc15);
    assert_eq!(remaining, a.state.fire_remaining);
    assert!(game(false).dc15_handoff(&receipt, a.state.ammo, 0).is_err());
    let (legacy, remaining) = a.dc15_handoff(&serde_json::json!({}), 3, 200).unwrap();
    assert_eq!(legacy.unwrap().charge, 3.);
    assert_eq!(remaining, 0.);
    assert!(a.dc15_handoff(&receipt, 0, 0).is_err());
}
#[test]
fn stable_checkpoint_rejects_combat_falling_low_health_and_projectiles() {
    let mut g = game(false);
    g.state.enemies.clear();
    g.state.pawn.body.grounded = true;
    assert!(g.checkpoint_safe());
    g.state.health = 49;
    assert!(!g.checkpoint_safe());
    g.state.health = 100;
    g.state.pawn.body.grounded = false;
    assert!(!g.checkpoint_safe());
    g.state.pawn.body.grounded = true;
    g.state.damage_flash = 0.2;
    assert!(!g.checkpoint_safe());
    g.state.damage_flash = 0.;
    let hostile = game(false).state.enemies[0].clone();
    g.state.enemies.push(hostile);
    g.state.enemies[0].position = g.state.pawn.body.position;
    assert!(!g.checkpoint_safe());
    g.state.enemies[0].health = 0;
    assert!(g.checkpoint_safe());
    g.state.combat = Some(combat::State {
        projectiles: vec![combat::Projectile {
            velocity: None,
            position: [0.; 3],
            direction: [1., 0., 0.],
            speed: 100.,
            age: 0.,
        }],
        ..Default::default()
    });
    assert!(!g.checkpoint_safe());
    g.state.phase = Phase::Lost;
    assert!(!g.checkpoint_safe());
}
#[test]
fn legacy_scene_throw_input_is_inert_and_legacy_replay_shape_is_preserved() {
    let mut a = game(false);
    let mut b = game(false);
    let input = Input::default();
    let json = serde_json::to_value(input).unwrap();
    assert!(json.get("throw_grenade").is_none());
    let restored: Input = serde_json::from_value(json).unwrap();
    assert!(!restored.throw_grenade);
    for _ in 0..120 {
        a.advance(STEP, input).unwrap();
        b.advance(
            STEP,
            Input {
                throw_grenade: true,
                ..input
            },
        )
        .unwrap();
    }
    assert_eq!(
        serde_json::to_vec(&a.state).unwrap(),
        serde_json::to_vec(&b.state).unwrap()
    );
    assert!(serde_json::to_value(&a.state)
        .unwrap()
        .get("thermal")
        .is_none());
}

#[test]
fn throw_wrapper_rejects_truncated_oversized_and_recursive_metadata() {
    for size in [0, 1, 8 * 1024 * 1024 + 1, u32::MAX] {
        let mut bytes = b"RCPLAY05".to_vec();
        bytes.extend_from_slice(&size.to_le_bytes());
        bytes.extend_from_slice(b"RCPLAY05");
        assert!(bundle::decode(&bytes).is_err());
    }
}
#[test]
fn gunner_wrapper_rejects_truncated_oversized_and_recursive_metadata() {
    for size in [0, 1, 16 * 1024 * 1024 + 1, u32::MAX] {
        let mut raw = b"RCPLAY06".to_vec();
        raw.extend(size.to_le_bytes());
        raw.extend(b"RCPLAY06");
        assert!(bundle::decode(&raw).is_err());
    }
}
#[test]
fn demolition_wrapper_rejects_truncated_oversized_and_recursive_metadata() {
    for size in [0, 1, 32 * 1024 * 1024 + 1, u32::MAX] {
        let mut raw = b"RCPLAY08".to_vec();
        raw.extend(size.to_le_bytes());
        raw.extend(b"RCPLAY08");
        assert!(bundle::decode(&raw).is_err());
    }
    for size in 8..16 {
        assert!(bundle::decode(&b"RCPLAY08xxxxxxxx"[..size]).is_err());
    }
}
#[test]
fn artillery_wrapper_rejects_truncated_oversized_and_recursive_metadata() {
    for size in [0, 1, 16 * 1024 * 1024 + 1, u32::MAX] {
        let mut raw = b"RCPLAY10".to_vec();
        raw.extend(size.to_le_bytes());
        raw.extend(b"RCPLAY10");
        assert!(bundle::decode(&raw).is_err());
    }
    for n in 8..16 {
        assert!(bundle::decode(&b"RCPLAY10xxxxxxxx"[..n]).is_err());
    }
}
#[test]
fn spire_wrapper_rejects_truncated_oversized_and_recursive_metadata() {
    for size in [0, 1, 16 * 1024 * 1024 + 1, u32::MAX] {
        let mut raw = b"RCPLAY09".to_vec();
        raw.extend(size.to_le_bytes());
        raw.extend(b"RCPLAY09");
        assert!(bundle::decode(&raw).is_err());
    }
    for size in 8..16 {
        assert!(bundle::decode(&b"RCPLAY09xxxxxxxx"[..size]).is_err());
    }
}
#[test]
fn dynamic_extension_appends_resets_and_rejects_duplicates_atomically() {
    let mut g = game(false);
    g.attach_dynamic(vec![dynamic_wall(617)]).unwrap();
    g.append_dynamic(vec![dynamic_wall(850), dynamic_wall(1093)])
        .unwrap();
    assert_eq!(
        g.dynamic
            .as_ref()
            .unwrap()
            .objects
            .iter()
            .map(|o| o.original)
            .collect::<Vec<_>>(),
        [617, 850, 1093]
    );
    let before = bytes(&g);
    let textures = g.scene.textures.len();
    let actors = g.world.actors.len();
    assert!(g
        .append_dynamic(vec![dynamic_wall(777), dynamic_wall(850)])
        .is_err());
    assert_eq!(bytes(&g), before);
    assert_eq!(g.world.actors.len(), actors);
    assert_eq!(g.scene.textures.len(), textures);
    g.disable_dynamic(1093).unwrap();
    assert_eq!(g.state.dynamic.as_ref().unwrap(), &[false, false, true]);
    g.restart();
    assert_eq!(bytes(&g), before);
}
#[test]
fn grenade_radius_includes_instigator_respects_cover_and_skips_dead_victims() {
    let mut g = game(false);
    let before = bytes(&g);
    let hits = g.blast_damage([0., 0., 20.], 400., 600.).unwrap();
    assert!(hits.iter().any(|d| d.victim == -1 && d.amount == 400));
    assert!(hits
        .iter()
        .any(|d| d.victim == 0 && d.amount > 0 && d.amount < 400));
    assert_eq!(bytes(&g), before);
    g.attach_dynamic(vec![dynamic_wall(850)]).unwrap();
    assert!(!g
        .blast_damage([0., 0., 20.], 400., 600.)
        .unwrap()
        .iter()
        .any(|d| d.victim == 0));
    g.disable_dynamic(850).unwrap();
    g.state.enemies[0].health = 0;
    assert!(!g
        .blast_damage([0., 0., 20.], 400., 600.)
        .unwrap()
        .iter()
        .any(|d| d.victim == 0));
    assert!(g
        .blast_damage([-1000., 0., 20.], 400., 600.)
        .unwrap()
        .is_empty());
}
#[test]
fn transparent_frame_preserves_effect_order_color_and_weapon_pass() {
    use rc_render::{
        effects::{Blend, TriangleEffect},
        FaceTexture,
    };
    let game = game(false);
    let mut frame = game.render_frame(160, 100).unwrap();
    frame.hud.clear();
    frame.triangles = vec![
        Triangle {
            points: [[0.; 3]; 3],
            surface: 0
        };
        2
    ];
    frame.weapon_start = 1;
    let before = replay::state_fingerprint(&game).unwrap();
    for (texture, blend) in [
        (0, Blend::Alpha),
        (1, Blend::SourceAlphaAdditive),
        (0, Blend::Alpha),
    ] {
        frame.effects.push(TriangleEffect {
            triangle: Triangle {
                points: [[1., 2., 3.]; 3],
                surface: 0,
            },
            face: FaceTexture {
                texture,
                uv: [[0.25, 0.75]; 3],
            },
            color: [0.2, 0.4, 0.6, 0.3],
            blend,
            depth_test: true,
            alpha_reference: None,
            atlas_blend: None,
        });
    }
    let packet = frame.gpu_packet(&game.scene);
    assert_eq!(&packet[..8], b"RCFRA003");
    let word = |i| u32::from_le_bytes(packet[i..i + 4].try_into().unwrap());
    assert_eq!(word(64), 5);
    assert_eq!(word(68), 0);
    let mut offset = 72;
    for (pass, texture, blend, vertices) in [
        (0, -1, 0, 3),
        (1, 0, 1, 3),
        (1, 1, 2, 3),
        (1, 0, 1, 3),
        (2, -1, 0, 3),
    ] {
        assert_eq!(word(offset), pass);
        assert_eq!(word(offset + 4) as i32, texture);
        assert_eq!(word(offset + 8), blend);
        assert_eq!(word(offset + 12), vertices);
        if pass == 1 {
            for (i, value) in [1_f32, 2., 3., 0.25, 0.75, 0.2, 0.4, 0.6, 0.3]
                .into_iter()
                .enumerate()
            {
                assert_eq!(word(offset + 16 + i * 4), value.to_bits());
            }
        }
        offset += 16 + vertices as usize * 36;
    }
    assert_eq!(offset, packet.len());
    assert_eq!(before, replay::state_fingerprint(&game).unwrap());
    frame.effects.clear();
    frame.appearance = Some(vec![None; 2]);
    assert_eq!(&frame.gpu_packet(&game.scene)[..8], b"RCFRA002");
}
#[test]
fn presentation_snapshot_survives_simulation_and_rejects_bad_dimensions() {
    let mut game = game(false);
    let before = replay::state_fingerprint(&game).unwrap();
    let frame = game.render_frame(160, 100).unwrap();
    let pixels = frame.render_cpu(&game.scene).unwrap();
    let packet = frame.gpu_packet(&game.scene);
    assert_eq!(&packet[..8], b"RCFRA001");
    assert_eq!(pixels, game.render(160, 100).unwrap());
    assert!(game.render_frame(0, 100).is_err());
    assert!(game.render_frame(2049, 100).is_err());
    assert_eq!(before, replay::state_fingerprint(&game).unwrap());
    game.advance(
        STEP,
        Input {
            look: [0.3, 0.1],
            fire: true,
            ..Input::default()
        },
    )
    .unwrap();
    assert_ne!(before, replay::state_fingerprint(&game).unwrap());
    assert_eq!(packet, frame.gpu_packet(&game.scene));
    assert_eq!(pixels, frame.render_cpu(&game.scene).unwrap());
    assert_ne!(
        packet,
        game.render_frame(160, 100).unwrap().gpu_packet(&game.scene)
    );
    game.restart();
    assert_eq!(
        packet,
        game.render_frame(160, 100).unwrap().gpu_packet(&game.scene)
    );
}
pub(super) fn game(wall: bool) -> Game {
    let mut triangles = vec![Triangle {
        points: [
            [-2000., -2000., -84.5],
            [2000., -2000., -84.5],
            [0., 2000., -84.5],
        ],
        surface: 0,
    }];
    if wall {
        triangles.extend([
            Triangle {
                points: [
                    [200., -1000., -84.5],
                    [200., 1000., -84.5],
                    [200., 1000., 1000.],
                ],
                surface: 1,
            },
            Triangle {
                points: [
                    [200., -1000., -84.5],
                    [200., 1000., 1000.],
                    [200., -1000., 1000.],
                ],
                surface: 1,
            },
        ]);
    }
    let scene = rc_render::Scene::new(triangles).unwrap();
    let mut hulls = vec![Hull {
        offset: 0,
        planes: vec![],
        bounds: [[-10000., -10000., -1000.], [10000., 10000., -84.5]],
    }];
    if wall {
        hulls.push(Hull {
            offset: 1,
            planes: vec![],
            bounds: [[200., -1000., -84.5], [210., 1000., 1000.]],
        });
    }
    let world = StaticBodyWorld {
        world: Ok(Arc::new(HullSet {
            hulls,
            missing_solid_leaves: 0,
            empty_root_solid: false,
        })),
        actors: vec![],
    };
    let volumes = VolumeWorld {
        default: VolumeSettings {
            gravity: [0., 0., -1100.],
            terminal_speed: 12000.,
            ground_friction: 8.,
            water: false,
            zone_velocity: [0.; 3],
        },
        volumes: vec![],
    };
    let profile = MovementProfile {
        controller: ControllerOptions {
            speed: 450.,
            acceleration: 1024.,
            braking: 600.,
            air_control: 0.35,
            jump_speed: 475.,
        },
        ratios: MovementRatios {
            walk: 0.5,
            back: 0.8,
            side: 0.95,
        },
        physics: PhysicsOptions {
            extent: [40., 40., 84.],
            gravity: 1100.,
            terminal_speed: 12000.,
            skin: 0.5,
            support_distance: 2.,
            minimum_up: 0.7,
            max_iterations: 8,
            step_height: 35.,
        },
        properties: BTreeMap::new(),
    };
    let mut game = Game::new(scene, world, volumes, profile, [0.; 3], 0.).unwrap();
    game.set_encounter(&[[400., 0., -24.]], [800., 0., 0.])
        .unwrap();
    game
}
fn bytes(g: &Game) -> Vec<u8> {
    serde_json::to_vec(&g.state).unwrap()
}
fn dynamic_wall(original: u32) -> dynamic::Object {
    let mesh = rc_render::Scene::new(vec![Triangle {
        points: [[200., -300., -84.], [200., 300., -84.], [200., 0., 300.]],
        surface: 0,
    }])
    .unwrap();
    dynamic::Object {
        released: None,
        original,
        collider: bundle::Collider {
            name: "removable wall".into(),
            hulls: vec![bundle::HullData {
                offset: 0,
                planes: vec![],
                bounds: [[200., -300., -84.], [210., 300., 300.]],
            }],
        },
        snapshot: mesh.snapshot().unwrap(),
    }
}
#[test]
fn removable_actor_collision_frame_and_restart_agree() {
    let mut g = game(false);
    g.attach_dynamic(vec![dynamic_wall(617)]).unwrap();
    let initial = bytes(&g);
    let packet = rc_render::gpu::world(&g.scene).unwrap();
    let closed = g.render_frame(160, 100).unwrap();
    let closed_packet = closed.gpu_packet(&g.scene);
    let closed_pixels = closed.render_cpu(&g.scene).unwrap();
    assert!(g.wall_fraction([0., 0., 20.], [400., 0., 20.]).unwrap() < 1.);
    g.disable_dynamic(617).unwrap();
    assert_eq!(g.wall_fraction([0., 0., 20.], [400., 0., 20.]).unwrap(), 1.);
    assert_eq!(
        g.render_frame(160, 100).unwrap().triangles.len() + 1,
        closed.triangles.len()
    );
    assert_ne!(g.render(160, 100).unwrap(), closed_pixels);
    assert_eq!(rc_render::gpu::world(&g.scene).unwrap(), packet);
    assert_eq!(closed.render_cpu(&g.scene).unwrap(), closed_pixels);
    g.restart();
    assert_eq!(bytes(&g), initial);
    assert_eq!(
        g.render_frame(160, 100).unwrap().gpu_packet(&g.scene),
        closed_packet
    );
    assert_eq!(g.render(160, 100).unwrap(), closed_pixels);
    assert!(g.wall_fraction([0., 0., 20.], [400., 0., 20.]).unwrap() < 1.);
}
#[test]
fn removable_actor_decode_failure_is_atomic() {
    let mut g = game(false);
    let before = bytes(&g);
    let packet = rc_render::gpu::world(&g.scene).unwrap();
    let mut bad = dynamic_wall(915);
    bad.snapshot.truncate(20);
    assert!(g.attach_dynamic(vec![dynamic_wall(617), bad]).is_err());
    assert_eq!(bytes(&g), before);
    assert!(g.world.actors.is_empty());
    assert!(g.dynamic.is_none());
    assert_eq!(rc_render::gpu::world(&g.scene).unwrap(), packet);
    assert!(g
        .attach_dynamic(vec![dynamic_wall(617), dynamic_wall(617)])
        .is_err());
    assert_eq!(bytes(&g), before);
    assert!(g.world.actors.is_empty());
    g.attach_dynamic(vec![dynamic_wall(617)]).unwrap();
    let before = bytes(&g);
    assert!(g.disable_dynamic(915).is_err());
    assert_eq!(bytes(&g), before);
    assert!(g.attach_dynamic(vec![dynamic_wall(915)]).is_err());
    assert_eq!(bytes(&g), before);
}
fn combat_fixture(wall: bool) -> Game {
    let mut g = game(wall);
    g.combat = Some(combat::Assets {
        jump: None,
        navigation: None,
        game_rules: None,
        bot: None,
        lunge: None,
        ballistics: None,
        melee: None,
        provenance: serde_json::Value::Null,
        magazine: 60,
        total_ammo: 300,
        shot_delay: 0.10000000149011612,
        damage: 20,
        enemy_health: 100,
        speed: 6000.,
        acceleration: 60000.,
        max_speed: 10000.,
        laser_bolt: true,
        lifespan: 10.,
        reload_duration: 44. / 30.,
        sounds: vec![],
        fire_sound: 0,
        notifies: vec![],
    });
    g.state.combat = Some(combat::State::default());
    g.state.ammo = 60;
    g.state.reserve = 240;
    g.state.enemies[0].health = 100;
    g.initial = g.state.clone();
    g
}
fn mission_audio_assets() -> mission::Assets {
    let mut wave = b"RIFF".to_vec();
    wave.extend(38u32.to_le_bytes());
    wave.extend(b"WAVEfmt ");
    wave.extend(16u32.to_le_bytes());
    wave.extend(1u16.to_le_bytes());
    wave.extend(1u16.to_le_bytes());
    wave.extend(22050u32.to_le_bytes());
    wave.extend(44100u32.to_le_bytes());
    wave.extend(2u16.to_le_bytes());
    wave.extend(16u16.to_le_bytes());
    wave.extend(b"data");
    wave.extend(2u32.to_le_bytes());
    wave.extend(0i16.to_le_bytes());
    mission::Assets {
        enemy_fire_sound: None,
        weapon_fire_sound: Some(3),
        grenades: None,
        crash: None,
        provenance: serde_json::Value::Null,
        objective: "Bacta".into(),
        route: vec![[0., 0., 0.]],
        station: [100., 0., 0.],
        use_radius: 100.,
        max_health: 100.,
        start_health: 25.,
        bacta_rate: 3.,
        sounds: (0..4)
            .map(|i| combat::Sound {
                source: format!("sample{i}"),
                wave: wave.clone(),
            })
            .collect(),
    }
}
#[test]
fn foley_campaign_caps_all_producers_after_tick_and_preserves_legacy_queue() {
    for foley in [false, true] {
        let mut g = game(false);
        let mission = mission_audio_assets();
        let sample = mission.sounds[0].clone();
        g.attach_mission(mission).unwrap();
        let mut profile = dc15_profile();
        if foley {
            profile.animation = Some(weapon_animation::Assets {
                fire_anim: "Fire".into(),
                idle_delay: 20.,
                foley: Some(weapon_animation::Foley {
                    params: vec![],
                    groups: vec![],
                    sounds: vec![sample],
                    notifies: vec![],
                }),
            });
        }
        g.attach_dc15(profile).unwrap();
        let m = g.state.mission.as_mut().unwrap();
        m.audio_sequence = 270;
        m.events = (1..=270)
            .map(|sequence| combat::Event {
                sequence,
                tick: 0,
                sound: 0,
                owner: -1,
            })
            .collect();
        // Mission tick adds a cue after normal weapon processing. The final
        // step must bound that producer too, rather than only the Foley emitter.
        g.advance(STEP, Input::default()).unwrap();
        let m = g.state.mission.as_ref().unwrap();
        assert_eq!(m.events.last().unwrap().sequence, 271);
        assert_eq!(m.events.len(), if foley { 256 } else { 271 });
        assert_eq!(m.events[0].sequence, if foley { 16 } else { 1 });
    }
}
#[test]
fn accepted_enemy_shot_has_one_damage_trace_and_original_sound_binding() {
    let mut g = game(false);
    let mut assets = mission_audio_assets();
    let cue = assets.sounds[0].wave.clone();
    assets.enemy_fire_sound = Some(4);
    assets.sounds.push(combat::Sound {
        source: "original-enemy-cue".into(),
        wave: cue,
    });
    g.attach_mission(assets).unwrap();
    g.set_encounter(&[[400., 0., -24.]], [800., 0., 0.])
        .unwrap();
    g.state.enemies[0].cooldown = 0.;
    let hp = g.state.health;
    g.enemy_tick().unwrap();
    assert_eq!(g.state.health, hp - 5);
    assert_eq!(g.state.statistics.enemy_shots, 1);
    assert_eq!(g.state.enemy_traces.len(), 1);
    assert!(g.state.enemy_traces[0].ranged);
    let events = &g.state.mission.as_ref().unwrap().events;
    assert_eq!(
        events
            .iter()
            .filter(|e| e.sound == 4 && e.owner == 0)
            .count(),
        1
    );
    assert_eq!(events.last().unwrap().tick, g.state.enemy_traces[0].tick);
    g.enemy_tick().unwrap();
    assert_eq!(g.state.statistics.enemy_shots, 1);
    g.suspend();
    assert!(g.state.enemy_traces.is_empty());
    assert!(g.state.mission.as_ref().unwrap().events.is_empty());
    g.state.enemies[0].health = 0;
    g.state.enemies[0].cooldown = 0.;
    g.enemy_tick().unwrap();
    assert_eq!(g.state.statistics.enemy_shots, 1);
    g.state.enemies[0].health = 100;
    g.state.phase = Phase::Lost;
    assert!(!g
        .state
        .accept_enemy_attack(0, [0.; 3], [1.; 3], 5, Some(4), true));
    assert!(g.state.enemy_traces.is_empty());
}
#[test]
fn enemy_cover_prevents_damage_trace_and_audio_together() {
    let mut g = game(true);
    g.attach_mission(mission_audio_assets()).unwrap();
    g.set_encounter(&[[400., 0., -24.]], [800., 0., 0.])
        .unwrap();
    g.state.enemies[0].cooldown = 0.;
    let health = g.state.health;
    g.enemy_tick().unwrap();
    assert_eq!(g.state.health, health);
    assert_eq!(g.state.statistics.enemy_shots, 0);
    assert!(g.state.enemy_traces.is_empty());
    assert!(g.state.mission.as_ref().unwrap().events.is_empty());
}
#[test]
fn mission_weapon_audio_follows_accepted_shots_and_restart() {
    let mut g = game(false);
    g.attach_mission(mission_audio_assets()).unwrap();
    let fire = Input {
        fire: true,
        ..Input::default()
    };
    g.advance(STEP, fire).unwrap();
    assert_eq!(g.state.statistics.shots, 1);
    assert_eq!(
        g.state
            .mission
            .as_ref()
            .unwrap()
            .events
            .iter()
            .filter(|e| e.sound == 3)
            .count(),
        1
    );
    g.advance(STEP, fire).unwrap();
    assert_eq!(g.state.statistics.shots, 1);
    g.state.reload_remaining = 1.;
    g.advance(0.2, fire).unwrap();
    assert_eq!(g.state.statistics.shots, 1);
    g.suspend();
    assert!(g.state.mission.as_ref().unwrap().events.is_empty());
    g.state.ammo = 999;
    for _ in 0..300 {
        g.fire().unwrap();
    }
    let events = &g.state.mission.as_ref().unwrap().events;
    assert_eq!(events.len(), 256);
    assert!(events.windows(2).all(|v| v[0].sequence < v[1].sequence));
    g.restart();
    assert_eq!(g.state.statistics.shots, 0);
    assert!(g.state.mission.as_ref().unwrap().events.is_empty());
    assert_eq!(g.state.mission.as_ref().unwrap().audio_sequence, 0);
}
#[test]
fn invalid_mission_weapon_sound_slot_is_rejected_without_mutation() {
    let mut g = game(false);
    let before = bytes(&g);
    let mut assets = mission_audio_assets();
    assets.weapon_fire_sound = Some(0);
    assert!(g.attach_mission(assets).is_err());
    assert_eq!(bytes(&g), before);
    assert!(g.mission.is_none());
}
#[test]
fn projectile_travels_then_hits_and_wall_blocks_its_swept_path() {
    for wall in [false, true] {
        let mut g = combat_fixture(wall);
        g.aim_at(g.state.enemies[0].position);
        g.advance(
            STEP,
            Input {
                fire: true,
                ..Input::default()
            },
        )
        .unwrap();
        assert_eq!(g.state.enemies[0].health, 100);
        assert_eq!(g.state.combat.as_ref().unwrap().projectiles.len(), 1);
        for _ in 0..30 {
            g.advance(STEP, Input::default()).unwrap();
        }
        assert_eq!(g.state.enemies[0].health, if wall { 100 } else { 80 });
        assert!(g.state.combat.as_ref().unwrap().projectiles.is_empty());
        assert_eq!(g.state.statistics.hits, u32::from(!wall));
        assert_eq!(g.state.combat.as_ref().unwrap().events.len(), 1);
        g.restart();
        assert!(g.state.combat.as_ref().unwrap().events.is_empty());
    }
}
#[test]
fn original_magazine_fire_cadence_and_reload_conserve_total_ammo() {
    let mut g = combat_fixture(false);
    g.state.yaw = std::f64::consts::PI;
    for _ in 0..60 {
        g.advance(
            STEP,
            Input {
                fire: true,
                ..Input::default()
            },
        )
        .unwrap();
    }
    assert_eq!(g.state.statistics.shots, 10);
    assert_eq!(g.state.ammo, 50);
    g.advance(
        STEP,
        Input {
            reload: true,
            ..Input::default()
        },
    )
    .unwrap();
    for _ in 0..100 {
        g.advance(STEP, Input::default()).unwrap();
    }
    assert_eq!((g.state.ammo, g.state.reserve), (60, 230));
    let before = bytes(&g);
    assert!(g.advance(0.5, Input::default()).is_err());
    assert_eq!(before, bytes(&g));
}
#[test]
fn animation_notify_intervals_are_once_across_entry_wrap_and_end() {
    use visual::crossed_notify as crossing;
    assert!(crossing(-f32::EPSILON, 0.1, 0.));
    assert!(!crossing(0., 0.1, 0.));
    assert!(crossing(0.9, 1., 0.));
    assert!(!crossing(0., 0.1, 0.)); // following wrapped frame must not dispatch zero again
    assert!(crossing(0.1, 0.2, 0.2));
    assert!(!crossing(0.2, 0.3, 0.2));
    assert!(crossing(0.9, 1.1, 0.05));
    assert!(!crossing(0.95, 0.95, 0.95));
}
#[test]
fn empty_original_magazine_reloads_after_fire_is_released() {
    let mut g = combat_fixture(false);
    g.state.yaw = std::f64::consts::PI;
    for _ in 0..360 {
        g.advance(
            STEP,
            Input {
                fire: true,
                ..Input::default()
            },
        )
        .unwrap();
    }
    assert_eq!(g.state.statistics.shots, 60);
    assert!(g.state.reload_remaining > 0.);
    for _ in 0..100 {
        g.advance(STEP, Input::default()).unwrap();
    }
    assert_eq!((g.state.ammo, g.state.reserve), (60, 180));
    assert_eq!(g.state.statistics.reloads, 1);
}
#[test]
fn original_wave_validation_rejects_truncation_format_and_length_mismatches() {
    let mut b = b"RIFF".to_vec();
    b.extend_from_slice(&38u32.to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&[1, 0, 1, 0]);
    b.extend_from_slice(&22050u32.to_le_bytes());
    b.extend_from_slice(&44100u32.to_le_bytes());
    b.extend_from_slice(&[2, 0, 16, 0]);
    b.extend_from_slice(b"data");
    b.extend_from_slice(&2u32.to_le_bytes());
    b.extend_from_slice(&[1, 2]);
    assert!(combat::validate_wave(&b).is_ok());
    for len in 0..b.len() {
        assert!(combat::validate_wave(&b[..len]).is_err());
    }
    b[20] = 17;
    assert!(combat::validate_wave(&b).is_err());
    b[20] = 1;
    b[40] = 3;
    assert!(combat::validate_wave(&b).is_err());
}
#[test]
fn fixed_ticks_are_independent_of_render_rate_and_buffer_a_short_jump() {
    let mut a = game(false);
    let mut b = game(false);
    let input = Input {
        forward: 1.,
        ..Input::default()
    };
    for _ in 0..60 {
        a.advance(STEP, input).unwrap();
        b.advance(STEP / 2., input).unwrap();
        b.advance(STEP / 2., input).unwrap();
    }
    assert_eq!(bytes(&a), bytes(&b));
    let mut g = game(false);
    g.advance(
        STEP / 2.,
        Input {
            jump: true,
            ..Input::default()
        },
    )
    .unwrap();
    g.advance(STEP / 2., Input::default()).unwrap();
    assert_eq!(g.state.statistics.jumps, 1);
    for _ in 0..90 {
        g.advance(
            STEP,
            Input {
                jump: true,
                ..Input::default()
            },
        )
        .unwrap();
    }
    assert_eq!(g.state.statistics.jumps, 1);
    assert!(g.state.pawn.body.grounded);
}
#[test]
fn body_cannot_cross_wall_and_wall_occludes_both_combatants() {
    let mut g = game(true);
    g.aim_at(g.state.enemies[0].position);
    for _ in 0..180 {
        g.advance(
            STEP,
            Input {
                forward: 1.,
                fire: true,
                ..Input::default()
            },
        )
        .unwrap();
    }
    assert!(
        g.state.pawn.body.position[0] <= 160.,
        "{:?}",
        g.state.pawn.body
    );
    assert_ne!(
        g.world
            .body_placement(g.state.pawn.body.position, g.profile.physics.extent)
            .unwrap()
            .state,
        PlacementState::Penetrating
    );
    assert_eq!(g.state.enemies[0].health, 60);
    assert_eq!(g.state.health, 100);
    assert_eq!(g.state.statistics.hits, 0);
    assert_eq!(g.state.statistics.enemy_shots, 0);
}
#[test]
fn nearest_enemy_hit_reload_win_and_restart_are_connected() {
    let mut g = game(false);
    let initial = bytes(&g);
    g.aim_at(g.state.enemies[0].position);
    for _ in 0..11 {
        g.advance(
            STEP,
            Input {
                fire: true,
                ..Input::default()
            },
        )
        .unwrap();
    }
    assert_eq!(g.state.statistics.kills, 1);
    assert_eq!(g.state.ammo, 18);
    g.advance(
        STEP,
        Input {
            reload: true,
            ..Input::default()
        },
    )
    .unwrap();
    for _ in 0..75 {
        g.advance(STEP, Input::default()).unwrap();
    }
    assert_eq!(g.state.ammo, MAGAZINE);
    assert_eq!(g.state.reserve, 198);
    g.aim_at([800., 0., g.eye()[2]]);
    for _ in 0..180 {
        g.advance(
            STEP,
            Input {
                forward: 1.,
                ..Input::default()
            },
        )
        .unwrap();
    }
    assert_eq!(g.state.phase, Phase::Won);
    let won = bytes(&g);
    assert_eq!(g.advance(0.1, Input::default()).unwrap(), 0);
    assert_eq!(bytes(&g), won);
    g.restart();
    assert_eq!(bytes(&g), initial);
}
#[test]
fn enemy_fire_causes_defeat_and_restart_restores_encounter() {
    let mut g = game(false);
    let initial = bytes(&g);
    for _ in 0..2000 {
        g.advance(STEP, Input::default()).unwrap();
    }
    assert_eq!(g.state.phase, Phase::Lost);
    assert_eq!(g.state.health, 0);
    assert_eq!(g.state.statistics.enemy_shots, 20);
    g.restart();
    assert_eq!(bytes(&g), initial);
}
#[test]
fn invalid_and_late_failed_frames_preserve_all_state_and_pending_time() {
    let mut g = game(false);
    g.advance(
        STEP / 2.,
        Input {
            jump: true,
            ..Input::default()
        },
    )
    .unwrap();
    let before = bytes(&g);
    for input in [
        Input {
            forward: f64::NAN,
            ..Input::default()
        },
        Input {
            look: [f64::INFINITY, 0.],
            ..Input::default()
        },
    ] {
        assert!(g.advance(STEP, input).is_err());
        assert_eq!(bytes(&g), before);
    }
    g.world.actors.push(BodyActor {
        name: "missing".into(),
        shape: BodyShape::Unsupported("fixture".into()),
    });
    assert!(g
        .advance(
            STEP,
            Input {
                look: [0.5, 0.2],
                fire: true,
                ..Input::default()
            }
        )
        .is_err());
    assert_eq!(bytes(&g), before);
    g.world.actors.clear();
    g.advance(STEP / 2., Input::default()).unwrap();
    assert_eq!(g.state.statistics.jumps, 1);
}
#[test]
fn pause_and_catchup_cap_do_not_replay_queued_input_or_long_background_time() {
    let mut g = game(false);
    g.advance(
        STEP / 2.,
        Input {
            jump: true,
            reload: true,
            ..Input::default()
        },
    )
    .unwrap();
    g.suspend();
    g.advance(STEP, Input::default()).unwrap();
    assert_eq!(g.state.statistics.jumps, 0);
    let ticks = g.advance(0.25, Input::default()).unwrap();
    assert_eq!(ticks, 8);
    assert!(g.state.statistics.discarded_seconds > 0.1);
    assert!(g.advance(1., Input::default()).is_err());
}
#[test]
fn a_fire_tap_shorter_than_a_fixed_tick_is_retained_once() {
    let mut g = game(false);
    g.aim_at(g.state.enemies[0].position);
    g.advance(
        STEP / 2.,
        Input {
            fire: true,
            ..Input::default()
        },
    )
    .unwrap();
    assert_eq!(g.state.statistics.shots, 0);
    g.advance(STEP / 2., Input::default()).unwrap();
    assert_eq!(g.state.statistics.shots, 1);
    for _ in 0..15 {
        g.advance(STEP, Input::default()).unwrap();
    }
    assert_eq!(g.state.statistics.shots, 1);
}
#[test]
fn bundle_decoder_roundtrip_fits_explicit_two_megabyte_stack() {
    // RUST_MIN_STACK must not be able to hide a regression in the decoder.
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(bundle_roundtrip_retains_collision_spawn_encounter_and_rejects_corruption)
        .unwrap()
        .join()
        .unwrap();
}
#[test]
fn bundle_roundtrip_retains_collision_spawn_encounter_and_rejects_corruption() {
    let g = game(true);
    let manifest = bundle::Manifest::capture(&g, serde_json::json!({"fixture":true})).unwrap();
    let packed = bundle::encode(&manifest, &g.scene).unwrap();
    let mut restored = bundle::decode(&packed).unwrap();
    assert_eq!(bytes(&g), bytes(&restored));
    for _ in 0..100 {
        restored
            .advance(
                STEP,
                Input {
                    forward: 1.,
                    ..Input::default()
                },
            )
            .unwrap();
    }
    assert!(restored.state.pawn.body.position[0] < 160.);
    for n in [0, 7, 11, 12, packed.len() - 1] {
        assert!(bundle::decode(&packed[..n]).is_err());
    }
    let mut bad = packed.clone();
    bad[0] = 0;
    assert!(bundle::decode(&bad).is_err());
    let mut bad = packed;
    bad[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(bundle::decode(&bad).is_err());
}
#[test]
fn look_axes_match_screen_strafe_and_transient_actors_render_with_world_depth() {
    let mut g = game(false);
    g.advance(
        STEP,
        Input {
            look: [0.2, -0.1],
            strafe: 1.,
            ..Input::default()
        },
    )
    .unwrap();
    assert!(g.state.yaw > 6.);
    assert!(g.state.pitch > 0.);
    assert!(g.state.pawn.body.position[1] < 0.);
    let frame = g.render(320, 200).unwrap();
    assert_eq!(frame.len(), 64000);
    assert!(g.render(0, 0).is_err());
    let mut hidden = game(true);
    let visible = hidden.render(320, 200).unwrap();
    hidden.state.enemies.clear();
    let absent = hidden.render(320, 200).unwrap();
    // World depth hides every enemy pixel. HUD objective changes only in its top strip;
    // the marker is behind the same wall as the target.
    assert!(visible[320 * 31..320 * 170] == absent[320 * 31..320 * 170]);
}

#[test]
fn input_replay_covers_pending_input_pause_restart_and_detects_changed_time() {
    use crate::replay::{Action, Replay};
    let mut original = game(false);
    let mut replay = Replay::new(b"test bundle");
    replay
        .record(
            &mut original,
            Action::Frame {
                elapsed: STEP / 4.,
                input: Input {
                    jump: true,
                    ..Input::default()
                },
            },
            true,
            "queued",
        )
        .unwrap();
    replay
        .record(&mut original, Action::Suspend, false, "pause")
        .unwrap();
    replay
        .record(
            &mut original,
            Action::Frame {
                elapsed: STEP,
                input: Input::default(),
            },
            true,
            "resume",
        )
        .unwrap();
    replay
        .record(&mut original, Action::Restart, true, "reset")
        .unwrap();
    let json = serde_json::to_vec(&replay).unwrap();
    let decoded: Replay = serde_json::from_slice(&json).unwrap();
    decoded.validate_bundle(b"test bundle").unwrap();
    assert!(decoded.validate_bundle(b"changed").is_err());
    let mut other = game(false);
    for entry in &decoded.entries {
        Replay::verify_entry(&mut other, entry, true).unwrap();
    }
    assert_eq!(
        crate::replay::state_fingerprint(&other).unwrap(),
        crate::replay::state_fingerprint(&original).unwrap()
    );
    let mut corrupt = decoded.entries[0].clone();
    corrupt.action = Action::Frame {
        elapsed: STEP / 3.,
        input: Input {
            jump: true,
            ..Input::default()
        },
    };
    assert!(Replay::verify_entry(&mut game(false), &corrupt, false).is_err());
    let mut version = decoded;
    version.version = 2;
    assert!(version.validate_bundle(b"test bundle").is_err());
}
#[test]
fn optional_profiling_preserves_simulation_and_pixels() {
    let mut control = game(true);
    let mut measured = game(true);
    let input = Input {
        forward: 1.,
        fire: true,
        ..Input::default()
    };
    control.advance(STEP, input).unwrap();
    let expected = control.render(64, 64).unwrap();
    rc_package::performance::begin().unwrap();
    measured.advance(STEP, input).unwrap();
    let actual = measured.render(64, 64).unwrap();
    let counters = rc_package::performance::finish().unwrap();
    assert_eq!(expected, actual);
    assert_eq!(
        crate::replay::state_fingerprint(&control).unwrap(),
        crate::replay::state_fingerprint(&measured).unwrap()
    );
    assert!(
        counters.calls[0] > 0
            && counters.calls[1] > 0
            && counters.calls[3] > 0
            && counters.calls[6] > 0
    );
    assert!(counters.ns[0] >= counters.ns[1]);
    assert!(counters.ns[3] >= counters.ns[4] + counters.ns[5]);
}

fn ballistics_fixture(wall: bool) -> Game {
    let mut g = combat_fixture(wall);
    g.combat.as_mut().unwrap().ballistics = Some(combat::Ballistics {
        provenance: serde_json::Value::Null,
        fire_offset: [127.4, 30., -10.],
        proximity_damage: 20,
    });
    g.initial = g.state.clone();
    g
}
#[test]
fn muzzle_cannot_spawn_beyond_near_wall_and_still_consumes_one_round() {
    let mut g = ballistics_fixture(true);
    g.state.pawn.body.position[0] = 100.;
    g.state.yaw = 0.;
    g.state.pitch = 0.;
    g.projectile_fire().unwrap();
    assert!(g.state.combat.as_ref().unwrap().projectiles.is_empty());
    assert_eq!(
        (
            g.state.ammo,
            g.state.statistics.shots,
            g.state.statistics.hits
        ),
        (59, 1, 0)
    );
    assert!(g.state.impact.is_some());
}
#[test]
fn proximity_actor_hit_uses_ammo_damage_without_spawning_projectile() {
    let mut g = ballistics_fixture(false);
    g.state.yaw = 0.;
    g.state.pitch = 0.;
    let eye = g.eye();
    g.state.enemies[0].position = [eye[0] + 65., eye[1] + 15., eye[2]];
    g.projectile_fire().unwrap();
    assert!(g.state.combat.as_ref().unwrap().projectiles.is_empty());
    assert_eq!(
        (
            g.state.enemies[0].health,
            g.state.statistics.hits,
            g.state.ammo
        ),
        (80, 1, 59)
    );
}
#[test]
fn offset_origin_and_reticle_correction_hit_before_wall() {
    let mut g = ballistics_fixture(false);
    g.aim_at(g.state.enemies[0].position);
    let eye = g.eye();
    g.projectile_fire().unwrap();
    let p = &g.state.combat.as_ref().unwrap().projectiles[0];
    assert_ne!(p.position, eye);
    assert_eq!(p.velocity, Some([0.; 3]));
    for _ in 0..30 {
        g.projectile_tick().unwrap();
    }
    assert_eq!(g.state.statistics.hits, 1);
    assert_eq!(g.state.enemies[0].health, 80);
}
#[test]
fn nonfinite_offset_rejected_before_scene_use() {
    let mut g = ballistics_fixture(false);
    g.combat
        .as_mut()
        .unwrap()
        .ballistics
        .as_mut()
        .unwrap()
        .fire_offset[0] = f32::NAN;
    assert!(g.combat.as_ref().unwrap().validate().is_err());
}

#[test]
fn released_actor_uses_resting_collision_geometry_and_resets() {
    let mut g = game(false);
    let mut wall = dynamic_wall(909);
    let mut mesh = rc_render::Scene::from_snapshot(&wall.snapshot).unwrap();
    for t in &mut mesh.triangles {
        for p in &mut t.points {
            p[0] += 200.;
        }
    }
    wall.released = Some(dynamic::Released {
        collider: bundle::Collider {
            name: "resting wall".into(),
            hulls: vec![bundle::HullData {
                offset: 0,
                planes: vec![],
                bounds: [[400., -300., -84.], [410., 300., 300.]],
            }],
        },
        snapshot: mesh.snapshot().unwrap(),
    });
    g.attach_dynamic(vec![wall]).unwrap();
    let before = bytes(&g);
    let closed = g.render_frame(160, 100).unwrap().gpu_packet(&g.scene);
    g.disable_dynamic(909).unwrap();
    assert_eq!(
        g.world
            .body_placement([205., 0., 0.], [1.; 3])
            .unwrap()
            .state,
        rc_package::movement::PlacementState::Free
    );
    assert_eq!(
        g.world
            .body_placement([405., 0., 0.], [1.; 3])
            .unwrap()
            .state,
        rc_package::movement::PlacementState::Penetrating
    );
    assert_ne!(
        g.render_frame(160, 100).unwrap().gpu_packet(&g.scene),
        closed
    );
    g.restart();
    assert_eq!(bytes(&g), before);
    assert_eq!(
        g.render_frame(160, 100).unwrap().gpu_packet(&g.scene),
        closed
    );
}
#[test]
fn invalid_released_actor_keeps_world_and_state_unchanged() {
    let mut g = game(false);
    let before = bytes(&g);
    let actors = g.world.actors.len();
    let mut wall = dynamic_wall(909);
    wall.released = Some(dynamic::Released {
        collider: bundle::Collider {
            name: "invalid".into(),
            hulls: vec![],
        },
        snapshot: vec![],
    });
    assert!(g.attach_dynamic(vec![wall]).is_err());
    assert_eq!(bytes(&g), before);
    assert_eq!(g.world.actors.len(), actors);
    assert!(g.dynamic.is_none());
}
#[test]
fn oversized_mission_metadata_rejected_before_parsing() {
    let mut raw = b"RCPLAY04".to_vec();
    raw.extend_from_slice(&(24u32 * 1024 * 1024 + 1).to_le_bytes());
    raw.extend_from_slice(&[0; 4]);
    assert_eq!(
        bundle::decode(&raw).err().unwrap(),
        "Mission metadata exceeds budget"
    );
}

#[test]
fn warrior_ai_bundle_rejects_oversized_and_truncated_layers() {
    let mut bytes = b"RCPLAY11".to_vec();
    bytes.extend((12_u32 * 1024 * 1024 + 1).to_le_bytes());
    bytes.extend([0; 4]);
    assert_eq!(
        bundle::decode(&bytes).err().unwrap(),
        "Warrior AI metadata exceeds budget"
    );
    bytes[8..12].copy_from_slice(&100_u32.to_le_bytes());
    assert_eq!(
        bundle::decode(&bytes).err().unwrap(),
        "Truncated Warrior AI metadata"
    );
    assert!(bundle::decode(b"RCPLAY11").is_err());
}

#[test]
fn squad_wrapper_rejects_truncated_oversized_and_recursive_layers() {
    for size in [0u32, 1, 24 * 1024 * 1024 + 1, u32::MAX] {
        let mut raw = b"RCPLAY12".to_vec();
        raw.extend(size.to_le_bytes());
        raw.extend(b"RCPLAY12");
        assert!(bundle::decode(&raw).is_err());
    }
    for size in 8..16 {
        assert!(bundle::decode(&b"RCPLAY12xxxxxxxx"[..size]).is_err());
    }
}

#[test]
fn opening_wrapper_rejects_truncated_oversized_and_recursive_layers() {
    for size in [0u32, 1, 24 * 1024 * 1024 + 1, u32::MAX] {
        let mut bytes = b"RCPLAY13".to_vec();
        bytes.extend(size.to_le_bytes());
        bytes.extend(b"RCPLAY13");
        assert!(bundle::decode(&bytes).is_err());
    }
}
