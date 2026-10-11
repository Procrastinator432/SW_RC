//! Validate selected explosive rules against extracted original defaults/contacts.
use rc_game::{
    detonator::{self, Effect as ProjectileEffect},
    gunner::{self, ContactClass, Effect},
};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 2 {
        return Err("usage: rc-detonator-check ASSET_DIRECTORY".into());
    }
    let out = Path::new(&args[1]);
    let projectile: detonator::Assets =
        serde_json::from_slice(&fs::read(out.join("detonator-assets.json"))?)?;
    let target: gunner::Assets =
        serde_json::from_slice(&fs::read(out.join("gunner-assets.json"))?)?;
    projectile.validate()?;
    target.validate()?;
    assert_eq!(
        (projectile.speed, projectile.max_speed, projectile.toss_z),
        (2200., 3000., 200.)
    );
    assert_eq!(
        (
            projectile.fuse_time,
            projectile.post_impact_fuse,
            projectile.life_span
        ),
        (12., 1., 8.)
    );
    assert_eq!((projectile.damage, projectile.damage_radius), (400., 600.));
    assert_eq!(
        (
            target.contact.original,
            target.contact.radius,
            target.contact.height
        ),
        (621, 100., 40.)
    );
    assert_eq!(
        (target.bunker_health, target.bunker_script_damage),
        (49000, 50000)
    );
    let mut p = detonator::State::new(&projectile, [1., 0., 0.], [300., 800., 50.], 100.)?;
    assert_eq!(p.velocity, [2320., 0., 200.]);
    assert!(p
        .impact(&projectile, [-1., 0., 0.], false, false)?
        .contains(&ProjectileEffect::BounceSound));
    assert_eq!(p.timer_remaining, Some(1.));
    p.advance(&projectile, 0.5)?;
    p.impact(&projectile, [0., 0., 1.], false, false)?;
    assert_eq!(p.timer_remaining, Some(0.5));
    assert_eq!(
        p.advance(&projectile, 0.5)?,
        vec![ProjectileEffect::Explode, ProjectileEffect::Destroy]
    );
    assert!(p.advance(&projectile, 1.)?.is_empty());
    let mut expired = detonator::State::new(&projectile, [1., 0., 0.], [0.; 3], 100.)?;
    for _ in 0..7 {
        assert!(expired.advance(&projectile, 1.)?.is_empty());
    }
    assert_eq!(
        expired.advance(&projectile, 1.)?,
        vec![ProjectileEffect::Destroy]
    );
    // These are synthetic class/contact dispatch probes at the original cylinder,
    // not evidence of a thrown grenade traversing the actual BSP/mesh world.
    let center = target.contact.position;
    let from = [center[0] - 200., center[1], center[2]];
    let to = [center[0] + 200., center[1], center[2]];
    let mut state = gunner::State::default();
    assert!(state
        .observe(&target, from, to, ContactClass::Pawn)?
        .is_empty());
    assert!(state
        .observe(&target, from, to, ContactClass::Bullet)?
        .is_empty());
    let mut effects = state.observe(&target, from, to, ContactClass::GrenadeProjectile)?;
    assert!(effects.contains(&Effect::ObjectiveCompleted(3)));
    assert!(state.objective_completed && !state.bunker_destroyed);
    let mut trace = vec![serde_json::json!({"seconds":0.,"effects":effects,"state":state})];
    for i in 1..=1200 {
        let emitted = state.advance(&target, 1. / 120.)?;
        if !emitted.is_empty() {
            trace
                .push(serde_json::json!({"seconds":i as f64/120.,"effects":emitted,"state":state}));
        }
        effects.extend(emitted);
        if state.completed {
            break;
        }
    }
    assert!(state.completed && state.bunker_destroyed && state.confirmation_finished);
    for effect in [
        Effect::HurtActor {
            tag: "SmallBunker".into(),
            damage: 50000,
        },
        Effect::DestroyActor("SmallBunkerHatch".into()),
        Effect::DisableCollision("BunkerCollision".into()),
        Effect::Event("Conversation04".into()),
    ] {
        assert_eq!(effects.iter().filter(|e| **e == effect).count(), 1);
    }
    assert!(state
        .observe(&target, from, to, ContactClass::GrenadeProjectile)?
        .is_empty());
    assert!(state.advance(&target, 1.)?.is_empty());
    let raw: serde_json::Value = serde_json::from_slice(&fs::read(out.join("sounds.json"))?)?;
    assert_eq!(raw.as_array().ok_or("Missing original sounds")?.len(), 8);
    for row in raw.as_array().unwrap() {
        rc_game::combat::validate_wave(&fs::read(
            out.join("sounds")
                .join(row["file"].as_str().ok_or("Missing PCM filename")?),
        )?)?;
    }
    fs::write(
        out.join("flow-trace.json"),
        serde_json::to_vec_pretty(&trace)?,
    )?;
    fs::write(
        out.join("flow-validation.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"passed":true,
        "original_projectile_defaults":true,"contact_shortens_fuse_once":true,"lifetime_without_explosion":true,
        "once_only_explosion":true,"grenade_class_contact_only":true,"objective_before_delayed_geometry":true,
        "script_damage_50000_for_49000_health":true,"hatch_and_collision_effects":true,"confirmation_timing":true,"eight_original_pcm_samples":true,
        "scope":"Selected script event/clock host with original defaults and synthetic contacts. No world trajectory, actor damage application, scene graphics, inventory/animation release, Android throw input or playable objective3 integration yet."}))?,
    )?;
    println!("Original detonator rules and gun-position contact/destruction/confirmation flow passed; scene integration remains open");
    Ok(())
}
