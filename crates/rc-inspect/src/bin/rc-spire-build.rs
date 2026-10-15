//! Preserve earlier scene layers; move selected barricades/exit door to original dynamic poses.
use rc_game::{
    bundle, dynamic,
    spire::{Action, Flow, Operation},
    spire_runtime::Assets,
};
use rc_package::{
    mesh_query::ActorTransform,
    properties::{self, Value},
    read_package,
};
use std::{env, fs, path::Path};
fn wrap(magic: &[u8], meta: &[u8], base: &[u8]) -> Vec<u8> {
    let mut b = magic.to_vec();
    b.extend((meta.len() as u32).to_le_bytes());
    b.extend(meta);
    b.extend(base);
    b
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-spire-build GAME_DATA DEMOLITION_SCENE OUTPUT".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let data = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&data)?;
    let cat = rc_inspect::defaults::load(root)?;
    let value = |id: usize, name: &str| -> Result<Value, String> {
        let e = &pkg.exports[id - 1];
        Ok(cat
            .instance(
                &pkg.object_path(e.class)?,
                &properties::read(&pkg, &data, e)?,
                name,
                0,
                "geo_01a",
            )?
            .value)
    };
    let indexed = |id: usize, name: &str, index: u32| -> Result<Value, String> {
        let e = &pkg.exports[id - 1];
        Ok(cat
            .instance(
                &pkg.object_path(e.class)?,
                &properties::read(&pkg, &data, e)?,
                name,
                index,
                "geo_01a",
            )?
            .value)
    };
    let vector = |id, name| -> Result<[f32; 3], String> {
        match value(id, name)? {
            Value::Vector(v) => Ok(v),
            _ => Err(format!("Invalid vector{id}/{name}")),
        }
    };
    let float = |id, name| -> Result<f64, String> {
        match value(id, name)? {
            Value::Float(v) => Ok(v as f64),
            _ => Err(format!("Invalid float{id}/{name}")),
        }
    };
    let path = |id, name| -> Result<String, String> {
        match value(id, name)? {
            Value::Object { path, .. } => Ok(path),
            Value::String(s) | Value::Name(s) => Ok(s),
            _ => Err(format!("Invalid path{id}/{name}")),
        }
    };
    let trigger = |id| -> Result<rc_game::crash::Trigger, String> {
        Ok(rc_game::crash::Trigger {
            original: id as u32,
            position: vector(id, "Location")?.map(f64::from),
            radius: float(id, "CollisionRadius")?,
            height: float(id, "CollisionHeight")?,
        })
    };
    let mut sounds = vec![];
    let mut audio = vec![];
    let mut add_sound = |source: &str| -> Result<(usize, f64), Box<dyn std::error::Error>> {
        let (s, seconds) = rc_inspect::mission_assets::sound_variant(root, source, &mut audio)?;
        let i = sounds.len();
        sounds.push(s);
        Ok((i, seconds))
    };
    let mut conversation = vec![];
    for id in 3183..=3186 {
        let (sound, seconds) = add_sound(&path(id, "Sound")?)?;
        conversation.push(Action {
            original: id as u32,
            operation: Operation::Sound { sound, seconds },
        });
    }
    let mut artillery = vec![];
    for id in 1307..=1315 {
        let class = pkg.object_path(pkg.exports[id - 1].class)?;
        let operation = match class.as_str() {
            "Gameplay.ACTION_PlaySound" => {
                let (sound, seconds) = add_sound(&path(id, "Sound")?)?;
                Operation::Sound { sound, seconds }
            }
            "Gameplay.ACTION_TriggerEvent" => Operation::Event {
                tag: path(id, "Event")?,
            },
            "Gameplay.ACTION_WaitForTimer" => Operation::Wait {
                seconds: float(id, "PauseTime")?,
            },
            "Gameplay.ACTION_HurtActor" => {
                for original in [860, 861] {
                    artillery.push(Action {
                        original: id as u32,
                        operation: Operation::Release { original },
                    });
                }
                Operation::Release { original: 862 }
            }
            _ => return Err(format!("Unsupported original exit action {id}/{class}").into()),
        };
        artillery.push(Action {
            original: id as u32,
            operation,
        });
    }
    let (opening, _) = add_sound(&path(34, "OpeningSound")?)?;
    let (opened, _) = add_sound(&path(34, "OpenedSound")?)?;
    let door_move = match indexed(34, "MoveTimes", 0)? {
        Value::Float(v) => v as f64,
        _ => return Err("Missing original door MoveTimes[0]".into()),
    };
    let door_delay = float(34, "DelayTime")?;
    let door_seconds = door_move + door_delay;
    for id in [860, 861, 862] {
        assert!(matches!(value(id, "Health")?, Value::Int(49000)));
    }
    assert_eq!(float(1314, "amount")?, 50000.);
    let flow = Flow {
        area: trigger(39)?,
        door: trigger(24)?,
        exit: trigger(9)?,
        next_url: match value(9, "URL")? {
            Value::String(s) => s,
            _ => return Err("Missing original exit URL".into()),
        },
        door_seconds,
        opening,
        opened,
        conversation,
        artillery,
    };
    flow.validate(sounds.len())?;
    let old = bundle::decode(&fs::read(&args[2])?)?;
    let mut receipts = vec![];
    let mut objects = vec![];
    let mut remove = std::collections::BTreeMap::new();
    let key = |t: &rc_package::geometry::Triangle| t.points.map(|p| p.map(f32::to_bits));
    let mut counts = vec![];
    for id in [860, 861, 862, 34, 35] {
        let transform = ActorTransform {
            location: vector(id, "Location")?,
            rotation: match value(id, "Rotation")? {
                Value::Rotator(v) => v,
                _ => return Err("Missing actor rotation".into()),
            },
            scale: vector(id, "DrawScale3D")?.map(|v| v * float(id, "DrawScale").unwrap() as f32),
            pivot: vector(id, "PrePivot")?,
        };
        let source = path(id, "StaticMesh")?;
        let scene =
            rc_inspect::mission_assets::mesh_scene(root, &source, &transform, &cat, &mut receipts)?;
        for t in &scene.triangles {
            *remove.entry(key(t)).or_insert(0usize) += 1;
        }
        let (file, object) = source.split_once('.').ok_or("Invalid static mesh path")?;
        let bytes = fs::read(root.join("StaticMeshes").join(format!("{file}.usx")))?;
        let p = read_package(&bytes)?;
        let e = p
            .exports
            .iter()
            .enumerate()
            .find_map(|(i, e)| {
                p.object_path(i as i32 + 1)
                    .is_ok_and(|s| s == object)
                    .then_some(e)
            })
            .ok_or("Missing collision mesh")?;
        let mesh = rc_package::mesh::read_static_mesh(&p, &bytes, e)?;
        let c = rc_package::mesh_collision::read(&p, &bytes, e, &mesh)?;
        if c.simple_model <= 0 {
            return Err(format!("No original door/barricade collision{id}").into());
        }
        let e = &p.exports[c.simple_model as usize - 1];
        let bsp = rc_package::geometry::read_bsp(&p, &bytes, e)?;
        let solid = rc_package::collision::read_model_collision(&p, &bytes, e, &bsp)?;
        let local = rc_package::hull::HullSet::read(&bsp, &solid)?;
        let h = local.transformed(&transform)?;
        let name = pkg.object_path(id as i32)?;
        let collider = if id >= 860 {
            let actor = old
                .world
                .actors
                .iter()
                .find(|a| a.name == name)
                .ok_or(format!("Missing original collider {name}"))?;
            match &actor.shape {
                rc_package::world_collision::BodyShape::Hulls(h) => {
                    rc_inspect::mission_assets::collider(name.clone(), h)
                }
                _ => return Err("Unsupported original barricade collider".into()),
            }
        } else {
            rc_inspect::mission_assets::collider(name, &h)
        };
        let released = if id < 100 {
            let mut opened = transform;
            let offset = match indexed(id, "KeyPos", 1)? {
                Value::Vector(v) => v,
                _ => return Err("Missing original KeyPos[1]".into()),
            };
            for (v, d) in opened.location.iter_mut().zip(offset) {
                *v += d;
            }
            let scene = rc_inspect::mission_assets::mesh_scene(
                root,
                &source,
                &opened,
                &cat,
                &mut receipts,
            )?;
            Some(dynamic::Released {
                collider: rc_inspect::mission_assets::collider(
                    format!("OpenDoor{id}"),
                    &local.transformed(&opened)?,
                ),
                snapshot: scene.snapshot()?,
            })
        } else {
            None
        };
        counts.push(
            serde_json::json!({"original":id,"faces":scene.triangles.len(),"hulls":h.hulls.len()}),
        );
        objects.push(dynamic::Object {
            original: id as u32,
            collider,
            snapshot: scene.snapshot()?,
            released,
        });
    }
    let mut base = fs::read(&args[2])?;
    let mut layers = vec![];
    while !base.starts_with(b"RCPLAY01") {
        let n = u32::from_le_bytes(base[8..12].try_into()?) as usize;
        layers.push((base[..8].to_vec(), base[12..12 + n].to_vec()));
        base = base[12 + n..].to_vec();
    }
    let n = u32::from_le_bytes(base[8..12].try_into()?) as usize;
    let mut manifest: bundle::Manifest = serde_json::from_slice(&base[12..12 + n])?;
    let scene = rc_render::Scene::from_snapshot(&base[12 + n..])?;
    let mut triangles = vec![];
    let mut faces = vec![];
    let mut removed = 0;
    for (i, t) in scene.triangles.into_iter().enumerate() {
        if let Some(n) = remove.get_mut(&key(&t)) {
            if *n > 0 {
                *n -= 1;
                removed += 1;
                continue;
            }
        }
        triangles.push(t);
        faces.push(scene.appearance[i]);
    }
    manifest
        .colliders
        .retain(|c| !objects.iter().any(|o| o.collider.name == c.name));
    let mut scene = rc_render::Scene::new(triangles)?;
    scene.appearance = faces;
    let original = rc_render::Scene::from_snapshot(&base[12 + n..])?;
    scene.textures = original.textures;
    scene.starts = original.starts;
    scene.solid = original.solid;
    let mut base = bundle::encode(&manifest, &scene)?;
    for (magic, meta) in layers.into_iter().rev() {
        base = wrap(&magic, &meta, &base);
    }
    fs::write(out.join("world.rcplay"), &base)?;
    let route = if out.join("route.json").exists() {
        serde_json::from_slice(&fs::read(out.join("route.json"))?)?
    } else {
        vec![flow.area.position]
    };
    let assets = Assets {
        flow,
        route,
        sounds,
        objects,
        provenance: serde_json::json!({"selected_scripts":[32,848,1065],"door_timing":{"delay_seconds":door_delay,"move_seconds":door_move,"key_position_index":1,"adapter":"original delay plus move interval, bounded final pose switch"},"next_url":"original destination request, no GEO_01B loading"}),
    };
    fs::write(out.join("spire-assets.json"), serde_json::to_vec(&assets)?)?;
    let final_bytes = wrap(b"RCPLAY09", &serde_json::to_vec(&assets)?, &base);
    let game = bundle::decode(&final_bytes)?;
    fs::write(out.join("geo_01a-spire.rcplay"), &final_bytes)?;
    fs::write(
        out.join("build-validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"removed_faces":removed,"objects":counts,"sound_count":game.mission.as_ref().unwrap().sounds.len(),"bytes":final_bytes.len(),"materials":receipts,"audio":audio}),
        )?,
    )?;
    println!(
        "Built spire scene: {} bytes; {removed} dynamic faces moved; {} samples",
        final_bytes.len(),
        game.mission.as_ref().unwrap().sounds.len()
    );
    Ok(())
}
