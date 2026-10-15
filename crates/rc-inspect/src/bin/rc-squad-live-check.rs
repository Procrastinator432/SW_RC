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
    let scene = fs::read(&args[1])?;
    let replay = fs::read(&args[2])?;
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let mut g = rc_game::bundle::decode(&scene)?;
    let mut rows = vec![];
    for row in replay[12..].chunks_exact(20) {
        advance(&mut g, &mut rows, input(row))?;
        if g.state
            .artillery
            .as_ref()
            .unwrap()
            .warriors
            .iter()
            .any(|w| w.health > 0 && w.ai.as_ref().is_some_and(|a| a.last_seen.is_some()))
        {
            break;
        }
    }
    let before = serde_json::to_value(&g.state)?;
    let enemy = g
        .state
        .artillery
        .as_ref()
        .unwrap()
        .warriors
        .iter()
        .find(|w| w.health > 0)
        .unwrap()
        .pawn
        .body
        .position;
    let look = aim(&g, enemy);
    advance(
        &mut g,
        &mut rows,
        Input {
            look,
            use_action: true,
            ..Input::default()
        },
    )?;
    advance(&mut g, &mut rows, Input::default())?;
    for _ in 0..3600 {
        let use_action = g.state.squad.as_ref().unwrap().player_down;
        advance(
            &mut g,
            &mut rows,
            Input {
                use_action,
                ..Input::default()
            },
        )?;
        if g.state.squad.as_ref().unwrap().kills > 0
            && g.state.health > 0
            && !g.state.squad.as_ref().unwrap().player_down
        {
            break;
        }
        if g.state.phase != Phase::Playing {
            break;
        }
    }
    fs::write(out.join("state.json"), serde_json::to_vec_pretty(&g.state)?)?;
    assert_eq!(g.state.phase, Phase::Playing);
    assert!(
        g.state.squad.as_ref().unwrap().kills > 0,
        "Scorch never reached and killed a mission Warrior"
    );
    assert!(!g.state.squad.as_ref().unwrap().player_down);
    let target = g.state.rendezvous.as_ref().unwrap().pawn.body.position;
    let look = aim(&g, target);
    advance(
        &mut g,
        &mut rows,
        Input {
            look,
            ..Input::default()
        },
    )?;
    save(out, "squad-assist-input.bin", &rows)?;
    let mut ppm = b"P6\n640 400\n255\n".to_vec();
    for p in g.render(640, 400)? {
        ppm.extend([(p >> 16) as u8, (p >> 8) as u8, p as u8]);
    }
    fs::write(out.join("squad-assist.ppm"), ppm)?;
    fs::write(
        out.join("validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"normal_inputs_only":true,"inputs":rows.len()/20,"before":before,"after":g.state,"scope":"Input-only prefix then ceasefire and Use when incapacitated; Scorch must reach, fire and kill a factory Warrior without state assignment"}),
        )?,
    )?;
    println!(
        "Input-only Scorch assist passed, {} inputs",
        rows.len() / 20
    );
    Ok(())
}
