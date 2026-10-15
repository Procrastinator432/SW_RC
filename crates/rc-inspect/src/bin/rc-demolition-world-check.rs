//! Feasibility/asset check after a real objective0-4 replay, without attaching goal5.
use rc_game::{bundle, dynamic, navigation, Input, Point, STEP};
use rc_package::{
    mesh_query::ActorTransform,
    properties::{self, Value},
    read_package,
};
use std::{env, fs, path::Path};
fn distance(a: Point, b: Point) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 6 {
        return Err("usage: rc-demolition-world-check GAME_DATA SCENE INPUT GRAPH OUTPUT".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[5]);
    fs::create_dir_all(out)?;
    let mut game = bundle::decode(&fs::read(&args[2])?)?;
    let inputs = fs::read(&args[3])?;
    assert!(inputs.starts_with(b"RCINPUT1") && (inputs.len() - 12) % 20 == 0);
    for row in inputs[12..].chunks_exact(20) {
        let f = |i| f32::from_le_bytes(row[i..i + 4].try_into().unwrap()) as f64;
        let b = u32::from_le_bytes(row[16..20].try_into().unwrap());
        game.advance(
            STEP,
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
            },
        )?;
    }
    assert_eq!(game.state.phase, rc_game::Phase::Won);
    assert!(game.state.rendezvous.as_ref().unwrap().flow.squad_joined);
    let original_state = serde_json::to_vec(&game.state)?;
    let graph: navigation::Assets = serde_json::from_slice(&fs::read(&args[4])?)?;
    let at = |id| {
        graph
            .nodes
            .iter()
            .position(|n| n.original == id)
            .ok_or("Missing route anchor")
    };
    let path = graph
        .shortest(&[(at(792)?, 0.)], &[(at(148)?, 0.)])
        .ok_or("Missing demolition approach route")?;
    let mut route = path
        .into_iter()
        .skip(1)
        .map(|i| {
            let mut p = graph.nodes[i].position;
            p[2] += 28.;
            p
        })
        .collect::<Vec<_>>();
    let bytes = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&bytes)?;
    let catalog = rc_inspect::defaults::load(root)?;
    let value = |id: usize, name| -> Result<Value, String> {
        let e = &pkg.exports[id - 1];
        Ok(catalog
            .instance(
                &pkg.object_path(e.class)?,
                &properties::read(&pkg, &bytes, e)?,
                name,
                0,
                "geo_01a",
            )?
            .value)
    };
    let vector = |id, name| -> Result<[f32; 3], String> {
        match value(id, name)? {
            Value::Vector(v) => Ok(v),
            _ => Err("Missing vector".into()),
        }
    };
    let marker = vector(466, "Location")?.map(f64::from);
    let mut endpoint = marker;
    endpoint[2] = route.last().ok_or("Empty route")?[2];
    route.push(endpoint);
    let mut pawn = game.state.rendezvous.as_ref().unwrap().pawn;
    let mut trace = vec![];
    let mut passed = true;
    for target in &route {
        let mut reached = false;
        let mut frames = 0;
        for _ in 0..1200 {
            let p = pawn.body.position;
            if distance(p, *target) < 25. && pawn.body.grounded {
                reached = true;
                break;
            }
            let yaw = ((target[1] - p[1]).atan2(target[0] - p[0]) * 65536. / std::f64::consts::TAU)
                as i32;
            pawn = game
                .volumes
                .pawn_diagnostic_tick(
                    &game.world,
                    &pawn,
                    rc_package::controller::ViewInput {
                        forward: 1.,
                        strafe: 0.,
                        yaw,
                        walking: false,
                        jump: false,
                    },
                    STEP,
                    &game.profile,
                    rc_package::pawn_motion::PawnRuntimeOptions {
                        maximum_desired_speed: 450.,
                        weapon_modifier: 1.,
                        crouch_ratio: 1.,
                        wounded_ratio: 1.,
                        crouched: false,
                        wounded: false,
                        wants_to_crouch: false,
                        current_jump_z: game.profile.controller.jump_speed,
                    },
                )?
                .state;
            frames += 1;
        }
        trace.push(
            serde_json::json!({"target":target,"reached":reached,"frames":frames,"body":pawn}),
        );
        if !reached {
            passed = false;
            break;
        }
    }
    assert_eq!(
        serde_json::to_vec(&game.state)?,
        original_state,
        "Diagnostic changed accepted gameplay state"
    );
    let id = 851;
    let t = ActorTransform {
        location: vector(id, "Location")?,
        rotation: match value(id, "Rotation")? {
            Value::Rotator(v) => v,
            _ => return Err("Missing rotation".into()),
        },
        scale: vector(id, "DrawScale3D")?.map(|v| {
            v * match value(id, "DrawScale").unwrap() {
                Value::Float(v) => v,
                _ => 1.,
            }
        }),
        pivot: vector(id, "PrePivot")?,
    };
    let mut receipts = vec![];
    let original = rc_inspect::mission_assets::mesh_scene(
        root,
        "Arena_HR.ArenaWalls.Bunker2",
        &t,
        &catalog,
        &mut receipts,
    )?;
    let destroyed = rc_inspect::mission_assets::mesh_scene(
        root,
        "Arena_HR.ArenaWalls.BunkerBlown2",
        &t,
        &catalog,
        &mut receipts,
    )?;
    let data = fs::read(root.join("StaticMeshes/Arena_HR.usx"))?;
    let p = read_package(&data)?;
    let e = p
        .exports
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            p.object_path(i as i32 + 1)
                .is_ok_and(|v| v == "ArenaWalls.BunkerBlown2")
                .then_some(e)
        })
        .ok_or("Missing BunkerBlown2")?;
    let mesh = rc_package::mesh::read_static_mesh(&p, &data, e)?;
    let collision = rc_package::mesh_collision::read(&p, &data, e, &mesh)?;
    if collision.simple_model <= 0 {
        return Err("BunkerBlown2 has no original simple model".into());
    }
    let e = &p.exports[collision.simple_model as usize - 1];
    let bsp = rc_package::geometry::read_bsp(&p, &data, e)?;
    let solid = rc_package::collision::read_model_collision(&p, &data, e, &bsp)?;
    let hulls = rc_package::hull::HullSet::read(&bsp, &solid)?.transformed(&t)?;
    assert!(!hulls.hulls.is_empty());
    let before = game
        .world
        .actors
        .iter()
        .find(|a| a.name == "Prop12")
        .ok_or("Missing original large bunker collision")?;
    let original_hulls = match &before.shape {
        rc_package::world_collision::BodyShape::Hulls(h) => h,
        _ => return Err("Unexpected bunker collider".into()),
    };
    let object = dynamic::Object {
        original: 851,
        collider: rc_inspect::mission_assets::collider("Prop12".into(), original_hulls),
        snapshot: original.snapshot()?,
        released: Some(dynamic::Released {
            collider: rc_inspect::mission_assets::collider("BunkerBlown2".into(), &hulls),
            snapshot: destroyed.snapshot()?,
        }),
    };
    fs::write(
        out.join("bunker-dynamic.json"),
        serde_json::to_vec(&object)?,
    )?;
    fs::write(
        out.join("world-validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":passed,"normal_prefix_inputs":(inputs.len()-12)/20,"scene_state_unchanged":true,"route":route,"trace":trace,"marker":marker,"original_faces":original.triangles.len(),"destroyed_faces":destroyed.triangles.len(),"original_hulls":original_hulls.hulls.len(),"destroyed_hulls":hulls.hulls.len(),"materials":receipts,"scope":"Independent physical companion route and prepared original dynamic object; not live objective5 or passage acceptance"}),
        )?,
    )?;
    if !passed {
        return Err("Standing companion route blocked; see world-validation.json".into());
    }
    println!("Scorch physically reaches marker466 after10448-input prefix; Bunker2 -> BunkerBlown2 preserves {} original destroyed hulls",hulls.hulls.len());
    Ok(())
}
