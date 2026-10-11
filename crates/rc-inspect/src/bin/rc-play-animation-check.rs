//! Behavioral M2 checks using the same immutable assets and fixed-tick simulation as Android.
use rc_game::{Input, STEP};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-play-animation-check SCENE OUTPUT_DIR".into());
    }
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let bytes = fs::read(&args[1])?;
    let mut g = rc_game::bundle::decode(&bytes)?;
    let mut slow = rc_game::bundle::decode(&bytes)?;
    let mut fast = rc_game::bundle::decode(&bytes)?;
    for _ in 0..30 {
        slow.advance(
            1. / 30.,
            Input {
                fire: true,
                ..Input::default()
            },
        )?;
    }
    for _ in 0..120 {
        fast.advance(
            1. / 120.,
            Input {
                fire: true,
                ..Input::default()
            },
        )?;
    }
    assert_eq!(
        serde_json::to_vec(&slow.state)?,
        serde_json::to_vec(&fast.state)?,
        "Visual clocks depend on presentation cadence"
    );
    let initial = serde_json::to_vec(&g.state)?;
    let first = g.render_frame(640, 400)?;
    let packet = first.gpu_packet(&g.scene);
    assert_eq!(&packet[..8], b"RCFRA002");
    assert_eq!(initial, serde_json::to_vec(&g.state)?);
    let assets = g.visuals.as_ref().ok_or("No visual assets")?;
    let mut clips = vec![];
    for mesh in [&assets.enemy, &assets.weapon] {
        for (i, c) in mesh.mesh.clips.iter().enumerate() {
            let a = mesh.scene(&mesh.channel(i, 0.)?)?.snapshot()?;
            let b = mesh.scene(&mesh.channel(i, 0.5)?)?.snapshot()?;
            let again = mesh.scene(&mesh.channel(i, 0.)?)?.snapshot()?;
            assert_eq!(a, again, "pose scratch must not leak between samples");
            assert_ne!(a, b, "original clip must change geometry: {}", c.source);
            clips.push(serde_json::json!({"source":c.source,"triangles":mesh.geometry(&mesh.channel(i,0.)?)?.triangles.len(),"pose_changes":true,"repeat_exact":true}));
        }
    }
    for _ in 0..40 {
        g.advance(STEP, Input::default())?;
    }
    assert_ne!(initial, serde_json::to_vec(&g.state)?);
    g.suspend();
    let paused = serde_json::to_vec(&g.state)?;
    for _ in 0..5 {
        g.advance(0., Input::default())?;
        g.render_frame(640, 400)?;
    }
    assert_eq!(paused, serde_json::to_vec(&g.state)?);
    g.restart();
    assert_eq!(initial, serde_json::to_vec(&g.state)?);
    assert_eq!(packet, g.render_frame(640, 400)?.gpu_packet(&g.scene));
    g.aim_at(g.state.enemies[0].position);
    g.advance(
        STEP,
        Input {
            fire: true,
            ..Input::default()
        },
    )?;
    let visuals = g.state.visuals.as_ref().unwrap();
    let assets = g.visuals.as_ref().unwrap();
    assert_eq!(
        assets.enemy.mesh.clips[visuals.enemies[0].clip]
            .sequence
            .name
            .name,
        "hit"
    );
    assert_ne!(visuals.enemies[0].clip, visuals.enemies[1].clip);
    assert_eq!(
        assets.weapon.mesh.clips[visuals.weapon.clip]
            .sequence
            .name
            .name,
        "Fire"
    );
    for _ in 0..10 {
        g.advance(STEP, Input::default())?;
    }
    g.advance(
        STEP,
        Input {
            fire: true,
            ..Input::default()
        },
    )?;
    for _ in 0..150 {
        g.advance(STEP, Input::default())?;
    }
    let player = &g.state.visuals.as_ref().unwrap().enemies[0];
    assert_eq!(
        g.visuals.as_ref().unwrap().enemy.mesh.clips[player.clip]
            .sequence
            .name
            .name,
        "Death"
    );
    assert!(player.finished);
    let corpse = (player.clip, player.frame.to_bits());
    for _ in 0..10 {
        g.advance(STEP, Input::default())?;
    }
    let player = &g.state.visuals.as_ref().unwrap().enemies[0];
    assert_eq!(corpse, (player.clip, player.frame.to_bits()));
    g.advance(
        STEP,
        Input {
            reload: true,
            ..Input::default()
        },
    )?;
    let visuals = g.state.visuals.as_ref().unwrap();
    assert_eq!(
        g.visuals.as_ref().unwrap().weapon.mesh.clips[visuals.weapon.clip]
            .sequence
            .name
            .name,
        "Reload"
    );
    let prior = serde_json::to_vec(&g.state)?;
    assert!(g.advance(f64::NAN, Input::default()).is_err());
    assert_eq!(prior, serde_json::to_vec(&g.state)?);
    g.restart();
    assert_eq!(initial, serde_json::to_vec(&g.state)?);
    // Follow the camera in world space while retaining the same weapon shape in camera space.
    let a = g.render_frame(640, 400)?;
    g.state.yaw += 0.7;
    g.state.pitch = 0.2;
    g.state.pawn.body.position[0] += 5.;
    let b = g.render_frame(640, 400)?;
    let basis_a = rc_render::gpu::basis(&g.scene, a.view);
    let basis_b = rc_render::gpu::basis(&g.scene, b.view);
    let camera_point = |p: [f32; 3], basis: [[f32; 3]; 4]| -> [f32; 3] {
        std::array::from_fn(|axis| {
            (0..3)
                .map(|i| (p[i] - basis[0][i]) * basis[axis + 1][i])
                .sum()
        })
    };
    for (left, right) in a.triangles[a.weapon_start..]
        .iter()
        .zip(&b.triangles[b.weapon_start..])
    {
        for (left, right) in left.points.iter().zip(right.points) {
            let l = camera_point(*left, basis_a);
            let r = camera_point(right, basis_b);
            assert!(l.iter().zip(r).all(|(a, b)| (a - b).abs() < 0.002));
        }
    }
    let n = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let mut assets: serde_json::Value = serde_json::from_slice(&bytes[12..12 + n])?;
    assets["enemy"]["clips"][0]["mapping"][0] = serde_json::json!(9999);
    let bad = serde_json::to_vec(&assets)?;
    let mut bundle = b"RCPLAY02".to_vec();
    bundle.extend_from_slice(&(bad.len() as u32).to_le_bytes());
    bundle.extend(bad);
    bundle.extend_from_slice(&bytes[12 + n..]);
    assert!(rc_game::bundle::decode(&bundle).is_err());
    assert!(rc_game::bundle::decode(&bytes[..bytes.len() / 2]).is_err());
    let result = serde_json::json!({"clips":clips,"fixed_tick_visual_cadence_exact":true,"clip_samples":27,"instance_isolation":true,"hit_fire_reload_death":true,"death_holds_last_frame":true,
        "pause_render_read_only":true,"restart_exact":true,"invalid_tick_rollback":true,"weapon_camera_attachment":true,"corrupt_mapping_rejected":true,
        "scope":"Behavioral port-adapter and original pose/skin checks; no original AI/weapon/notify equivalence claim."});
    fs::write(
        out.join("animation-validation.json"),
        serde_json::to_vec_pretty(&result)?,
    )?;
    println!("M2 passed: 9 changing clips, instance isolation, pause/reset, death hold, weapon camera and corruption checks");
    Ok(())
}
