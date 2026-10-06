use rc_package::{
    controller::{ControlledBody, ViewInput},
    defaults::Catalog,
    level::PlayerStart,
    movement_profile::MovementProfile,
    pawn_motion::PawnRuntimeOptions,
    physics::BodyState,
    properties::Value,
    script_motion::{ScriptBody, ScriptMotionInput},
    volumes::VolumeWorld,
    world_collision::StaticBodyWorld,
};
pub struct PawnProbes {
    pub probes: Vec<serde_json::Value>,
    pub properties: Vec<serde_json::Value>,
    pub errors: usize,
}
pub struct AiProbePolicy<'a> {
    pub prefer_duck_targets: bool,
    pub wall_classes: &'a std::collections::BTreeMap<String, String>,
}
pub fn ai_contact_cycles(
    catalog: &Catalog,
    report: &PawnProbes,
    world: &StaticBodyWorld,
    volumes: &VolumeWorld,
    profile: &MovementProfile,
    shape: &rc_package::crouch::CrouchProfile,
    policy: AiProbePolicy<'_>,
) -> Result<Vec<serde_json::Value>, String> {
    use rc_package::{
        ai_contact::{AiContactOptions, AiControllerSnapshot, WallEventSnapshot},
        crouch_motion::{CrouchMotionOptions, CrouchPhysicsInput, CrouchingScriptBody},
    };
    use std::collections::BTreeMap;
    let prefer_duck_targets = policy.prefer_duck_targets;
    let mut wall_snapshots = BTreeMap::from([(
        "World.BSP".to_string(),
        WallEventSnapshot {
            excluded_wall_class: false,
            notify_handled: false,
        },
    )]);
    let mut wall_metadata = Vec::new();
    for actor in &world.actors {
        let class = policy
            .wall_classes
            .get(&actor.name)
            .ok_or_else(|| format!("missing wall actor class: {}", actor.name))?;
        let snapshot = WallEventSnapshot::from_actor_class(catalog, class, false)?;
        wall_metadata.push(serde_json::json!({"object":actor.name,"class":class,"excluded_wall_class":snapshot.excluded_wall_class,"notify_handled":snapshot.notify_handled,"notify_source":"supplied diagnostic false"}));
        wall_snapshots.insert(actor.name.clone(), snapshot);
    }
    let ratio = |name: &str| -> Result<f64, String> {
        match catalog
            .resolve("CTCharacters.PlayerCommando", name, 0)?
            .value
        {
            Value::Float(v) if v.is_finite() => Ok(f64::from(v)),
            _ => Err(format!("invalid {name}")),
        }
    };
    let can_crouch = match catalog
        .resolve("CTCharacters.PlayerCommando", "bCanCrouch", 0)?
        .value
    {
        Value::Bool(v) => v,
        _ => return Err("invalid bCanCrouch".into()),
    };
    let runtime = PawnRuntimeOptions {
        maximum_desired_speed: profile.controller.speed,
        weapon_modifier: 1.0,
        crouch_ratio: ratio("CrouchSpeedRatio")?,
        wounded_ratio: ratio("WoundedSpeedRatio")?,
        crouched: false,
        wounded: false,
        wants_to_crouch: false,
        current_jump_z: profile.controller.jump_speed,
    };
    let mut probes = Vec::new();
    for probe in &report.probes {
        if probe["completed"] != true || !probe["error"].is_null() {
            return Err("AI contact probe requires completed script endpoint".into());
        }
        let point = probe["final_state"]["body"]["position"]
            .as_array()
            .ok_or("missing script endpoint")?;
        if point.len() != 3 {
            return Err("invalid endpoint vector".into());
        }
        let position = [
            point[0].as_f64().ok_or("X")?,
            point[1].as_f64().ok_or("Y")?,
            point[2].as_f64().ok_or("Z")?,
        ];
        let initial = CrouchingScriptBody {
            script: ScriptBody {
                body: BodyState {
                    position,
                    velocity: [0.0; 3],
                    grounded: true,
                },
                pressed_jump: false,
                wants_to_crouch: false,
            },
            crouched: false,
            try_to_uncrouch: false,
            uncrouch_time: 0.0,
        };
        let mut searches = Vec::new();
        let mut chosen: Option<(bool, f64, i32, [f64; 3], String)> = None;
        let directions = if prefer_duck_targets { 32 } else { 8 };
        let range = if prefer_duck_targets { 1500.0 } else { 600.0 };
        let advance_budget = if prefer_duck_targets { 300 } else { 120 };
        for yaw in (0..directions).map(|i| i * (65536 / directions)) {
            let angle = f64::from(yaw) * std::f64::consts::TAU / 65536.0;
            let end = [
                position[0] + range * angle.cos(),
                position[1] + range * angle.sin(),
                position[2],
            ];
            let sweep = world.sweep_motion(position, end, shape.standing)?;
            if !sweep.complete {
                return Err("AI contact target search incomplete".into());
            }
            let mut predictions = Vec::new();
            for c in sweep
                .contacts
                .iter()
                .take(if prefer_duck_targets { 1 } else { usize::MAX })
            {
                let approach = (0..3)
                    .map(|i| c.hit.normal[i] * (end[i] - position[i]))
                    .sum::<f64>();
                if !c.hit.start_overlapping
                    && c.hit.normal[2].abs() < profile.physics.minimum_up
                    && approach < 0.0
                {
                    let mut predicted_armed = false;
                    if prefer_duck_targets {
                        let travel =
                            (c.hit.fraction - profile.physics.skin / (-approach)).clamp(0.0, 1.0);
                        let impact =
                            std::array::from_fn(|i| position[i] + (end[i] - position[i]) * travel);
                        let mut snapshot = initial;
                        snapshot.script.body.position = impact;
                        let probe_end = [
                            impact[0] + shape.standing[0] * angle.cos(),
                            impact[1] + shape.standing[0] * angle.sin(),
                            impact[2],
                        ];
                        let prediction = world.can_crouch_walk_diagnostic(
                            &snapshot, impact, probe_end, can_crouch, shape,
                        )?;
                        predicted_armed = prediction.decision
                            == rc_package::auto_crouch::AutoCrouchDecision::Armed;
                        predictions.push(serde_json::json!({"object":c.object,"impact":impact,"result":prediction}));
                    }
                    if chosen.as_ref().is_none_or(|v| {
                        (predicted_armed && !v.0)
                            || (predicted_armed == v.0 && c.hit.fraction < v.1)
                    }) {
                        chosen =
                            Some((predicted_armed, c.hit.fraction, yaw, end, c.object.clone()));
                    }
                }
            }
            searches.push(
                serde_json::json!({"yaw":yaw,"end":end,"query":sweep,"predictions":predictions}),
            );
        }
        let Some((predicted_armed, fraction, yaw, destination, object)) = chosen else {
            probes.push(serde_json::json!({"start":probe["start"],"searches":searches,"initial_state":initial,"frames":[],"error":null,"outcome":"NoTarget","final_state":initial}));
            continue;
        };
        let controller = AiControllerSnapshot {
            controller_present: true,
            direct_hit_wall: false,
            human_controlled: false,
            destination,
            minimum_hit_wall: 0.0,
            walls: wall_snapshots.clone(),
        };
        let mut state = initial;
        let mut frames = Vec::new();
        let mut error = None;
        let mut contacted = false;
        let mut idle = 0;
        for tick in 0..advance_budget + 90 {
            let phase = if contacted || tick >= advance_budget {
                "idle"
            } else {
                "advance"
            };
            let input = CrouchPhysicsInput {
                view: ViewInput {
                    forward: if phase == "advance" { 1.0 } else { 0.0 },
                    strafe: 0.0,
                    yaw,
                    walking: false,
                    jump: false,
                },
                can_crouch,
            };
            match volumes.ai_crouching_contact_tick(
                world,
                &state,
                input,
                1.0 / 60.0,
                profile,
                AiContactOptions {
                    motion: CrouchMotionOptions { runtime, shape },
                    controller: &controller,
                },
            ) {
                Ok(frame) => {
                    contacted |= frame.dispatch.is_some();
                    state = frame.state;
                    frames.push(serde_json::json!({"phase":phase,"tick":tick,"result":frame}));
                    if phase == "idle" {
                        idle += 1;
                    }
                    if idle >= 90 {
                        break;
                    }
                }
                Err(reason) => {
                    error = Some(reason);
                    break;
                }
            }
        }
        let outcome = if error.is_some() {
            "Error"
        } else if !contacted {
            "NoContact"
        } else if state.crouched {
            "ContactStillCrouched"
        } else {
            "ContactStanding"
        };
        probes.push(serde_json::json!({"start":probe["start"],"searches":searches,"wall_metadata":wall_metadata,"target":{"yaw":yaw,"destination":destination,"search_fraction":fraction,"object":object,"predicted_armed":predicted_armed},"initial_state":initial,"runtime":runtime,"frames":frames,"error":error,"outcome":outcome,"final_state":state}));
    }
    Ok(probes)
}
pub fn run(
    catalog: &Catalog,
    starts: &[PlayerStart],
    volumes: &VolumeWorld,
    world: &StaticBodyWorld,
    profile: &MovementProfile,
) -> Result<PawnProbes, String> {
    run_common(catalog, starts, volumes, world, profile, false)
}
pub fn run_script(
    catalog: &Catalog,
    starts: &[PlayerStart],
    volumes: &VolumeWorld,
    world: &StaticBodyWorld,
    profile: &MovementProfile,
) -> Result<PawnProbes, String> {
    run_common(catalog, starts, volumes, world, profile, true)
}
fn run_common(
    catalog: &Catalog,
    starts: &[PlayerStart],
    volumes: &VolumeWorld,
    world: &StaticBodyWorld,
    profile: &MovementProfile,
    script: bool,
) -> Result<PawnProbes, String> {
    let mut properties = Vec::new();
    let mut ratios = Vec::new();
    for name in ["CrouchSpeedRatio", "WoundedSpeedRatio"] {
        let resolved = catalog.resolve("CTCharacters.PlayerCommando", name, 0)?;
        let value = match &resolved.value {
            Value::Float(v) if v.is_finite() => f64::from(*v),
            _ => return Err(format!("invalid pawn ratio {name}")),
        };
        ratios.push(value);
        properties.push(serde_json::json!({"name":name,"resolved":resolved}));
    }
    let can_crouch = if script {
        let resolved = catalog.resolve("CTCharacters.PlayerCommando", "bCanCrouch", 0)?;
        let value = match resolved.value {
            Value::Bool(v) => v,
            _ => return Err("invalid bCanCrouch".into()),
        };
        properties.push(serde_json::json!({"name":"bCanCrouch","resolved":resolved}));
        value
    } else {
        true
    };
    let runtime = PawnRuntimeOptions {
        maximum_desired_speed: profile.controller.speed,
        weapon_modifier: 1.0,
        crouch_ratio: ratios[0],
        wounded_ratio: ratios[1],
        crouched: false,
        wounded: false,
        wants_to_crouch: false,
        current_jump_z: profile.controller.jump_speed,
    };
    let mut selected: Vec<_> = starts.iter().take(1).collect();
    for start in starts {
        let v = volumes.select(start.location)?;
        if v.complete
            && v.settings.gravity != volumes.default.gravity
            && !selected.iter().any(|s| s.actor == start.actor)
        {
            selected.push(start);
            break;
        }
    }
    let mut probes = Vec::new();
    let mut errors = 0;
    for start in selected {
        let volume = volumes.select(start.location)?;
        let budget = if volume.settings.gravity[2] > volumes.default.gravity[2] {
            1800
        } else {
            600
        };
        let mut state = ControlledBody {
            body: BodyState {
                position: start.location.map(f64::from),
                velocity: [0.0; 3],
                grounded: false,
            },
            jump_held: false,
        };
        let mut script_state = ScriptBody {
            body: state.body,
            pressed_jump: false,
            wants_to_crouch: false,
        };
        let mut phase = "settle";
        let mut age = 0;
        let mut stable = 0;
        let mut jumps = 0;
        let mut completed = false;
        let mut error = None;
        let mut frames = Vec::new();
        for _ in 0..budget {
            let view = ViewInput {
                forward: if phase == "move" { 1.0 } else { 0.0 },
                strafe: if phase == "move" { 1.0 } else { 0.0 },
                yaw: start.rotation[1],
                walking: false,
                jump: matches!(
                    phase,
                    "first_jump" | "first_flight" | "held_idle" | "second_jump" | "second_flight"
                ),
            };
            let outcome = if script {
                let mut axes = view;
                axes.jump = false;
                let event = ScriptMotionInput {
                    view: axes,
                    jump_event: matches!(phase, "first_jump" | "second_jump"),
                    cannot_jump_now: false,
                    duck: 0,
                    can_crouch,
                };
                volumes
                    .script_motion_tick(world, &script_state, event, 1.0 / 60.0, profile, runtime)
                    .map(|frame| {
                        script_state = frame.state;
                        (
                            ControlledBody {
                                body: frame.state.body,
                                jump_held: view.jump,
                            },
                            frame.jump_started,
                            serde_json::json!({"phase":phase,"input":event,"result":frame}),
                        )
                    })
            } else {
                volumes
                    .pawn_diagnostic_tick(world, &state, view, 1.0 / 60.0, profile, runtime)
                    .map(|frame| {
                        (
                            frame.state,
                            frame.jump_started,
                            serde_json::json!({"phase":phase,"input":view,"result":frame}),
                        )
                    })
            };
            match outcome {
                Ok((next_state, jumped, record)) => {
                    state = next_state;
                    jumps += usize::from(jumped);
                    frames.push(record);
                    age += 1;
                    let stopped =
                        state.body.grounded && state.body.velocity.iter().all(|v| v.abs() < 1e-9);
                    let next = match phase {
                        "settle" if state.body.grounded => Some("move"),
                        "move" if age >= 30 => Some("brake"),
                        "brake" if stopped => Some("first_jump"),
                        "first_jump" if jumped => Some("first_flight"),
                        "first_flight" if state.body.grounded => Some("held_idle"),
                        "held_idle" if age >= 30 => Some("release"),
                        "release" => Some("second_jump"),
                        "second_jump" if jumped => Some("second_flight"),
                        "second_flight" if state.body.grounded => Some("final_idle"),
                        _ => None,
                    };
                    if phase == "final_idle" {
                        if stopped {
                            stable += 1;
                        } else {
                            stable = 0;
                        }
                        if stable >= 30 {
                            completed = true;
                            break;
                        }
                    }
                    if let Some(next) = next {
                        phase = next;
                        age = 0;
                    }
                }
                Err(reason) => {
                    error = Some(reason);
                    break;
                }
            }
        }
        if error.is_none() && (!completed || jumps != 2) {
            error =
                Some("pawn diagnostic cycle did not complete with exactly two jumps".to_string());
        }
        errors += usize::from(error.is_some());
        let final_state = if script {
            serde_json::to_value(script_state).map_err(|e| e.to_string())?
        } else {
            serde_json::to_value(state).map_err(|e| e.to_string())?
        };
        probes.push(serde_json::json!({"start":start.actor,"requested_tick_budget":budget,"frames":frames,"runtime":runtime,"error":error,"completed":completed,"jump_count":jumps,"final_state":final_state}));
    }
    Ok(PawnProbes {
        probes,
        properties,
        errors,
    })
}

/// Isolated shape cycles at completed script movement endpoints; no crouch tick timing.
pub fn crouch_round_trips(
    world: &StaticBodyWorld,
    report: &PawnProbes,
    profile: &rc_package::crouch::CrouchProfile,
) -> Result<Vec<serde_json::Value>, String> {
    use rc_package::crouch::{CrouchBody, CrouchChange};
    let mut probes = Vec::new();
    for probe in &report.probes {
        if probe["completed"] != true || !probe["error"].is_null() {
            return Err("crouch diagnostic requires completed movement endpoint".into());
        }
        let body = &probe["final_state"]["body"];
        let vector = |name: &str| -> Result<[f64; 3], String> {
            let values = body[name].as_array().ok_or("missing final body vector")?;
            if values.len() != 3 {
                return Err("invalid final body vector".into());
            }
            Ok([
                values[0].as_f64().ok_or("invalid vector X")?,
                values[1].as_f64().ok_or("invalid vector Y")?,
                values[2].as_f64().ok_or("invalid vector Z")?,
            ])
        };
        let initial = CrouchBody {
            body: BodyState {
                position: vector("position")?,
                velocity: vector("velocity")?,
                grounded: body["grounded"]
                    .as_bool()
                    .ok_or("missing final grounded flag")?,
            },
            crouched: false,
        };
        let mut state = initial;
        let mut frames = Vec::new();
        for _ in 0..10 {
            for want in [true, false] {
                let frame = world.crouch_diagnostic(&state, want, profile)?;
                if frame.change != CrouchChange::Changed {
                    return Err("original endpoint shape cycle blocked".into());
                }
                state = frame.state;
                frames.push(serde_json::to_value(frame).map_err(|e| e.to_string())?);
            }
        }
        probes.push(serde_json::json!({"start":probe["start"],"initial_state":initial,"frames":frames,"final_state":state}));
    }
    Ok(probes)
}

pub fn crouch_motion_cycles(
    catalog: &Catalog,
    report: &PawnProbes,
    world: &StaticBodyWorld,
    volumes: &VolumeWorld,
    profile: &MovementProfile,
    shape: &rc_package::crouch::CrouchProfile,
) -> Result<Vec<serde_json::Value>, String> {
    use rc_package::crouch_motion::{CrouchMotionOptions, CrouchingScriptBody};
    let number = |name: &str| -> Result<f64, String> {
        match catalog
            .resolve("CTCharacters.PlayerCommando", name, 0)?
            .value
        {
            Value::Float(v) if v.is_finite() => Ok(f64::from(v)),
            _ => Err(format!("invalid {name}")),
        }
    };
    let can_crouch = match catalog
        .resolve("CTCharacters.PlayerCommando", "bCanCrouch", 0)?
        .value
    {
        Value::Bool(v) => v,
        _ => return Err("invalid bCanCrouch".into()),
    };
    let runtime = PawnRuntimeOptions {
        maximum_desired_speed: profile.controller.speed,
        weapon_modifier: 1.0,
        crouch_ratio: number("CrouchSpeedRatio")?,
        wounded_ratio: number("WoundedSpeedRatio")?,
        crouched: false,
        wounded: false,
        wants_to_crouch: false,
        current_jump_z: profile.controller.jump_speed,
    };
    let mut probes = Vec::new();
    for probe in &report.probes {
        if probe["completed"] != true || !probe["error"].is_null() {
            return Err("crouch motion requires completed endpoint".into());
        }
        let point = probe["final_state"]["body"]["position"]
            .as_array()
            .ok_or("missing endpoint")?;
        if point.len() != 3 {
            return Err("invalid endpoint".into());
        }
        let initial = CrouchingScriptBody {
            script: ScriptBody {
                body: BodyState {
                    position: [
                        point[0].as_f64().ok_or("X")?,
                        point[1].as_f64().ok_or("Y")?,
                        point[2].as_f64().ok_or("Z")?,
                    ],
                    velocity: [0.0; 3],
                    grounded: true,
                },
                pressed_jump: false,
                wants_to_crouch: false,
            },
            crouched: false,
            try_to_uncrouch: false,
            uncrouch_time: 0.0,
        };
        let mut state = initial;
        let mut phase = "duck_move";
        let mut age = 0;
        let mut completed = false;
        let mut frames = Vec::new();
        for _ in 0..1200 {
            let input = ScriptMotionInput {
                view: ViewInput {
                    forward: if phase == "duck_move" { 1.0 } else { 0.0 },
                    strafe: 0.0,
                    yaw: 0,
                    walking: false,
                    jump: false,
                },
                jump_event: false,
                cannot_jump_now: false,
                duck: u8::from(matches!(phase, "duck_move" | "duck_brake")),
                can_crouch,
            };
            let frame = volumes.crouching_script_tick(
                world,
                &state,
                input,
                1.0 / 60.0,
                profile,
                CrouchMotionOptions { runtime, shape },
            )?;
            state = frame.state;
            age += 1;
            frames.push(serde_json::json!({"phase":phase,"input":input,"result":frame}));
            let stopped = state.script.body.grounded
                && state.script.body.velocity.iter().all(|v| v.abs() < 1e-9);
            if phase == "duck_move" && age >= 30 {
                phase = "duck_brake";
                age = 0;
            } else if phase == "duck_brake" && stopped {
                phase = "release";
                age = 0;
            } else if phase == "release" {
                phase = "stand_idle";
                age = 0;
            } else if phase == "stand_idle" && age >= 30 && stopped && !state.crouched {
                completed = true;
                break;
            }
        }
        if !completed {
            return Err("crouch motion cycle did not finish standing at rest".into());
        }
        probes.push(
            serde_json::json!({"start":probe["start"],"initial_state":initial,"runtime":runtime,
            "frames":frames,"completed":completed,"final_state":state}),
        );
    }
    Ok(probes)
}
