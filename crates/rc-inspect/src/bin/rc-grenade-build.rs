//! Attach selected original connecting scripts and pickup to the corrected crash scene.
use rc_game::{bundle, dynamic, grenade_pickup, grenade_runtime};
use rc_inspect::assets::Assets as Resolver;
use rc_package::{
    hull::HullSet,
    mesh_query::ActorTransform,
    properties::{self, Value},
    read_package,
};
use std::{env, fs, path::Path};
fn collider(name: String, h: &HullSet) -> bundle::Collider {
    bundle::Collider {
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
    }
}
fn sound(
    root: &Path,
    path: &str,
) -> Result<(rc_game::combat::Sound, f64), Box<dyn std::error::Error>> {
    let (package, object) = path.split_once('.').ok_or("Bad audio path")?;
    let bytes = fs::read(root.join("Sounds").join(format!("{package}.uax")))?;
    let p = read_package(&bytes)?;
    let e = p
        .exports
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            p.object_path(i as i32 + 1)
                .is_ok_and(|s| s.eq_ignore_ascii_case(object))
                .then_some(e)
        })
        .ok_or("Missing sound")?;
    let payload = p.payload(&bytes, e)?;
    let at = payload
        .windows(4)
        .position(|w| w == b"RIFF")
        .ok_or("No RIFF")?;
    let n = u32::from_le_bytes(payload[at + 4..at + 8].try_into()?) as usize + 8;
    let wave = payload.get(at..at + n).ok_or("Truncated wave")?.to_vec();
    rc_game::combat::validate_wave(&wave)?;
    let (mut at, mut rate, mut size) = (12, None, None);
    while at + 8 <= wave.len() {
        let n = u32::from_le_bytes(wave[at + 4..at + 8].try_into()?) as usize;
        let chunk = wave.get(at + 8..at + 8 + n).ok_or("Short WAV chunk")?;
        match &wave[at..at + 4] {
            b"fmt " => rate = Some(u32::from_le_bytes(chunk[8..12].try_into()?) as f64),
            b"data" => size = Some(n as f64),
            _ => {}
        }
        at += 8 + n + (n & 1);
    }
    Ok((
        rc_game::combat::Sound {
            source: path.into(),
            wave,
        },
        size.ok_or("No samples")? / rate.ok_or("No rate")?,
    ))
}
fn mesh_scene(
    root: &Path,
    path: &str,
    t: &ActorTransform,
    catalog: &rc_package::defaults::Catalog,
    receipts: &mut Vec<serde_json::Value>,
) -> Result<rc_render::Scene, Box<dyn std::error::Error>> {
    let (package, object) = path.split_once('.').ok_or("Bad mesh path")?;
    let bytes = fs::read(root.join("StaticMeshes").join(format!("{package}.usx")))?;
    let p = read_package(&bytes)?;
    let e = p
        .exports
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            p.object_path(i as i32 + 1)
                .is_ok_and(|s| s.eq_ignore_ascii_case(object))
                .then_some(e)
        })
        .ok_or("Missing wall mesh")?;
    let mesh = rc_package::mesh::read_static_mesh(&p, &bytes, e)?;
    let mut triangles = mesh.triangles()?;
    for tri in &mut triangles {
        tri.points = tri.points.map(|p| {
            rc_package::mesh::transform_point(p, t.location, t.rotation, t.scale, t.pivot)
        });
    }
    let mut scene = rc_render::Scene::new(
        triangles
            .into_iter()
            .map(|t| rc_package::geometry::Triangle {
                points: t.points,
                surface: t.surface,
            })
            .collect(),
    )?;
    let mut resolver = Resolver::new(&[
        &root.join("Textures"),
        &root.join("StaticMeshes"),
        &root.join("System"),
    ])?;
    let mut bindings = vec![];
    for m in &mesh.materials {
        let original = resolver.qualify(package, m.as_ref().ok_or("No wall material")?);
        let mut source = original.clone();
        let mut channel = 0;
        let mut opaque = false;
        let mut chain = vec![];
        for _ in 0..16 {
            let (class, props) = resolver.material_properties(&source)?;
            chain.push(source.clone());
            let field = match class.as_str() {
                "Engine.TexCoordSource" => {
                    channel = match catalog
                        .instance(&class, &props, "SourceChannel", 0, &source)?
                        .value
                    {
                        Value::Int(v) if v >= 0 => v as usize,
                        _ => return Err("Bad UV channel".into()),
                    };
                    "Material"
                }
                "Engine.Shader" => {
                    let opacity = catalog
                        .instance(&class, &props, "Opacity", 0, &source)?
                        .value;
                    let blend = catalog
                        .instance(&class, &props, "OutputBlending", 0, &source)?
                        .value;
                    opaque = matches!(opacity, Value::Object { index: 0, .. })
                        && matches!(blend, Value::Byte(0));
                    "Diffuse"
                }
                "Engine.FinalBlend" => "Material",
                "Engine.Combiner" => match catalog
                    .instance(&class, &props, "CombineOperation", 0, &source)?
                    .value
                {
                    Value::Byte(1) => "Material2",
                    Value::Byte(7) => "Mask",
                    _ => "Material1",
                },
                _ => break,
            };
            let path = match catalog.instance(&class, &props, field, 0, &source)?.value {
                Value::Object { path, index } if index != 0 => path,
                _ => return Err("Missing wall material layer".into()),
            };
            let package = source.split_once('.').ok_or("Bad layer path")?.0;
            source = resolver.qualify(package, &path);
        }
        let mut d = resolver.diffuse_at_time(&source, 256, 0.)?;
        if opaque {
            for p in &mut d.texture.pixels {
                *p |= 0xff000000;
            }
        }
        receipts.push(serde_json::json!({"mesh":path,"material":original,"diffuse":d.source,"channel":channel,"opaque":opaque,"chain":chain}));
        bindings.push((channel, d.scale));
        scene.textures.push(d.texture);
    }
    let mut face = 0;
    for section in &mesh.sections {
        let first = section.first_index as usize;
        for ids in mesh.indices[first..first + section.face_count as usize * 3].chunks_exact(3) {
            let material = scene.triangles[face].surface;
            let (channel, scale) = bindings[material];
            let uv = mesh
                .uv_streams
                .get(channel)
                .ok_or("Missing source UV channel")?;
            scene.appearance.push(Some(rc_render::FaceTexture {
                texture: material,
                uv: std::array::from_fn(|i| uv[ids[i] as usize].map(|v| v * scale)),
            }));
            face += 1;
        }
    }
    if face != scene.triangles.len() {
        return Err("Incomplete wall textures".into());
    }
    Ok(scene)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 6 {
        return Err(
            "usage: rc-grenade-build GAME_DATA CRASH_SCENE PICKUP_ASSETS ROUTE_DIR OUTPUT".into(),
        );
    }
    let root = Path::new(&args[1]);
    let source = Path::new(&args[3]);
    let route_dir = Path::new(&args[4]);
    let out = Path::new(&args[5]);
    fs::create_dir_all(out)?;
    let raw = fs::read(&args[2])?;
    let mut game = bundle::decode(&raw)?;
    for id in [1122, 915, 617] {
        game.disable_dynamic(id)?;
    }
    let n = u32::from_le_bytes(raw[8..12].try_into()?) as usize;
    let mut mission: rc_game::mission::Assets = serde_json::from_slice(&raw[12..12 + n])?;
    let animated = &raw[12 + n..];
    let vn = u32::from_le_bytes(animated[8..12].try_into()?) as usize;
    let visual = &animated[12..12 + vn];
    let base = &animated[12 + vn..];
    let bn = u32::from_le_bytes(base[8..12].try_into()?) as usize;
    let mut manifest: bundle::Manifest = serde_json::from_slice(&base[12..12 + bn])?;
    let scene = rc_render::Scene::from_snapshot(&base[12 + bn..])?;
    let map = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&map)?;
    let catalog = rc_inspect::defaults::load(root)?;
    let value = |id: u32, name: &str| -> Result<Value, String> {
        let e = &pkg.exports[id as usize - 1];
        Ok(catalog
            .instance(
                &pkg.object_path(e.class)?,
                &properties::read(&pkg, &map, e)?,
                name,
                0,
                "geo_01a.ctm",
            )?
            .value)
    };
    let number = |id, name| -> Result<f64, String> {
        match value(id, name)? {
            Value::Float(v) => Ok(v as f64),
            Value::Int(v) => Ok(v as f64),
            _ => Err("Bad original scalar".into()),
        }
    };
    let text = |id, name| -> Result<String, String> {
        match value(id, name)? {
            Value::Name(v) | Value::String(v) => Ok(v),
            Value::Object { path, .. } => Ok(path),
            _ => Err("Bad original reference".into()),
        }
    };
    let point = |id| -> Result<[f32; 3], String> {
        match value(id, "Location")? {
            Value::Vector(v) => Ok(v),
            _ => Err("Bad original location".into()),
        }
    };
    let trigger = |id| -> Result<rc_game::crash::Trigger, String> {
        Ok(rc_game::crash::Trigger {
            original: id,
            position: point(id)?.map(f64::from),
            radius: number(id, "CollisionRadius")?,
            height: number(id, "CollisionHeight")?,
        })
    };
    for (id, name, expected) in [
        (3292, "ExternalEvent", "CrashSiteSecured"),
        (3295, "Event", "KnockOverWall"),
        (293, "Event", "SpawnBarricadeTroopers"),
        (238, "Event", "DialogMakingABarricade"),
        (1329, "ExternalEvent", "SpawnBarricadeTroopers"),
        (1332, "ExternalEvent", "DialogMakingABarricade"),
        (1345, "TargetTag", "BarricadeWall"),
    ] {
        if text(id, name)? != expected {
            return Err("Changed connecting script binding".into());
        }
    }
    if number(1345, "amount")? < number(964, "Health")? {
        return Err("Original wall damage insufficient".into());
    }
    let mut script_receipts = vec![];
    for (id, expected) in [
        (40, (1329..=1355).collect::<Vec<i32>>()),
        (911, (3292..=3295).collect()),
    ] {
        let e = &pkg.exports[id - 1];
        let p = properties::read(&pkg, &map, e)?;
        let a = p
            .values
            .iter()
            .find(|p| p.name == "Actions")
            .ok_or("No actions")?;
        let payload = pkg.payload(&map, e)?;
        let actions =
            properties::object_array(&pkg, &payload[a.payload_offset..a.payload_offset + a.bytes])?;
        if actions.iter().map(|(i, _)| *i).collect::<Vec<_>>() != expected {
            return Err("Changed connecting action order".into());
        }
        script_receipts.push(serde_json::json!({"script":id,"actions":actions.iter().map(|(i,_)|{let e=&pkg.exports[*i as usize-1];Ok(serde_json::json!({"original":i,"class":pkg.object_path(e.class)?,"properties":properties::read(&pkg,&map,e)?}))}).collect::<Result<Vec<_>,String>>()?}));
    }
    // Remove these original static instances; they reappear with stateful poses.
    let mut original_hulls = vec![];
    for (id, name) in [
        (909, "KarmaProp7"),
        (910, "KarmaProp4"),
        (912, "KarmaProp0"),
        (913, "KarmaProp6"),
        (964, "Prop3"),
    ] {
        if id != 964 && text(id, "Tag")? != "KnockOverWall" {
            return Err("Wrong original falling wall".into());
        }
        let i = game
            .world
            .actors
            .iter()
            .position(|a| a.name == name)
            .ok_or("Missing original wall collider")?;
        let actor = game.world.actors.remove(i);
        let h = match actor.shape {
            rc_package::world_collision::BodyShape::Hulls(h) => h,
            _ => return Err("Unsupported wall collider".into()),
        };
        original_hulls.push((id, name, h));
        manifest.colliders.retain(|c| c.name != name);
    }
    let mut materials = vec![];
    let mut objects = vec![];
    let mut walls = vec![];
    for (id, name, hulls) in original_hulls {
        let vector = |name| -> Result<[f32; 3], String> {
            match value(id, name)? {
                Value::Vector(v) => Ok(v),
                _ => Err("Bad transform".into()),
            }
        };
        let rotation = match value(id, "Rotation")? {
            Value::Rotator(v) => v,
            _ => return Err("Bad original rotation".into()),
        };
        let scale = number(id, "DrawScale")?;
        let t = ActorTransform {
            location: point(id)?,
            rotation,
            scale: vector("DrawScale3D")?.map(|v| v * scale as f32),
            pivot: vector("PrePivot")?,
        };
        let mut mesh = mesh_scene(root, &text(id, "StaticMesh")?, &t, &catalog, &mut materials)?;
        let snapshot = mesh.snapshot()?;
        let released = if id == 964 {
            None
        } else {
            // Explicit limited adapter: tipped resting pose, not a general Karma solver.
            let mut fall = ActorTransform {
                location: t.location,
                pivot: t.location,
                rotation: [16384, 0, 0],
                scale: [1.; 3],
            };
            for tri in &mut mesh.triangles {
                tri.points = tri.points.map(|p| {
                    rc_package::mesh::transform_point(
                        p,
                        fall.location,
                        fall.rotation,
                        fall.scale,
                        fall.pivot,
                    )
                });
            }
            let min = mesh
                .triangles
                .iter()
                .flat_map(|t| t.points)
                .map(|p| p[2])
                .fold(f32::INFINITY, f32::min);
            let mut ground = None;
            for lift in [0., 32., 64., 120., 360.] {
                let p = [
                    t.location[0] as f64,
                    t.location[1] as f64,
                    t.location[2] as f64 + lift,
                ];
                if let Ok(Some(h)) = game.world.floor_contact(p, [1.; 3], 2048., 0.7) {
                    ground = Some(p[2] - 2048. * h.hit.fraction - 1.);
                    break;
                }
            }
            let dz = ground.ok_or("No resting wall floor")? as f32 - min + 0.25;
            fall.location[2] += dz;
            for tri in &mut mesh.triangles {
                for p in &mut tri.points {
                    p[2] += dz;
                }
            }
            let rest = hulls.transformed(&fall)?;
            walls.push(serde_json::json!({"original":id,"rest_delta_z":dz,"pose_rotation":fall.rotation,"ground":ground,"scope":"bounded tipped resting pose; native Karma integration absent"}));
            Some(dynamic::Released {
                collider: collider(format!("Rest.{id}"), &rest),
                snapshot: mesh.snapshot()?,
            })
        };
        objects.push(dynamic::Object {
            original: id,
            collider: collider(name.into(), &hulls),
            snapshot,
            released,
        });
    }
    let flow: grenade_pickup::Assets =
        serde_json::from_slice(&fs::read(source.join("grenade-assets.json"))?)?;
    let mut sources = vec![];
    for cue in &flow.cues {
        sources.push(cue.source.clone());
    }
    for id in [1334, 1338, 1343, 1344] {
        sources.push(text(id, "Sound")?);
    }
    let mut durations = vec![];
    for path in &sources {
        let (s, duration) = sound(root, path)?;
        mission.sounds.push(s);
        durations.push(duration);
    }
    let route_data: serde_json::Value =
        serde_json::from_slice(&fs::read(route_dir.join("pickup-route.json"))?)?;
    let route = serde_json::from_value(route_data["route"].clone())?;
    let mut barrier_waypoint = 0;
    for segment in route_data["segments"]
        .as_array()
        .ok_or("No route segments")?
    {
        barrier_waypoint += segment["points"]
            .as_array()
            .ok_or("No segment points")?
            .len();
        if segment["target"] == 238 {
            break;
        }
    }
    let g = grenade_runtime::Assets {
        flow,
        route,
        barrier_waypoint: barrier_waypoint - 1,
        spawn_trigger: trigger(293)?,
        dialog_trigger: trigger(238)?,
        wall_delay: number(3294, "PauseTime")?,
        barrier_waits: vec![
            number(1333, "PauseTime")?,
            durations[4],
            number(1335, "PauseTime")?,
            number(1337, "PauseTime")?,
            durations[5],
            number(1339, "PauseTime")?,
        ],
        sounds: sources,
        objects,
        pickup_snapshot: fs::read(source.join("pickup.rcscene"))?,
    };
    g.validate()?;
    mission.grenades = Some(g);
    let base = bundle::encode(&manifest, &scene)?;
    let mut animated = b"RCPLAY02".to_vec();
    animated.extend_from_slice(&(visual.len() as u32).to_le_bytes());
    animated.extend(visual);
    animated.extend(base);
    let metadata = serde_json::to_vec(&mission)?;
    let mut result = b"RCPLAY04".to_vec();
    result.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    result.extend(metadata);
    result.extend(animated);
    bundle::decode(&result)?;
    fs::write(out.join("geo_01a-grenades.rcplay"), result)?;
    fs::write(
        out.join("mission-receipt.json"),
        serde_json::to_vec_pretty(&mission)?,
    )?;
    fs::write(
        out.join("integration-receipt.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"scripts":script_receipts,"walls":walls,"materials":materials,"scope":"Selected original objectives0/1/2; connecting wall/dialog host, bounded resting wall pose, provisional combat. Original trooper staging, Karma dynamics and full explosion FX absent."}),
        )?,
    )?;
    println!("Built contiguous original Bacta/crash/grenade pickup section");
    Ok(())
}
