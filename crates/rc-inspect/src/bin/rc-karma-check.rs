//! Controlled incoming-projectile probes on genuine mission replay states.
//! Projectile launch/fuse are prepared test inputs, not a manual playthrough.
//! Pawn position, health, actor poses and mission progress are never assigned.
use rc_game::{Game, Input, STEP};
use std::{env, fs, path::Path};
fn input(r: &[u8]) -> Input {
    let v = |i| f32::from_le_bytes(r[i..i + 4].try_into().unwrap()) as f64;
    let f = u32::from_le_bytes(r[16..20].try_into().unwrap());
    Input {
        forward: v(0),
        strafe: v(4),
        look: [v(8), v(12)],
        jump: f & 1 != 0,
        fire: f & 2 != 0,
        reload: f & 4 != 0,
        walking: f & 8 != 0,
        use_action: f & 16 != 0,
        throw_grenade: f & 32 != 0,
        crouch: f & 64 != 0,
    }
}
fn thresholds(g: &Game) -> &[i32] {
    g.state
        .opening
        .as_ref()
        .unwrap()
        .rigid_interactions
        .as_ref()
        .unwrap()
}
fn ray(g: &Game, active: bool) -> Result<(usize, [f64; 3], [f64; 3]), String> {
    let p = g.opening.as_ref().unwrap();
    let bodies = g.state.opening.as_ref().unwrap().rigid.as_ref().unwrap();
    for (i, (b, s)) in p.assets.rigid.iter().zip(bodies).enumerate() {
        if s.active != active || (active && !s.sleeping) {
            continue;
        }
        let actor = g
            .dynamic
            .as_ref()
            .unwrap()
            .objects
            .iter()
            .find(|o| o.original == b.original)
            .unwrap()
            .actor;
        let vertices = b.vertices.map(|v| s.point(b, v));
        for axis in 0..3 {
            let span = vertices
                .iter()
                .map(|v| (v[axis] - s.center[axis]).abs())
                .fold(0., f64::max);
            for side in [-1., 1.] {
                for offset in [-0.2, 0., 0.2] {
                    let target = std::array::from_fn(|k| {
                        s.center[k]
                            + if k == (axis + 1) % 3 {
                                offset * 20.
                            } else {
                                0.
                            }
                    });
                    let from = std::array::from_fn(|k| {
                        target[k] + if k == axis { side * (span + 20.) } else { 0. }
                    });
                    let q = g.world.sweep_motion(from, target, [0.; 3])?;
                    if !q.complete {
                        return Err("Incomplete controlled projectile query".into());
                    }
                    if q.contacts.first().is_some_and(|h| {
                        h.object == g.world.actors[actor].name && h.hit.fraction < 1.
                    }) {
                        return Ok((i, from, target));
                    }
                }
            }
        }
    }
    Err("No visible selected body for controlled projectile probe".into())
}
fn shot(g: &mut Game, from: [f64; 3], target: [f64; 3]) -> Result<(), String> {
    let delta: [f64; 3] = std::array::from_fn(|k| target[k] - from[k]);
    let length = delta.iter().map(|v| v * v).sum::<f64>().sqrt();
    let direction = delta.map(|v| v / length);
    let damage = g
        .dc15
        .as_ref()
        .map_or_else(|| g.combat.as_ref().map_or(30, |a| a.damage), |a| a.damage);
    let q = g.world.sweep_motion(from, target, [0.; 3])?;
    let fraction = q
        .contacts
        .first()
        .ok_or("Missing incoming projectile contact")?
        .hit
        .fraction;
    assert!(g.rigid_projectile_hit(from, target, fraction, damage, direction)?);
    assert_eq!(g.advance(STEP, Input::default())?, 1);
    Ok(())
}
fn occluder(g: &mut Game, from: [f64; 3], target: [f64; 3]) {
    let query = g.world.sweep_motion(from, target, [0.; 3]).unwrap();
    assert!(query.complete);
    let fraction = query.contacts.first().map_or(1., |h| h.hit.fraction);
    let center: [f64; 3] =
        std::array::from_fn(|i| from[i] + (target[i] - from[i]) * fraction * 0.5);
    let length = (0..3)
        .map(|i| (target[i] - from[i]).powi(2))
        .sum::<f64>()
        .sqrt()
        * fraction;
    let half = (length * 0.2).clamp(0.01, 5.);
    let hull = rc_package::hull::HullSet {
        hulls: vec![rc_package::hull::Hull {
            offset: 0,
            bounds: [center.map(|v| v - half), center.map(|v| v + half)],
            planes: vec![],
        }],
        missing_solid_leaves: 0,
        empty_root_solid: false,
    };
    g.world.actors.push(rc_package::world_collision::BodyActor {
        name: "KarmaProbe.Occluder".into(),
        shape: rc_package::world_collision::BodyShape::Hulls(std::sync::Arc::new(hull)),
    });
}
fn grenade(g: &mut Game, from: [f64; 3]) -> Result<(), String> {
    let a = &g.thermal.as_ref().unwrap().assets.projectile;
    let mut grenade = rc_game::detonator::State::new(
        a,
        [1., 0., 0.],
        [0.; 3],
        g.state.statistics.ticks as f64 * STEP,
    )?;
    grenade.velocity = [0.; 3];
    grenade.timer_remaining = Some(STEP / 2.);
    let flight = rc_game::detonator_flight::State::new(grenade, from)?;
    let t = g.state.thermal.as_mut().unwrap();
    t.released += 1;
    t.projectiles.push(flight);
    let detonated = t.detonated;
    assert_eq!(g.advance(STEP, Input::default())?, 1);
    assert_eq!(g.state.thermal.as_ref().unwrap().detonated, detonated + 1);
    Ok(())
}
fn seal(v: &serde_json::Value) -> Vec<u8> {
    let b = serde_json::to_vec(v).unwrap();
    let mut out = b"RCSAVE01".to_vec();
    out.extend((b.len() as u32).to_le_bytes());
    out.extend(
        u64::from_str_radix(&rc_game::replay::fingerprint(&b), 16)
            .unwrap()
            .to_le_bytes(),
    );
    out.extend(b);
    out
}
fn continuation(
    g: &mut Game,
    scene: &[u8],
    identity: &str,
    out: &Path,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let checkpoint = g.checkpoint(identity)?;
    fs::write(out.join(format!("{name}.rcsave")), &checkpoint)?;
    let mut restored = rc_game::bundle::decode(scene)?;
    restored.restore_checkpoint(&checkpoint, identity)?;
    assert_eq!(
        g.render_frame(160, 100)?.gpu_packet(&g.scene),
        restored.render_frame(160, 100)?.gpu_packet(&restored.scene)
    );
    let mut bad: serde_json::Value = serde_json::from_slice(&checkpoint[20..])?;
    bad["state"]["opening"]
        .as_object_mut()
        .unwrap()
        .remove("rigid_interactions");
    assert!(restored.restore_checkpoint(&seal(&bad), identity).is_err());
    assert_eq!(checkpoint, restored.checkpoint(identity)?);
    let mut bad: serde_json::Value = serde_json::from_slice(&checkpoint[20..])?;
    bad["state"]["opening"]["rigid_interactions"] = serde_json::json!([0]);
    assert!(restored.restore_checkpoint(&seal(&bad), identity).is_err());
    assert_eq!(checkpoint, restored.checkpoint(identity)?);
    for _ in 0..60 {
        assert_eq!(g.advance(STEP, Input::default())?, 1);
        assert_eq!(restored.advance(STEP, Input::default())?, 1);
    }
    assert_eq!(g.checkpoint(identity)?, restored.checkpoint(identity)?);
    assert_eq!(
        g.render_frame(160, 100)?.gpu_packet(&g.scene),
        restored.render_frame(160, 100)?.gpu_packet(&restored.scene)
    );
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 4 {
        return Err("usage: rc-karma-check SCENE REPLAY OUT".into());
    }
    let scene = fs::read(&args[1])?;
    let recording = fs::read(&args[2])?;
    if recording.len() < 12 || &recording[..8] != b"RCINPUT1" || (recording.len() - 12) % 20 != 0 {
        return Err("Invalid input recording".into());
    }
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let identity = format!("{}:{}", scene.len(), rc_game::replay::fingerprint(&scene));
    let rows: Vec<_> = recording[12..].chunks_exact(20).collect();
    let mut g = rc_game::bundle::decode(&scene)?;
    if g.opening
        .as_ref()
        .and_then(|p| p.assets.rigid_interactions.as_ref())
        .is_none()
    {
        return Err("Missing selected Karma interactions".into());
    }
    for r in rows.iter().take(4000) {
        assert_eq!(g.advance(STEP, input(r))?, 1);
    }
    let initial = g.checkpoint(&identity)?;
    let (i, from, target) = ray(&g, false)?;
    let before = g.state.opening.as_ref().unwrap().rigid.as_ref().unwrap()[i].clone();
    let threshold_before = thresholds(&g)[i];
    let damage = g
        .dc15
        .as_ref()
        .map_or_else(|| g.combat.as_ref().map_or(30, |a| a.damage), |a| a.damage)
        as i32;
    shot(&mut g, from, target)?;
    assert_eq!(thresholds(&g)[i], threshold_before - damage);
    assert_eq!(
        before,
        g.state.opening.as_ref().unwrap().rigid.as_ref().unwrap()[i],
        "Shot must not switch PHYS_None to Karma"
    );
    g.restore_checkpoint(&initial, &identity)?;
    for r in rows.iter().skip(4000).take(2000) {
        assert_eq!(g.advance(STEP, input(r))?, 1);
    }
    let base = g.checkpoint(&identity)?;
    let (i, from, target) = ray(&g, true)?;
    let before_wall = thresholds(&g).to_vec();
    occluder(&mut g, from, target);
    let fraction = g.wall_fraction(from, target)?;
    assert!(!g.rigid_projectile_hit(from, target, fraction, damage as u32, [1., 0., 0.])?);
    assert_eq!(before_wall, thresholds(&g));
    g.world.actors.pop();
    g.restore_checkpoint(&base, &identity)?;
    let old = g.state.opening.as_ref().unwrap().rigid.as_ref().unwrap()[i].clone();
    let threshold_before = thresholds(&g)[i];
    shot(&mut g, from, target)?;
    assert_eq!(thresholds(&g)[i], threshold_before - damage);
    let after = g.state.opening.as_ref().unwrap().rigid.as_ref().unwrap()[i].clone();
    assert!(old.sleeping && !after.sleeping && after.velocity != old.velocity);
    continuation(&mut g, &scene, &identity, out, "projectile")?;
    g.restore_checkpoint(&base, &identity)?;
    let (i, from, _) = ray(&g, true)?;
    let before_blast = thresholds(&g).to_vec();
    let p = g.opening.as_ref().unwrap();
    let b = &p.assets.rigid[i];
    let prop = &p.assets.rigid_interactions.as_ref().unwrap().props[i];
    let at = g.state.opening.as_ref().unwrap().rigid.as_ref().unwrap()[i].point(b, prop.location);
    occluder(&mut g, from, at);
    grenade(&mut g, from)?;
    assert_eq!(
        thresholds(&g)[i],
        before_blast[i],
        "Synthetic occluder must block blast damage to this actor"
    );
    g.world.actors.pop();
    g.restore_checkpoint(&base, &identity)?;
    grenade(&mut g, from)?;
    assert!(
        thresholds(&g).iter().zip(&before_blast).any(|(a, b)| a < b),
        "Blast must reach a visible selected body"
    );
    let after_blast = g.state.opening.as_ref().unwrap().rigid.as_ref().unwrap()[i].clone();
    assert!(!after_blast.sleeping);
    continuation(&mut g, &scene, &identity, out, "blast")?;
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"scene_identity":identity,"inactive_shot_preserves_physics":true,"active_shot_wakes_body":true,"shot_before":old,"shot_after":after,"grenade_explosion_dispatch":true,"occluded_projectile_and_blast_rejected":true,"blast_body":after_blast,"checkpoint_continuations":2,"atomic_corrupt_thresholds":4,"scope":"Prepared incoming projectile and fuse events in genuine original mission states; shared weapon impact dispatch and ordinary grenade ticks, original parameters; an extra synthetic occluder supplies the negative trace cases. No Pawn position, health or actor-pose assignment; not a manual gameplay run."}),
        )?,
    )?;
    println!("Original-scene Karma projectile/blast dispatch and checkpoint continuations passed");
    Ok(())
}
