use rc_game::{bundle, rendezvous_runtime::Assets};
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
        return Err("usage: rc-rendezvous-build GAME_DATA GUNNER_SCENE OUTPUT".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let data = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&data)?;
    let catalog = rc_inspect::defaults::load(root)?;
    let value = |id: usize, name| -> Result<Value, String> {
        let e = &pkg.exports[id - 1];
        let props = properties::read(&pkg, &data, e)?;
        Ok(catalog
            .instance(&pkg.object_path(e.class)?, &props, name, 0, "geo_01a")?
            .value)
    };
    let vector = |id, name| -> Result<[f32; 3], String> {
        match value(id, name)? {
            Value::Vector(v) => Ok(v),
            _ => Err("Missing original vector".into()),
        }
    };
    let number = |id, name| -> Result<f64, String> {
        match value(id, name)? {
            Value::Float(v) => Ok(f64::from(v)),
            _ => Err("Missing original number".into()),
        }
    };
    let trigger = |id| -> Result<rc_game::crash::Trigger, String> {
        Ok(rc_game::crash::Trigger {
            original: id as u32,
            position: vector(id, "Location")?.map(f64::from),
            radius: number(id, "CollisionRadius")?,
            height: number(id, "CollisionHeight")?,
        })
    };
    let flow: rc_game::rendezvous::Assets =
        serde_json::from_slice(&fs::read(out.join("rendezvous-assets.json"))?)?;
    let mesh: rc_game::visual::Mesh =
        serde_json::from_slice(&fs::read(out.join("scorch-mesh.json"))?)?;
    let sources = flow
        .programs
        .iter()
        .flat_map(|p| &p.actions)
        .filter_map(|a| match &a.operation {
            rc_game::rendezvous::Operation::Sound { source, .. } => Some(source.clone()),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    let sounds = sources
        .into_iter()
        .map(|source| {
            let wave = fs::read(
                out.join("sounds")
                    .join(format!("{}.wav", source.rsplit('.').next().unwrap())),
            )?;
            Ok(rc_game::combat::Sound { source, wave })
        })
        .collect::<Result<Vec<_>, std::io::Error>>()?;
    let graph: rc_game::navigation::Assets =
        serde_json::from_slice(&fs::read(out.join("route-crouched/route-graph.json"))?)?;
    let at = |id| {
        graph
            .nodes
            .iter()
            .position(|n| n.original == id)
            .ok_or("Missing companion route anchor")
    };
    let stand = |mut p: [f64; 3]| {
        p[2] += 28.;
        p
    };
    let path = |from, to| -> Result<Vec<[f64; 3]>, String> {
        let p = graph
            .shortest(&[(at(from)?.to_owned(), 0.)], &[(at(to)?.to_owned(), 0.)])
            .ok_or("No companion route")?;
        Ok(p.into_iter()
            .skip(1)
            .map(|i| stand(graph.nodes[i].position))
            .collect())
    };
    let route: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join("route-crouched/rendezvous-route.json"))?)?;
    let a = Assets {
        flow,
        mesh: Some(mesh),
        sounds,
        spawn: stand(graph.nodes[at(457)?].position),
        shoot_route: path(457, 1130)?,
        greet_route: path(1130, 792)?,
        player_route: serde_json::from_value(route["route"].clone())?,
        intro: trigger(532)?,
        behind: trigger(1133)?,
        ground_speed: 450.,
        provenance: serde_json::json!({"map":"geo_01a","factory":457,"script":454,"spawn_class":"CTCharacters.Commando62Delta","move_actions":[2316,2318],"shoot_anchor":1130,"greet_anchor":792,"player_route_requires_crouch":true,"controller":"bounded physical port controller; not a general native GoalQueue"}),
    };
    fs::write(
        out.join("rendezvous-runtime-assets.json"),
        serde_json::to_vec(&a)?,
    )?;
    let mut base = fs::read(&args[2])?;
    let mut layers = vec![];
    while !base.starts_with(b"RCPLAY01") {
        let n = u32::from_le_bytes(base[8..12].try_into()?) as usize;
        layers.push((base[..8].to_vec(), base[12..12 + n].to_vec()));
        base = base[12 + n..].to_vec();
    }
    let n = u32::from_le_bytes(base[8..12].try_into()?) as usize;
    let manifest: bundle::Manifest = serde_json::from_slice(&base[12..12 + n])?;
    let scene = rc_render::Scene::from_snapshot(&base[12 + n..])?;
    let scale = number(851, "DrawScale")? as f32;
    let t = ActorTransform {
        location: vector(851, "Location")?,
        rotation: match value(851, "Rotation")? {
            Value::Rotator(r) => r,
            _ => return Err("Missing original bunker rotation".into()),
        },
        scale: vector(851, "DrawScale3D")?.map(|v| v * scale),
        pivot: vector(851, "PrePivot")?,
    };
    let mut receipts = vec![];
    let wall = rc_inspect::mission_assets::mesh_scene(
        root,
        "Arena_HR.ArenaWalls.Bunker2",
        &t,
        &catalog,
        &mut receipts,
    )?;
    let key = |t: &rc_package::geometry::Triangle| t.points.map(|p| p.map(f32::to_bits));
    let keys = scene
        .triangles
        .iter()
        .map(key)
        .collect::<std::collections::BTreeSet<_>>();
    let matched = wall
        .triangles
        .iter()
        .filter(|t| keys.contains(&key(t)))
        .count();
    if matched != 0 && matched != wall.triangles.len() {
        return Err("Partial original large-bunker geometry".into());
    }
    let added = if matched == 0 {
        wall.triangles.len()
    } else {
        0
    };
    let mut triangles = scene.triangles;
    let mut appearance = scene.appearance;
    let mut textures = scene.textures;
    if added > 0 {
        let base = textures.len();
        textures.extend(wall.textures);
        triangles.extend(wall.triangles);
        appearance.extend(wall.appearance.into_iter().map(|f| {
            f.map(|mut f| {
                f.texture += base;
                f
            })
        }));
    }
    let mut rebuilt = rc_render::Scene::new(triangles)?;
    rebuilt.appearance = appearance;
    rebuilt.textures = textures;
    rebuilt.starts = scene.starts;
    rebuilt.solid = scene.solid;
    let mut base = bundle::encode(&manifest, &rebuilt)?;
    for (magic, meta) in layers.into_iter().rev() {
        base = wrap(&magic, &meta, &base);
    }
    let bytes = serde_json::to_vec(&a)?;
    if bytes.len() > 24 * 1024 * 1024 || bytes.len() + base.len() + 12 > 96 * 1024 * 1024 {
        return Err("Scorch scene exceeds budget".into());
    }
    let result = wrap(b"RCPLAY07", &bytes, &base);
    let game = bundle::decode(&result)?;
    fs::write(out.join("geo_01a-scorch.rcplay"), &result)?;
    fs::write(
        out.join("build-validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"bytes":result.len(),"additional_original_bunker_faces":added,"materials":receipts,"intro":game.rendezvous.as_ref().unwrap().assets.intro,"behind":game.rendezvous.as_ref().unwrap().assets.behind,"sounds":game.mission.as_ref().unwrap().sounds.len(),"companion":game.state.rendezvous,"scope":"Built scene; input-driven acceptance separate"}),
        )?,
    )?;
    println!(
        "Built RCPLAY07 Scorch scene: {} bytes, {added} additional original bunker faces",
        result.len()
    );
    Ok(())
}
