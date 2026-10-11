//! Prepared release-point feasibility in the actual supplied geo_01a collision world.
//! This does not prove a player route, Game integration, all projectile blockers or Android.
use rc_game::{bundle, detonator, detonator_flight as flight, gunner, navigation, Point, STEP};
use rc_package::{
    properties::{self, Value},
    read_package,
};
use std::{env, fs, path::Path};
fn distance(a: Point, b: Point) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}
fn render_flight(game:&mut rc_game::Game,source:&Path,out:&Path,trace:&serde_json::Value)->Result<(),Box<dyn std::error::Error>> {
    let mesh=rc_render::Scene::from_snapshot(&fs::read(source.join("detonator.rcscene"))?)?;
    if mesh.triangles.len()!=352 || mesh.appearance.iter().any(Option::is_none) {return Err("Incomplete grenade visual".into());}
    let texture_base=game.scene.textures.len();
    game.scene.textures.extend(mesh.textures);
    game.state.pawn.body.position=serde_json::from_value(trace["stand"].clone())?;
    let frames=trace["frames"].as_array().ok_or("No flight frames")?;
    let mut rows=vec![];
    for index in [0,frames.len()/2,frames.len()-1] {
        let position:Point=serde_json::from_value(frames[index]["position"].clone())?;
        game.aim_at(position);
        let mut frame=game.render_frame(640,400)?;
        frame.hud.clear();
        frame.triangles.truncate(frame.weapon_start);
        let faces=frame.appearance.as_mut().ok_or("No frame materials")?;
        faces.truncate(frame.weapon_start);
        let baseline=frame.render_cpu(&game.scene)?;
        frame.triangles.extend(mesh.triangles.iter().map(|t|rc_package::geometry::Triangle {
            points:t.points.map(|p|std::array::from_fn(|i|p[i]+position[i] as f32)),surface:t.surface,
        }));
        frame.appearance.as_mut().unwrap().extend(mesh.appearance.iter().map(|f|f.map(|mut f| {f.texture+=texture_base;f})));
        frame.weapon_start=frame.triangles.len(); // Projectile participates in world depth.
        let pixels=frame.render_cpu(&game.scene)?;
        let changed=pixels.iter().zip(&baseline).filter(|(a,b)|a!=b).count();
        fs::write(out.join(format!("flight-{index}.argb")),pixels.iter().flat_map(|v|v.to_le_bytes()).collect::<Vec<_>>())?;
        fs::write(out.join(format!("flight-{index}.rcframe")),frame.gpu_packet(&game.scene))?;
        rows.push(serde_json::json!({"frame":index,"position":position,"changed_pixels":changed}));
    }
    if !rows.iter().any(|r|r["changed_pixels"].as_u64().unwrap_or(0)>0) {return Err("Grenade is invisible in every flight view".into());}
    fs::write(out.join("render-validation.json"),serde_json::to_vec_pretty(&serde_json::json!({
        "passed":true,"projectile_faces":352,"textured_faces":352,"world_depth":true,"frames":rows,
        "scope":"Prepared CPU renders and shared GPU packet generation at actual flight positions. Camera tracks projectile; no live Game presentation or Android acceptance"
    }))?)?;
    Ok(())
}
fn walk_route(game: &rc_game::Game, points: &[Point]) -> Result<(Point, Vec<Point>), String> {
    let mut pawn = rc_package::controller::ControlledBody {
        body: rc_package::physics::BodyState {
            position: points[0],
            velocity: [0.; 3],
            grounded: true,
        },
        jump_held: false,
    };
    let options = rc_package::pawn_motion::PawnRuntimeOptions {
        maximum_desired_speed: game.profile.controller.speed,
        weapon_modifier: 1.,
        crouch_ratio: 1.,
        wounded_ratio: 1.,
        crouched: false,
        wounded: false,
        wants_to_crouch: false,
        current_jump_z: game.profile.controller.jump_speed,
    };
    let mut reached = vec![pawn.body.position];
    for target in points.iter().skip(1) {
        let mut finished = false;
        let mut best = f64::INFINITY;
        let mut stalled = 0;
        for _ in 0..900 {
            let d = distance(pawn.body.position, *target);
            if d < 25. && pawn.body.grounded {
                finished = true;
                break;
            }
            if d < best - 1. {
                best = d;
                stalled = 0;
            } else {
                stalled += 1;
            }
            if stalled > 90 {
                break;
            }
            let yaw = ((target[1] - pawn.body.position[1]).atan2(target[0] - pawn.body.position[0])
                * 65536.
                / std::f64::consts::TAU) as i32;
            pawn = game
                .volumes
                .pawn_diagnostic_tick(
                    &game.world,
                    &pawn,
                    rc_package::controller::ViewInput {
                        forward: (d / 90.).min(1.),
                        strafe: 0.,
                        yaw,
                        walking: false,
                        jump: false,
                    },
                    STEP,
                    &game.profile,
                    options,
                )?
                .state;
        }
        if !finished {
            return Err(format!(
                "Route stalled {:?} -> {target:?}",
                pawn.body.position
            ));
        }
        reached.push(pawn.body.position);
    }
    Ok((pawn.body.position, reached))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 6 {
        return Err(
            "usage: rc-detonator-world-check GAME_DATA SCENE DETONATOR_ASSETS ROUTE_GRAPH OUTPUT"
                .into(),
        );
    }
    let root = Path::new(&args[1]);
    let source = Path::new(&args[3]);
    let out = Path::new(&args[5]);
    fs::create_dir_all(out)?;
    let mut game = bundle::decode(&fs::read(&args[2])?)?;
    for id in [1122, 915, 617, 909, 910, 912, 913, 964] {
        game.disable_dynamic(id)?;
    }
    let rules: detonator::Assets =
        serde_json::from_slice(&fs::read(source.join("detonator-assets.json"))?)?;
    let target: gunner::Assets =
        serde_json::from_slice(&fs::read(source.join("gunner-assets.json"))?)?;
    let catalog = rc_inspect::defaults::load(root)?;
    let raw = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&raw)?;
    let mut receipt = vec![];
    let mut number = |class: &str, name: &str| -> Result<f32, String> {
        let resolved = catalog.resolve(class, name, 0)?;
        let v = match &resolved.value {
            Value::Float(v) => *v,
            _ => return Err("Expected float".into()),
        };
        receipt.push(serde_json::json!({"class":class,"property":name,"resolved":resolved}));
        Ok(v)
    };
    let mut assets = flight::Assets {
        mass: number("CTInventory.ThermalDetonatorProj", "Mass")?,
        buoyancy: number("CTInventory.ThermalDetonatorProj", "Buoyancy")?,
        volumes: vec![flight::Volume {
            name: "DefaultPhysicsVolume".into(),
            fluid_friction: number("Engine.DefaultPhysicsVolume", "FluidFriction")?,
        }],
    };
    for volume in &game.volumes.volumes {
        let (i, e) = pkg
            .exports
            .iter()
            .enumerate()
            .find(|(i, _)| {
                pkg.object_path(*i as i32 + 1)
                    .is_ok_and(|p| p == volume.name)
            })
            .ok_or("Missing original volume")?;
        let resolved = catalog.instance(
            &pkg.object_path(e.class)?,
            &properties::read(&pkg, &raw, e)?,
            "FluidFriction",
            0,
            "geo_01a.ctm",
        )?;
        let friction = match &resolved.value {
            Value::Float(v) => *v,
            _ => return Err("Expected volume friction".into()),
        };
        receipt.push(
            serde_json::json!({"original":i+1,"property":"FluidFriction","resolved":resolved}),
        );
        assets.volumes.push(flight::Volume {
            name: volume.name.clone(),
            fluid_friction: friction,
        });
    }
    assets.validate()?;
    let graph: navigation::Assets = serde_json::from_slice(&fs::read(&args[4])?)?;
    let pickup = graph
        .nodes
        .iter()
        .position(|n| n.original == 610)
        .ok_or("No pickup route anchor")?;
    let offset = match catalog
        .resolve("CTInventory.DC15s", "FPGrenadeOffset", 0)?
        .value
    {
        Value::Vector(v) => v,
        _ => return Err("No original grenade offset".into()),
    };
    fs::write(
        out.join("flight-assets.json"),
        serde_json::to_vec_pretty(&assets)?,
    )?;
    fs::write(
        out.join("original-flight-data.json"),
        serde_json::to_vec_pretty(&receipt)?,
    )?;
    let mut anchors = vec![];
    for (i, e) in pkg.exports.iter().enumerate() {
        if pkg.object_path(e.class)? != "Engine.PathNode" {
            continue;
        }
        let pos = match catalog
            .instance(
                &pkg.object_path(e.class)?,
                &properties::read(&pkg, &raw, e)?,
                "Location",
                0,
                "geo_01a.ctm",
            )?
            .value
        {
            Value::Vector(v) => v.map(f64::from),
            _ => continue,
        };
        if distance(pos, target.contact.position) > 2000. {
            continue;
        }
        let mut supported = vec![];
        for lift in [0., 32., 64., 120., 360., 640.] {
            let start = [pos[0], pos[1], pos[2] + lift];
            if let Ok(Some(h)) =
                game.world
                    .floor_contact(start, game.profile.physics.extent, 1280., 0.7)
            {
                let stand = [start[0], start[1], start[2] - 1280. * h.hit.fraction + 0.5];
                if game
                    .world
                    .body_placement(stand, game.profile.physics.extent)?
                    .state
                    != rc_package::movement::PlacementState::Penetrating
                {
                    supported.push(stand);
                }
            }
        }
        if let Some(stand) = supported
            .into_iter()
            .min_by(|a, b| distance(*a, pos).total_cmp(&distance(*b, pos)))
        {
            anchors.push((i + 1, stand));
        }
    }
    anchors.sort_by(|a, b| {
        distance(a.1, target.contact.position).total_cmp(&distance(b.1, target.contact.position))
    });
    let mut attempts = vec![];
    let mut success = None;
    let mut routes = vec![];
    'anchors: for (original, _) in &anchors {
        let Some(end) = graph
            .nodes
            .iter()
            .position(|n| n.original == *original as i32)
        else {
            continue;
        };
        let Some(path) = graph.shortest(&[(pickup, 0.)], &[(end, 0.)]) else {
            continue;
        };
        let points = path
            .iter()
            .map(|&i| graph.nodes[i].position)
            .collect::<Vec<_>>();
        let (stand, reached) = match walk_route(&game, &points) {
            Ok(v) => v,
            Err(e) => {
                routes.push(serde_json::json!({"original":original,"passed":false,"error":e}));
                continue;
            }
        };
        routes.push(serde_json::json!({"original":original,"passed":true,"points":points,"reached":reached}));
        let eye = [
            stand[0],
            stand[1],
            stand[2] + game.profile.physics.extent[2] * 0.65,
        ];
        let delta: Point = std::array::from_fn(|i| target.contact.position[i] - eye[i]);
        let yaw = delta[1].atan2(delta[0]);
        let base = delta[2].atan2(delta[0].hypot(delta[1]));
        for extra_pitch in [-0.1, 0., 0.1, 0.2, 0.3, 0.4] {
            let pitch = base + extra_pitch;
            let matrix = rc_package::mesh_transform::rotation_matrix(
                [
                    (pitch * 65536. / std::f64::consts::TAU) as i32,
                    (yaw * 65536. / std::f64::consts::TAU) as i32,
                    0,
                ],
                [0; 3],
            )?;
            let axes = std::array::from_fn::<_, 3, _>(|i| {
                std::array::from_fn::<_, 3, _>(|j| f32::from_bits(matrix[i][j]))
            });
            let origin =
                rc_package::projectile_motion::fire_start(eye.map(|v| v as f32), axes, offset)
                    .map(f64::from);
            let mut s = flight::State::new(
                detonator::State::new(&rules, axes[0], [0.; 3], 100.)?,
                origin,
            )?;
            let mut gun = gunner::State::default();
            let mut frames = vec![];
            let mut error = None;
            let mut nearest = distance(origin, target.contact.position);
            for tick in 0..480 {
                let f = match s.tick(&rules, &assets, &game.world, &game.volumes, STEP) {
                    Ok(f) => f,
                    Err(e) => {
                        error = Some(e);
                        break;
                    }
                };
                for p in &f.segments {
                    gun.observe(
                        &target,
                        p.from,
                        p.to,
                        gunner::ContactClass::GrenadeProjectile,
                    )?;
                }
                nearest = nearest.min(distance(s.position, target.contact.position));
                frames.push(serde_json::json!({"tick":tick+1,"position":s.position,"projectile":s.projectile,"frame":f,"gunner":gun}));
                if gun.objective_completed || s.projectile.ended.is_some() {
                    break;
                }
            }
            attempts.push(serde_json::json!({"original":original,"stand":stand,"pitch":pitch,"nearest":nearest,"contact":gun.objective_completed,"error":error}));
            if gun.objective_completed {
                success = Some(
                    serde_json::json!({"original_anchor":original,"route":points,"reached":reached,"stand":stand,"eye":eye,"origin":origin,"yaw":yaw,"pitch":pitch,"frames":frames}),
                );
                break 'anchors;
            }
        }
    }
    fs::write(
        out.join("world-attempts.json"),
        serde_json::to_vec_pretty(&attempts)?,
    )?;
    fs::write(
        out.join("world-routes.json"),
        serde_json::to_vec_pretty(&routes)?,
    )?;
    let Some(success) = success else {
        return Err(format!(
            "No supported release point reached original trigger621 in {} attempts",
            attempts.len()
        )
        .into());
    };
    render_flight(&mut game,source,out,&success)?;
    fs::write(
        out.join("world-flight-trace.json"),
        serde_json::to_vec_pretty(&success)?,
    )?;
    fs::write(
        out.join("world-validation.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "passed":true,"supported_original_anchors":anchors.len(),"attempts":attempts.len(),"original_trigger":621,
            "actual_clipped_projectile_contact":true,"original_release_offset":offset,"route_from_pickup_physically_walked":true,
            "scope":"Continuous pawn diagnostic route from pickup followed by physical flight from actually reached endpoint in supplied player-selected static world and current resting-wall states. No all original projectile blocker/touch flags, integrated Game/JNI or Android proof"
        }))?,
    )?;
    println!(
        "Original trigger621 reached by physical grenade flight after {} attempts",
        attempts.len()
    );
    Ok(())
}
