//! Original map actor probes against selected simple collision Models.
use rc_package::{
    collision, geometry, hull, level, mesh, mesh_collision,
    mesh_query::{self, ActorBlocking, ActorTransform},
    properties::{self, Value},
    read_package,
    world_collision::{
        BodyActor, BodyShape, StaticActor, StaticBodyWorld, StaticShape, StaticWorld,
    },
};
use std::{collections::HashMap, env, fs, path::Path, sync::Arc};
#[path = "../defaults.rs"]
mod defaults;
#[path = "../volumes.rs"]
mod volumes_load;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    let walking_mode = args
        .get(4)
        .is_some_and(|v| v.to_str() == Some("--walking-diagnostic"));
    if args.len() != 4 && !(args.len() == 5 && walking_mode) {
        return Err(
            "Usage: rc-mesh-probe <GameData> <map.ctm> <report.json> [--walking-diagnostic]".into(),
        );
    }
    let game = Path::new(&args[1]);
    let catalog = defaults::load(game)?;
    let bytes = fs::read(&args[2])?;
    let pkg = read_package(&bytes)?;
    let source = Path::new(&args[2]).to_string_lossy();
    let starts = level::player_starts_with_defaults(&pkg, &bytes, &catalog, &source)?;
    if starts.is_empty() {
        return Err("No resolved start anchors; probe corpus would be empty".into());
    }
    let mut packages = HashMap::new();
    for entry in fs::read_dir(game.join("StaticMeshes"))? {
        let file = entry?.path();
        if file
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("usx"))
        {
            packages.insert(
                file.file_stem().unwrap().to_string_lossy().to_lowercase(),
                file,
            );
        }
    }
    let mut loaded = HashMap::new();
    let mut assets = HashMap::new();
    let binding = level::world_binding(&pkg, &bytes)?;
    let world = (|| -> Result<_, String> {
        let export = &pkg.exports[binding.model_export];
        let bsp = geometry::read_bsp(&pkg, &bytes, export)?;
        let solid = collision::read_model_collision(&pkg, &bytes, export, &bsp)?;
        let body = hull::HullSet::read(&bsp, &solid).map(Arc::new);
        Ok((solid, body))
    })();
    let (world, body) = match world {
        Ok((solid, body)) => (Ok(solid), body),
        Err(e) => (Err(e.clone()), Err(e)),
    };
    let mut static_world = StaticWorld {
        world,
        actors: vec![],
    };
    let mut body_world = StaticBodyWorld {
        world: body,
        actors: vec![],
    };
    let size = |name| -> Result<f64, String> {
        match catalog
            .resolve("CTCharacters.PlayerCommando", name, 0)?
            .value
        {
            Value::Float(v) if v.is_finite() && v > 0.0 => Ok(f64::from(v)),
            _ => Err("invalid player body dimensions".into()),
        }
    };
    let radius = size("CollisionRadius")?;
    let extent = [radius, radius, size("CollisionHeight")?];
    let mut subclass_count = 0;
    let mut actors = Vec::new();
    let mut unclassified = Vec::new();
    let (mut skipped, mut supported, mut unsupported, mut blocked, mut clear) = (0, 0, 0, 0, 0);
    for (i, export) in pkg.exports.iter().enumerate() {
        let class = pkg.object_path(export.class)?;
        let actor = pkg.object_path(i as i32 + 1)?;
        let is_actor = match catalog.derives_from(&class, "Engine.Actor") {
            Ok(value) => value,
            Err(error) => {
                unclassified.push(serde_json::json!({"object":actor,"class":class,"reason":error}));
                continue;
            }
        };
        if !is_actor {
            continue;
        }
        let props = properties::read(&pkg, &bytes, export)?;
        let mesh_value = catalog.instance(&class, &props, "StaticMesh", 0, &source);
        if matches!(&mesh_value, Ok(v) if matches!(v.value, Value::Object {index:0,..})) {
            continue;
        }
        if !class.eq_ignore_ascii_case("Engine.StaticMeshActor") {
            subclass_count += 1;
        }
        let result = (|| -> Result<_, String> {
            let value = |name| {
                catalog
                    .instance(&class, &props, name, 0, &source)
                    .map(|v| v.value)
            };
            let boolean = |name| match value(name)? {
                Value::Bool(v) => Ok(v),
                _ => Err(format!("{name} not Bool")),
            };
            let flags = ActorBlocking {
                collide_actors: boolean("bCollideActors")?,
                block_actors: boolean("bBlockActors")?,
                block_players: boolean("bBlockPlayers")?,
                cylinder: boolean("bUseCylinderCollision")?,
            };
            if !flags.collide_actors || !flags.block_players {
                return Ok(None);
            }
            let vector = |name| match value(name)? {
                Value::Vector(v) => Ok(v),
                _ => Err(format!("{name} not Vector")),
            };
            let rotation = match value("Rotation")? {
                Value::Rotator(v) => v,
                _ => return Err("Rotation not Rotator".into()),
            };
            let draw_scale = match value("DrawScale")? {
                Value::Float(v) => v,
                _ => return Err("DrawScale not Float".into()),
            };
            let transform = ActorTransform {
                location: vector("Location")?,
                rotation,
                scale: vector("DrawScale3D")?.map(|v| v * draw_scale),
                pivot: vector("PrePivot")?,
            };
            let resolved_mesh = mesh_value?;
            let local_mesh = resolved_mesh.stage == "instance_delta";
            let (index, path) = match resolved_mesh.value {
                Value::Object { index, path } => (index, path),
                _ => return Err("StaticMesh not Object".into()),
            };
            if !assets.contains_key(&path) {
                let asset = (|| -> Result<_, String> {
                    let (asset_pkg, asset_bytes, asset_export) = if index > 0 && local_mesh {
                        (&pkg, &bytes, &pkg.exports[index as usize - 1])
                    } else {
                        let (package, object) =
                            path.split_once('.').ok_or("mesh reference lacks package")?;
                        let key = package.to_lowercase();
                        if !loaded.contains_key(&key) {
                            let data = fs::read(packages.get(&key).ok_or("mesh package missing")?)
                                .map_err(|e| e.to_string())?;
                            loaded.insert(key.clone(), (read_package(&data)?, data));
                        }
                        let (p, b) = &loaded[&key];
                        let e = p
                            .exports
                            .iter()
                            .enumerate()
                            .find_map(|(i, e)| {
                                p.object_path(i as i32 + 1)
                                    .ok()
                                    .filter(|s| s.eq_ignore_ascii_case(object))
                                    .map(|_| e)
                            })
                            .ok_or("mesh export missing")?;
                        (p, b, e)
                    };
                    if asset_pkg.object_path(asset_export.class)? != "Engine.StaticMesh" {
                        return Err("reference is not StaticMesh".into());
                    }
                    let geometry = mesh::read_static_mesh(asset_pkg, asset_bytes, asset_export)?;
                    let collision =
                        mesh_collision::read(asset_pkg, asset_bytes, asset_export, &geometry)?;
                    let simple = if collision.simple_model > 0 {
                        let e = &asset_pkg.exports[collision.simple_model as usize - 1];
                        let bsp = geometry::read_bsp(asset_pkg, asset_bytes, e)?;
                        let solid = Arc::new(collision::read_model_collision(
                            asset_pkg,
                            asset_bytes,
                            e,
                            &bsp,
                        )?);
                        let hulls = hull::HullSet::read(&bsp, &solid).map(Arc::new);
                        Some((solid, hulls))
                    } else {
                        None
                    };
                    Ok((collision, simple))
                })();
                assets.insert(path.clone(), asset);
            }
            let (collision, simple) = assets[&path].as_ref().map_err(Clone::clone)?;
            let mut probes = Vec::new();
            for start in &starts {
                for axis in 0..3 {
                    for direction in [-1.0, 1.0] {
                        let mut end = start.location;
                        end[axis] += direction * 10000.0;
                        let line_clear = mesh_query::player_line_clear(
                            collision,
                            simple.as_ref().map(|v| v.0.as_ref()),
                            &transform,
                            flags,
                            start.location,
                            end,
                        )?;
                        probes.push(serde_json::json!({"start":start.actor,"axis":axis,"direction":direction,"clear":line_clear}));
                    }
                }
            }
            let shape = match collision.line_route(flags.cylinder, 0, !collision.nodes.is_empty()) {
                mesh_query::LineRoute::Clear => StaticShape::Clear,
                mesh_query::LineRoute::SimpleModel => StaticShape::Simple {
                    solid: simple
                        .as_ref()
                        .ok_or("selected simple Model unavailable")?
                        .0
                        .clone(),
                    transform: transform.clone(),
                },
                _ => return Err("unsupported selected shape".into()),
            };
            let body_route = collision.box_route(flags.cylinder, 0, !collision.nodes.is_empty());
            let body = (|| -> Result<BodyShape, String> {
                Ok(match body_route {
                    mesh_query::LineRoute::Clear => BodyShape::Clear,
                    mesh_query::LineRoute::SimpleModel => BodyShape::Hulls(Arc::new(
                        simple
                            .as_ref()
                            .ok_or("selected box Model unavailable")?
                            .1
                            .as_ref()
                            .map_err(Clone::clone)?
                            .transformed(&transform)?,
                    )),
                    _ => return Err("selected cylinder/complex box form unsupported".into()),
                })
            })();
            let body_error = body.as_ref().err().cloned();
            let body_shape = body.unwrap_or_else(BodyShape::Unsupported);
            Ok(Some((
                serde_json::json!({"mesh":path,"simple_model":collision.simple_model_path,"route":collision.line_route(flags.cylinder,0,!collision.nodes.is_empty()),"body_route":body_route,"body_error":body_error,"block_actors":flags.block_actors,"block_players":flags.block_players,"probes":probes}),
                shape,
                body_shape,
            )))
        })();
        match result {
            Ok(None) => {
                skipped += 1;
                actors.push(serde_json::json!({"actor":actor,"class":class,"skipped":"does not collide with/block players"}));
            }
            Ok(Some((v, shape, body_shape))) => {
                body_world.actors.push(BodyActor {
                    name: actor.clone(),
                    shape: body_shape,
                });
                static_world.actors.push(StaticActor {
                    name: actor.clone(),
                    shape,
                });
                supported += 1;
                for p in v["probes"].as_array().unwrap() {
                    if p["clear"] == true {
                        clear += 1;
                    } else {
                        blocked += 1;
                    }
                }
                actors.push(serde_json::json!({"actor":actor,"class":class,"result":v}));
            }
            Err(error) => {
                body_world.actors.push(BodyActor {
                    name: actor.clone(),
                    shape: BodyShape::Unsupported(error.clone()),
                });
                static_world.actors.push(StaticActor {
                    name: actor.clone(),
                    shape: StaticShape::Unsupported(error.clone()),
                });
                unsupported += 1;
                actors.push(serde_json::json!({"actor":actor,"class":class,"error":error}));
            }
        }
    }
    let mut world_probes = Vec::new();
    let mut incomplete_probes = 0;
    let mut body_probes = Vec::new();
    let mut incomplete_body_probes = 0;
    for start in &starts {
        for length in [20.0, 100.0, 500.0, 10000.0] {
            for axis in 0..3 {
                for direction in [-1.0, 1.0] {
                    let mut end = start.location;
                    end[axis] += direction * length;
                    for reverse in [false, true] {
                        let (a, b) = if reverse {
                            (end, start.location)
                        } else {
                            (start.location, end)
                        };
                        let result = static_world.line(a, b)?;
                        incomplete_probes += usize::from(!result.complete);
                        let body = body_world.sweep(a.map(f64::from), b.map(f64::from), extent)?;
                        incomplete_body_probes += usize::from(!body.complete);
                        body_probes.push(serde_json::json!({"start":start.actor,"axis":axis,"direction":direction,"length":length,"reverse":reverse,"result":body}));
                        world_probes.push(serde_json::json!({"start":start.actor,"axis":axis,"direction":direction,"length":length,"reverse":reverse,"result":result}));
                    }
                }
            }
        }
    }
    let mut movement_probes = Vec::new();
    let mut placement_probes = Vec::new();
    let mut grounded_probes = Vec::new();
    let mut step_probes = Vec::new();
    let mut movement_errors = 0;
    let mut physics_probes = Vec::new();
    let physics_options = rc_package::physics::PhysicsOptions {
        extent,
        gravity: 980.0,
        terminal_speed: 4000.0,
        skin: 0.5,
        support_distance: 2.0,
        step_height: 24.0,
        minimum_up: 0.7,
        max_iterations: 8,
    };
    let mut controller_probes = Vec::new();
    let movement_profile = rc_package::movement_profile::MovementProfile::read(&catalog)?;
    let loaded_volumes =
        volumes_load::load(&pkg, &bytes, &catalog, &source, movement_profile.physics)?;
    movement_errors += loaded_volumes.errors;
    let controller_options = movement_profile.controller;
    let mut input_modes = Vec::new();
    for yaw in [0, 16384] {
        for (mode, forward, strafe, walking) in [
            ("forward", 1.0, 0.0, false),
            ("back", -1.0, 0.0, false),
            ("side", 0.0, 1.0, false),
            ("walk", 1.0, 0.0, true),
        ] {
            let view = rc_package::controller::ViewInput {
                forward,
                strafe,
                yaw,
                walking,
                jump: false,
            };
            let world = view.world_input(movement_profile.ratios)?;
            input_modes.push(serde_json::json!({"mode":mode,"yaw":yaw,"input":world,"target_speed":world.direction[0].hypot(world.direction[1])*controller_options.speed}));
        }
    }
    let mut control_starts: Vec<_> = starts.iter().take(2).collect();
    for start in &starts {
        let selected = loaded_volumes.world.select(start.location)?;
        if selected.complete
            && selected.settings.gravity != loaded_volumes.world.default.gravity
            && !control_starts.iter().any(|s| s.actor == start.actor)
        {
            control_starts.push(start);
            if control_starts.len() >= 4 {
                break;
            }
        }
    }
    for start in control_starts {
        let mut state = rc_package::controller::ControlledBody {
            body: rc_package::physics::BodyState {
                position: start.location.map(f64::from),
                velocity: [0.0; 3],
                grounded: false,
            },
            jump_held: false,
        };
        let mut frames = Vec::new();
        let mut error = None;
        let start_volume = loaded_volumes.world.select(start.location)?;
        let tick_count = if start_volume.complete
            && start_volume.settings.gravity[2] > loaded_volumes.world.default.gravity[2]
        {
            720
        } else {
            300
        };
        for tick in 0..tick_count {
            let view = rc_package::controller::ViewInput {
                forward: if (120..150).contains(&tick) { 1.0 } else { 0.0 },
                strafe: if (120..150).contains(&tick) { 1.0 } else { 0.0 },
                yaw: start.rotation[1],
                walking: false,
                jump: (180..270).contains(&tick),
            };
            let input = view.world_input(movement_profile.ratios)?;
            match loaded_volumes.world.control_tick(
                &body_world,
                &state,
                input,
                1.0 / 60.0,
                controller_options,
                movement_profile.physics,
            ) {
                Ok((volume, frame)) => {
                    state = frame.state;
                    frames
                        .push(serde_json::json!({"volume":volume,"view_input":view,"input":input,"result":frame}));
                }
                Err(reason) => {
                    movement_errors += 1;
                    error = Some(reason);
                    break;
                }
            }
        }
        controller_probes.push(serde_json::json!({"start":start.actor,"frames":frames,"error":error,"requested_ticks":tick_count,"final_state":state}));
    }
    let mut walking_probes = Vec::new();
    let mut walking_properties = Vec::new();
    if walking_mode {
        let mut ratios = Vec::new();
        for name in ["CrouchSpeedRatio", "WoundedSpeedRatio"] {
            let resolved = catalog.resolve("CTCharacters.PlayerCommando", name, 0)?;
            let value = match &resolved.value {
                Value::Float(value) if value.is_finite() => f64::from(*value),
                _ => return Err(format!("invalid walking ratio {name}").into()),
            };
            ratios.push(value);
            walking_properties.push(serde_json::json!({"property":name,"resolved":resolved}));
        }
        for start in starts.iter().take(1) {
            let mut state = rc_package::physics::BodyState {
                position: start.location.map(f64::from),
                velocity: [0.0; 3],
                grounded: false,
            };
            let mut frames = Vec::new();
            let mut error = None;
            for tick in 0..300 {
                let volume = loaded_volumes
                    .world
                    .select(state.position.map(|v| v as f32))?;
                if !volume.complete {
                    error = Some("incomplete volume selection in walking diagnostic".to_string());
                    break;
                }
                let physics = match volume.settings.apply(movement_profile.physics) {
                    Ok(physics) => physics,
                    Err(reason) => {
                        error = Some(reason);
                        break;
                    }
                };
                let walking = (210..240).contains(&tick);
                let view = rc_package::controller::ViewInput {
                    forward: if (120..150).contains(&tick) || walking {
                        1.0
                    } else {
                        0.0
                    },
                    strafe: if (120..150).contains(&tick) { 1.0 } else { 0.0 },
                    yaw: start.rotation[1],
                    walking,
                    jump: false,
                };
                // Input-to-acceleration mapping remains an explicit diagnostic policy.
                let direction = view
                    .world_input(rc_package::controller::MovementRatios {
                        walk: 1.0,
                        back: 1.0,
                        side: 1.0,
                    })?
                    .direction;
                let acceleration = direction.map(|v| v * controller_options.acceleration);
                let options = rc_package::pawn_velocity::WalkingDiagnosticOptions {
                    base_acceleration: controller_options.acceleration,
                    base_speed: controller_options.speed,
                    maximum_desired_speed: controller_options.speed,
                    ground_friction: f64::from(volume.settings.ground_friction),
                    yaw: start.rotation[1],
                    factors: rc_package::pawn_velocity::SpeedFactors {
                        ratios: movement_profile.ratios,
                        crouch_ratio: ratios[0],
                        wounded_ratio: ratios[1],
                        walking,
                        crouched: false,
                        wounded: false,
                        weapon_modifier: 1.0,
                    },
                };
                match body_world.walking_diagnostic_tick(
                    &state,
                    acceleration,
                    1.0 / 60.0,
                    options,
                    physics,
                ) {
                    Ok(frame) => {
                        state = frame.motion.body;
                        frames.push(serde_json::json!({"volume":volume,"view_input":view,"acceleration":acceleration,"options":options,"result":frame}));
                    }
                    Err(reason) => {
                        error = Some(reason);
                        break;
                    }
                }
            }
            movement_errors += usize::from(error.is_some());
            walking_probes.push(serde_json::json!({"start":start.actor,"requested_ticks":300,"frames":frames,"error":error,"final_state":state}));
        }
    }
    for start in starts.iter().take(4) {
        let mut body = rc_package::physics::BodyState {
            position: start.location.map(f64::from),
            velocity: [0.0; 3],
            grounded: false,
        };
        let mut frames = Vec::new();
        let mut error = None;
        for tick in 0..180 {
            if tick == 120 {
                body.velocity[0] = 120.0;
            }
            if tick == 150 {
                body.velocity[0] = 0.0;
                body.velocity[1] = 0.0;
            }
            match body_world.body_tick(&body, 1.0 / 60.0, physics_options) {
                Ok(frame) => {
                    body = frame.body;
                    frames.push(frame);
                }
                Err(reason) => {
                    movement_errors += 1;
                    error = Some(reason);
                    break;
                }
            }
        }
        physics_probes.push(serde_json::json!({"start":start.actor,"frames":frames,"error":error,"final_body":body}));
    }
    for start in &starts {
        let position = start.location.map(f64::from);
        for lift in [0.0, 16.0, 84.0, 128.0] {
            let candidate = [position[0], position[1], position[2] + lift];
            let placement = body_world.body_placement(candidate, extent);
            movement_errors += usize::from(placement.is_err());
            placement_probes
                .push(serde_json::json!({"start":start.actor,"lift":lift,"result":placement}));
        }
        if body_world
            .body_placement(position, extent)
            .is_ok_and(|p| p.state != rc_package::movement::PlacementState::Penetrating)
        {
            let floor = body_world.floor_contact(position, extent, 1000.0, 0.7)?;
            if let Some(contact) = floor {
                let mut ground = position;
                ground[2] -= 1000.0 * contact.hit.fraction;
                let placement = body_world.body_placement(ground, extent);
                let movement = body_world.slide_body(ground, [128.0, 0.0, 0.0], extent, 0.5, 8);
                movement_errors += usize::from(placement.is_err() || movement.is_err());
                grounded_probes.push(serde_json::json!({"start":start.actor,"floor":contact,"placement":placement,"movement":movement}));
                for length in [128.0, 512.0] {
                    for axis in 0..2 {
                        for direction in [-1.0, 1.0] {
                            let mut velocity = [0.0; 3];
                            velocity[axis] = direction * length / 0.05;
                            let body = rc_package::physics::BodyState {
                                position: ground,
                                velocity,
                                grounded: true,
                            };
                            let result = body_world.body_tick(&body, 0.05, physics_options);
                            movement_errors += usize::from(result.is_err());
                            step_probes.push(serde_json::json!({"start":start.actor,"axis":axis,"direction":direction,"length":length,"result":result}));
                        }
                    }
                }
            }
        }
        for displacement in [
            [10000.0, 3000.0, 0.0],
            [-10000.0, -3000.0, 0.0],
            [0.0, 0.0, -1000.0],
        ] {
            let result =
                body_world.slide_body(start.location.map(f64::from), displacement, extent, 0.5, 8);
            let floor = match &result {
                Ok(movement)
                    if movement.state != rc_package::movement::MoveState::InitialContact =>
                {
                    Some(body_world.floor_contact(movement.position, extent, 10.0, 0.7))
                }
                _ => None,
            };
            movement_errors +=
                usize::from(result.is_err() || floor.as_ref().is_some_and(Result::is_err));
            movement_probes.push(serde_json::json!({"start":start.actor,"displacement":displacement,"result":result,"floor":floor}));
        }
    }
    if let Some(parent) = Path::new(&args[3]).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &args[3],
        serde_json::to_vec_pretty(
            &serde_json::json!({"map":source,"world_binding":binding,"starts":starts.len(),"actors":actors,"supported_actors":supported,"skipped_actors":skipped,"unsupported_actors":unsupported,"blocked_actor_probes":blocked,"clear_actor_probes":clear,"included_other_mesh_actor_classes":subclass_count,"unclassified_exports":unclassified,"incomplete_world_probes":incomplete_probes,"world_probes":world_probes,"body_probes":body_probes,"body_extent":extent,"incomplete_body_probes":incomplete_body_probes,"volume_actors":loaded_volumes.actors,"unidentified_volume_classes":loaded_volumes.unidentified,"volume_import_errors":loaded_volumes.errors,"movement_profile":movement_profile,"input_modes":input_modes,"controller_probes":controller_probes,"walking_diagnostic_probes":walking_probes,"walking_diagnostic_properties":walking_properties,"walking_diagnostic_scope":"Opt-in static PC bridge: first start, 300 ticks at 1/60; 120 passive fall/idle, 30 diagonal acceleration, 60 idle, 30 walking forward, 60 idle. Selected volume gravity/terminal/friction; SpeedFactor applied to acceleration limit and new velocity cap. Caller sets MaximumDesiredSpeed=GroundSpeed and weapon modifier1, healthy/uncrouched. Input acceleration mapping, inverse yaw matrix and body integration are diagnostic policies; no native Walking/Falling/SSE parity, ModifyVelocity, jump or Android integration. Airborne acceleration explicitly rejected.","controller_options":controller_options,"controller_scope":"Own PC yaw-relative directional policy with original class default properties; static PhysicsVolume selection recomputed at every tick start; crossing substeps/callbacks absent; first two starts plus up to two with different selected gravity, 300 ticks (720 for initially weaker gravity) at 1/60: 120 fall/idle, 30 diagonal command, 30 brake, 90 held jump, remaining 30 or 450 ticks released idle. Original GroundSpeed/AccelRate/DecelRate/AirControl/JumpZ/ratios/dimensions/Gravity/TerminalVelocity applied; MAXSTEPHEIGHT35/MINFLOORZ0.7 from Actor source; skin/support/iterations own policy. No native formula parity or Android integration.","step_probes":step_probes,"step_probe_scope":"Independent 0.05-second stress ticks from exact floor positions, horizontal displacements 128/512 in four directions; not normal gameplay speeds or continuous trajectories. Step height 24 is caller-selected.","physics_probes":physics_probes,"physics_options":physics_options,"physics_scope":"PC integration policy; first four starts per map, 180 fixed ticks at 1/60, 120 falling/30 horizontal command/30 idle. Gravity 980 and terminal speed 4000 are caller-selected diagnostic parameters, not decoded defaults; step height 24 is also a diagnostic parameter; conservative up/forward/down step attempts enabled; no native physics parity, friction, volumes or Android integration.","placement_probes":placement_probes,"grounded_probes":grounded_probes,"movement_probes":movement_probes,"movement_scope":"PC conservative displacement solver: skin 0.5, 8 iterations; touching starts allowed, penetrating starts refused; placement candidates lifted by 0/16/84/128 only diagnosed, not auto-selected; exact floor contact plus 128-unit horizontal movement tested; static AABB clipping/sliding and 10-unit downward support probes with minimum normal Z 0.7. No native Pawn physics, gravity, steps, spawn placement or Android integration. Result Err supplies no candidate position.","scope":"Level-referenced world Model plus Actor-derived objects with non-null StaticMesh at serialized pose; Boolean lines and mathematical player-sized AABB sweeps at stored Actor poses. Query completeness applies to classified mesh actors and the Level-referenced Model only; unclassified exports reported separately. No skeletal/cylinder actors, brush/mover simulation, native hit records/tolerances/backoff or movement. Complex/cylinder/unsupported Models rejected"}),
        )?,
    )?;
    println!("{supported} supported actors; {skipped} nonblocking; {unsupported} unsupported; {blocked} blocked/{clear} clear per-actor probes");
    if unsupported > 0
        || incomplete_probes > 0
        || incomplete_body_probes > 0
        || movement_errors > 0
        || static_world.world.is_err()
    {
        return Err("Mesh probes incomplete; see explicit errors in report".into());
    }
    Ok(())
}
