//! Extend the Bacta delivery with the selected original crash-site actors and route.
use rc_game::{bundle, crash, crash_runtime, dynamic, navigation, Point};
use rc_package::{
    properties::{self, Value},
    read_package,
};
use std::{env, fs, path::Path};
fn dist(a: Point, b: Point) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}
fn walk(game: &rc_game::Game, from: Point, to: Point) -> Result<bool, String> {
    let mut pawn = rc_package::controller::ControlledBody {
        body: rc_package::physics::BodyState {
            position: from,
            velocity: [0.; 3],
            grounded: true,
        },
        jump_held: false,
    };
    let runtime = rc_package::pawn_motion::PawnRuntimeOptions {
        maximum_desired_speed: game.profile.controller.speed,
        weapon_modifier: 1.,
        crouch_ratio: 1.,
        wounded_ratio: 1.,
        crouched: false,
        wounded: false,
        wants_to_crouch: false,
        current_jump_z: game.profile.controller.jump_speed,
    };
    let mut best = dist(from, to);
    let mut stalled = 0;
    for _ in 0..600 {
        let p = pawn.body.position;
        if dist(p, to) < 35. && pawn.body.grounded {
            return Ok(true);
        }
        let yaw = ((to[1] - p[1]).atan2(to[0] - p[0]) * 65536. / std::f64::consts::TAU) as i32;
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
                rc_game::STEP,
                &game.profile,
                runtime,
            )?
            .state;
        let d = dist(pawn.body.position, to);
        if d < best - 1. {
            best = d;
            stalled = 0;
        } else {
            stalled += 1;
        }
        if stalled > 45 {
            return Ok(false);
        }
    }
    Ok(false)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 5 {
        return Err("usage: rc-crash-build GAME_DATA BACTA_SCENE FLOW_DIR OUTPUT_DIR".into());
    }
    let root = Path::new(&args[1]);
    let source = Path::new(&args[3]);
    let out = Path::new(&args[4]);
    fs::create_dir_all(out)?;
    let old = fs::read(&args[2])?;
    let size = u32::from_le_bytes(old[8..12].try_into()?) as usize;
    if &old[..8] != b"RCPLAY04" {
        return Err("Requires original Bacta bundle".into());
    }
    let mut mission: rc_game::mission::Assets = serde_json::from_slice(&old[12..12 + size])?;
    let mut game = bundle::decode(&old)?;
    let flow: crash::Assets = serde_json::from_slice(&fs::read(source.join("crash-assets.json"))?)?;
    flow.validate()?;
    let map = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&map)?;
    let catalog = rc_inspect::defaults::load(root)?;
    let value = |id: u32, name: &str| -> Result<Value, String> {
        let e = &pkg.exports[id as usize - 1];
        let p = properties::read(&pkg, &map, e)?;
        Ok(catalog
            .instance(&pkg.object_path(e.class)?, &p, name, 0, "geo_01a.ctm")?
            .value)
    };
    let transform = |id| -> Result<rc_package::mesh_query::ActorTransform, String> {
        let vector = |name| match value(id, name)? {
            Value::Vector(v) => Ok(v),
            _ => Err("Not vector".to_string()),
        };
        let scalar = match value(id, "DrawScale")? {
            Value::Float(v) => v,
            _ => return Err("Not scale".into()),
        };
        Ok(rc_package::mesh_query::ActorTransform {
            location: vector("Location")?,
            rotation: match value(id, "Rotation")? {
                Value::Rotator(v) => v,
                _ => return Err("Not rotation".into()),
            },
            scale: vector("DrawScale3D")?.map(|v| v * scalar),
            pivot: vector("PrePivot")?,
        })
    };
    let capture = |name: String, h: &rc_package::hull::HullSet| bundle::Collider {
        name,
        hulls: h
            .hulls
            .iter()
            .map(|h| bundle::HullData {
                offset: h.offset,
                planes: h.planes.clone(),
                bounds: h.bounds,
            })
            .collect(),
    };
    let mut objects = vec![];
    for id in [flow.bacta_barrier, flow.exit_barrier] {
        let brush = match value(id, "Brush")? {
            Value::Object { index, .. } if index > 0 => index as usize - 1,
            _ => return Err("Missing blocking brush".into()),
        };
        let bsp = rc_package::geometry::read_bsp(&pkg, &map, &pkg.exports[brush])?;
        let solid =
            rc_package::collision::read_model_collision(&pkg, &map, &pkg.exports[brush], &bsp)?;
        let hulls = rc_package::hull::HullSet::read(&bsp, &solid)?.transformed(&transform(id)?)?;
        if hulls.hulls.is_empty() || hulls.missing_solid_leaves != 0 || hulls.empty_root_solid {
            return Err(format!("Incomplete original barrier {id}").into());
        }
        objects.push(dynamic::Object {
            released: None,
            original: id,
            collider: capture(format!("Original.{id}"), &hulls),
            snapshot: vec![],
        });
    }
    let index = game
        .world
        .actors
        .iter()
        .position(|a| a.name == "Prop0")
        .ok_or("Missing original path barricade collision")?;
    let actor = game.world.actors.remove(index);
    let collider = match actor.shape {
        rc_package::world_collision::BodyShape::Hulls(h) => capture(actor.name, &h),
        _ => return Err("Unsupported barricade shape".into()),
    };
    let (package, object) = match value(flow.path_barrier, "StaticMesh")? {
        Value::Object { path, .. } => {
            let (a, b) = path.split_once('.').ok_or("Bad mesh path")?;
            (a.to_owned(), b.to_owned())
        }
        _ => return Err("Missing barricade mesh".into()),
    };
    let data = fs::read(root.join("StaticMeshes").join(format!("{package}.usx")))?;
    let p = read_package(&data)?;
    let e = p
        .exports
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            p.object_path(i as i32 + 1)
                .is_ok_and(|s| s.eq_ignore_ascii_case(&object))
                .then_some(e)
        })
        .ok_or("Missing barricade export")?;
    let mesh = rc_package::mesh::read_static_mesh(&p, &data, e)?;
    let mut triangles = mesh.triangles()?;
    let t = transform(flow.path_barrier)?;
    for triangle in &mut triangles {
        triangle.points = triangle.points.map(|p| {
            rc_package::mesh::transform_point(p, t.location, t.rotation, t.scale, t.pivot)
        });
    }
    // This actor was explicitly omitted by the old static-scene builder. Bind that
    // fact before adding it as transient geometry, avoiding a permanent duplicate.
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read("analysis/scenes/geo_01a-textured/scene.json")?)?;
    if !report["omitted"]
        .as_array()
        .ok_or("No static-scene omission inventory")?
        .iter()
        .any(|a| a["actor"] == "Prop0")
    {
        return Err("Barricade may already be baked into static scene".into());
    }
    let mut resolver = rc_inspect::assets::Assets::new(&[
        &root.join("Textures"),
        &root.join("StaticMeshes"),
        &root.join("System"),
    ])?;
    let mut textures = vec![];
    let mut material_indices = vec![];
    let mut material_receipt = vec![];
    for material in &mesh.materials {
        let material = material.as_ref().ok_or("Missing barricade material")?;
        let path = resolver.qualify(&package, material);
        let diffuse = resolver.diffuse_at_time(&path, 256, 0.)?;
        material_receipt.push(serde_json::json!({
            "material": path, "diffuse": diffuse.source, "chain": diffuse.chain
        }));
        material_indices.push((textures.len(), diffuse.scale));
        textures.push(diffuse.texture);
    }
    fs::write(
        out.join("barrier-materials.json"),
        serde_json::to_vec_pretty(&material_receipt)?,
    )?;
    let mut uv = vec![];
    let stream = mesh
        .uv_streams
        .first()
        .ok_or("Missing barricade UV stream")?;
    for section in &mesh.sections {
        let first = section.first_index as usize;
        for face in mesh.indices[first..first + section.face_count as usize * 3].chunks_exact(3) {
            uv.push([
                stream[face[0] as usize],
                stream[face[1] as usize],
                stream[face[2] as usize],
            ]);
        }
    }
    let appearance = triangles
        .iter()
        .zip(&uv)
        .map(|(tri, uv)| {
            let (texture, scale) = material_indices[tri.surface];
            Some(rc_render::FaceTexture {
                texture,
                uv: uv.map(|v| v.map(|v| v * scale)),
            })
        })
        .collect();
    let geometry = triangles
        .into_iter()
        .map(|t| rc_package::geometry::Triangle {
            points: t.points,
            surface: t.surface,
        })
        .collect();
    let visible = rc_render::Scene {
        triangles: geometry,
        appearance,
        textures,
        center: game.scene.center,
        radius: game.scene.radius,
        starts: vec![],
        solid: None,
    };
    objects.push(dynamic::Object {
        released: None,
        original: flow.path_barrier,
        collider,
        snapshot: visible.snapshot()?,
    });
    // Build guidance on the freed world; runtime collision starts with all three
    // original barriers enabled and changes only through source-derived effects.
    let ground = |position: Point, extent: Point| -> Result<Point, String> {
        let start = [position[0], position[1], position[2] + 120.];
        let hit = game
            .world
            .floor_contact(start, extent, 640., 0.7)?
            .ok_or("No supported original anchor floor")?;
        Ok([start[0], start[1], start[2] - 640. * hit.hit.fraction + 0.5])
    };
    let mut spawns = vec![];
    for f in &flow.factories {
        spawns.push(ground(f.position, [22., 22., 60.])?);
    }
    let start = *mission.route.last().unwrap();
    let mut nodes = vec![navigation::Node {
        original: 737,
        name: "BactaStand".into(),
        position: start,
        extra_cost: 0.,
    }];
    let mut targets = vec![];
    for (i, e) in pkg.exports.iter().enumerate() {
        let id = i as u32 + 1;
        let class = pkg.object_path(e.class)?;
        if class != "Engine.PathNode" && ![128, 608].contains(&id) {
            continue;
        }
        let pos = match value(id, "Location")? {
            Value::Vector(v) => v.map(f64::from),
            _ => continue,
        };
        if !(-3300. ..=1300.).contains(&pos[0]) || !(-5400. ..=800.).contains(&pos[1]) {
            continue;
        }
        let Ok(pos) = ground(pos, game.profile.physics.extent) else {
            continue;
        };
        if game
            .world
            .body_placement(pos, game.profile.physics.extent)?
            .state
            == rc_package::movement::PlacementState::Penetrating
        {
            continue;
        }
        if [69, 128, 608, 294].contains(&id) {
            targets.push((id, nodes.len()));
        }
        nodes.push(navigation::Node {
            original: id as i32,
            name: pkg.object_path(id as i32)?,
            position: pos,
            extra_cost: 0.,
        });
    }
    let mut edges = vec![];
    println!("Checking {} original route anchors", nodes.len());
    for i in 0..nodes.len() {
        for j in i + 1..nodes.len() {
            let d = dist(nodes[i].position, nodes[j].position);
            if d > 1000. {
                continue;
            }
            let hit = game.world.sweep_motion(
                nodes[i].position,
                nodes[j].position,
                game.profile.physics.extent,
            )?;
            if !hit.complete {
                return Err("Incomplete route collision".into());
            }
            if !hit.contacts.is_empty() && d > 700. {
                continue;
            }
            let mut supported = hit.contacts.is_empty();
            if supported {
                let steps = (d / 48.).ceil().max(1.) as usize;
                for step in 0..=steps {
                    let p = std::array::from_fn(|axis| {
                        nodes[i].position[axis]
                            + (nodes[j].position[axis] - nodes[i].position[axis]) * step as f64
                                / steps as f64
                    });
                    if game
                        .world
                        .floor_contact(
                            p,
                            game.profile.physics.extent,
                            game.profile.physics.step_height + 4.,
                            0.7,
                        )?
                        .is_none()
                    {
                        supported = false;
                        break;
                    }
                }
            }
            for (s, e) in [(i, j), (j, i)] {
                if supported || (d < 700. && walk(&game, nodes[s].position, nodes[e].position)?) {
                    edges.push(navigation::Edge {
                        original: 0,
                        start: s,
                        end: e,
                        distance: d,
                    });
                }
            }
        }
    }
    let graph = navigation::Assets {
        provenance: serde_json::Value::Null,
        nodes,
        edges,
        lunge_distance: 1.,
        jump_distance: 1.,
        offensive_jump_probability: 0.,
    };
    fs::write(
        out.join("route-graph.json"),
        serde_json::to_vec_pretty(&graph)?,
    )?;
    let mut route = vec![];
    let mut last = 0;
    for id in [69, 128, 608, 294] {
        let end = targets
            .iter()
            .find(|(i, _)| *i == id)
            .ok_or(format!("Unsupported original tracker/trigger {id}"))?
            .1;
        let path = graph
            .shortest(&[(last, 0.)], &[(end, 0.)])
            .ok_or(format!("No physical path to original actor {id}"))?;
        route.extend(path.iter().skip(1).map(|&i| graph.nodes[i].position));
        last = end;
    }
    println!(
        "Crash-site route: {} anchors, {} links, {} guidance points",
        graph.nodes.len(),
        graph.edges.len(),
        route.len()
    );
    // Original TPRG1003 is the only extra line in this bounded follow-on host.
    let voice = fs::read(root.join("Sounds/GEO_Voice.uax"))?;
    let vp = read_package(&voice)?;
    let e = vp
        .exports
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            vp.object_path(i as i32 + 1)
                .is_ok_and(|p| p == "GEO_01.TPRG1003")
                .then_some(e)
        })
        .ok_or("Missing retreat line")?;
    let payload = vp.payload(&voice, e)?;
    let at = payload
        .windows(4)
        .position(|w| w == b"RIFF")
        .ok_or("Missing RIFF")?;
    let n = u32::from_le_bytes(payload[at + 4..at + 8].try_into()?) as usize + 8;
    mission.sounds.push(rc_game::combat::Sound {
        source: "GEO_Voice.GEO_01.TPRG1003".into(),
        wave: payload.get(at..at + n).ok_or("Short wave")?.to_vec(),
    });
    mission.crash = Some(crash_runtime::Assets {
        flow,
        objects,
        route,
        spawns,
    });
    let manifest = bundle::Manifest::capture(
        &game,
        serde_json::json!({"source":"Original Bacta scene plus selected removable barriers and BattleDroid factories","combat":"Provisional hitscan/approach/shooting adapter; no native BattleDroid AI or DC15s damage parity"}),
    )?;
    let base = bundle::encode(&manifest, &game.scene)?;
    let visuals: rc_game::visual::Assets =
        serde_json::from_slice(&fs::read(source.join("visuals/visual-assets.json"))?)?;
    let base = bundle::encode_animated(&base, &visuals)?;
    let metadata = serde_json::to_vec(&mission)?;
    let mut result = b"RCPLAY04".to_vec();
    result.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    result.extend(metadata);
    result.extend(base);
    let checked = bundle::decode(&result)?;
    fs::write(
        out.join("mission-receipt.json"),
        serde_json::to_vec_pretty(&mission)?,
    )?;
    fs::write(out.join("geo_01a-crash.rcplay"), result)?;
    println!(
        "Built contiguous original objective0/1 section: {}",
        checked.mission_status()
    );
    Ok(())
}
