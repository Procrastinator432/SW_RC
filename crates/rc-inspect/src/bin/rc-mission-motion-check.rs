//! Original-size maneuver probes on original static geometry, no campaign claim.
use rc_game::{mission_motion as motion, warrior_ai::EXTENT};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    let game = rc_game::bundle::decode(&fs::read(&args[1])?)?;
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let mesh = &game.artillery.as_ref().unwrap().warrior;
    let nav = &game.warrior_ai.as_ref().unwrap().navigation;
    let mut lunges = vec![];
    for clip in ["Lunge", "Lunge2"] {
        let mut found = None;
        'nodes: for n in &nav.nodes {
            for i in 0..16 {
                let yaw = i as f64 * std::f64::consts::TAU / 16.;
                if let Some(m) = motion::lunge(&game.world, mesh, n.position, yaw, clip)? {
                    found = Some(m);
                    break 'nodes;
                }
            }
        }
        let mut m = found.ok_or(format!("No original-size {clip} corridor"))?;
        let start = m.origin;
        let mut position = start;
        let mut steps = 0;
        loop {
            let (done, blocked) = m.advance(&game.world, mesh, &mut position)?;
            assert!(!blocked);
            steps += 1;
            if done {
                break;
            }
            assert!(steps < 600);
        }
        assert!((position[0] - start[0]).hypot(position[1] - start[1]) > 100.);
        lunges.push(serde_json::json!({"clip":clip,"origin":start,"end":position,"steps":steps,"extent":EXTENT}));
    }
    let jump = &game.tactics.as_ref().unwrap().assets.jump;
    let mut found = None;
    'nodes: for n in &nav.nodes {
        for dest in &jump.nodes {
            let d = ((n.position[0] - dest.position[0] as f64).powi(2)
                + (n.position[1] - dest.position[1] as f64).powi(2))
            .sqrt();
            if !(500. ..3000.).contains(&d) {
                continue;
            }
            if let Some(m) = motion::jump(
                &game.world,
                &game.volumes,
                mesh,
                jump,
                n.position,
                dest,
                false,
            )? {
                found = Some(m);
                break 'nodes;
            }
        }
    }
    let mut m = found.ok_or("No supported original-size JumpDest trajectory")?;
    let start = m.origin;
    let mut position = start;
    let mut clips = vec![];
    let mut steps = 0;
    loop {
        let (done, blocked) = m.advance(&game.world, mesh, &mut position)?;
        assert!(
            !blocked,
            "Accepted jump blocked at {position:?}, stage{}",
            m.stage
        );
        if clips.last() != Some(&m.clip) {
            clips.push(m.clip.clone());
        }
        steps += 1;
        if done {
            break;
        }
        assert!(steps < 1200);
    }
    assert_eq!(clips, vec!["TakeOff", "FallBreathe", "Land"]);
    assert!(game
        .world
        .floor_contact(position, EXTENT, 2., 0.7)?
        .is_some());
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"scope":"Isolated original-geometry maneuvers, original-size collision and root curves; bounded ballistic host, no native physFalling parity", "lunges":lunges,"jump":{"origin":start,"end":position,"node":m.node,"clips":clips,"steps":steps,"extent":EXTENT}}),
        )?,
    )?;
    println!("Both native-size lunges and JumpDest takeoff/flight/landing passed");
    Ok(())
}
