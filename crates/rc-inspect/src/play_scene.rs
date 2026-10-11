//! Assemble the first playable scene from the same original collision data as rc-mesh-probe.
use crate::{defaults, volumes};
use rc_package::{
    collision, geometry,
    hull::HullSet,
    level, mesh, mesh_collision,
    mesh_query::{ActorTransform, LineRoute},
    properties::{self, Value},
    read_package,
    world_collision::{BodyActor, BodyShape, StaticBodyWorld},
};
use std::{collections::HashMap, fs, path::Path, sync::Arc};

pub fn load(game_data: &Path, map: &Path, snapshot: &Path) -> Result<rc_game::Game, String> {
    let catalog = defaults::load(game_data)?;
    let bytes = fs::read(map).map_err(|e| e.to_string())?;
    let pkg = read_package(&bytes)?;
    let source = map.to_string_lossy();
    let starts = level::player_starts_with_defaults(&pkg, &bytes, &catalog, &source)?;
    let start = starts.first().ok_or("No player start in scene")?;
    let profile = rc_package::movement_profile::MovementProfile::read(&catalog)?;
    let loaded_volumes = volumes::load(&pkg, &bytes, &catalog, &source, profile.physics)?;
    if loaded_volumes.errors != 0 {
        return Err(format!(
            "Unresolved PhysicsVolumes: {} errors; {:?}",
            loaded_volumes.errors, loaded_volumes.actors
        ));
    }
    let binding = level::world_binding(&pkg, &bytes)?;
    let export = &pkg.exports[binding.model_export];
    let bsp = geometry::read_bsp(&pkg, &bytes, export)?;
    let solid = collision::read_model_collision(&pkg, &bytes, export, &bsp)?;
    let mut world = StaticBodyWorld {
        world: Ok(Arc::new(HullSet::read(&bsp, &solid)?)),
        actors: vec![],
    };
    let files: HashMap<_, _> = fs::read_dir(game_data.join("StaticMeshes"))
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("usx")))
        .map(|p| (p.file_stem().unwrap().to_string_lossy().to_lowercase(), p))
        .collect();
    let mut loaded = HashMap::new();
    let mut cache = HashMap::new();
    for (i, e) in pkg.exports.iter().enumerate() {
        let class = pkg.object_path(e.class)?;
        if !catalog
            .derives_from(&class, "Engine.Actor")
            .unwrap_or(false)
        {
            continue;
        }
        let props = properties::read(&pkg, &bytes, e)?;
        let value = |name| {
            catalog
                .instance(&class, &props, name, 0, &source)
                .map(|v| v.value)
        };
        let mesh_value = catalog.instance(&class, &props, "StaticMesh", 0, &source)?;
        let (index, path) = match mesh_value.value {
            Value::Object { index: 0, .. } => continue,
            Value::Object { index, path } => (index, path),
            _ => return Err("Actor StaticMesh not Object".into()),
        };
        let boolean = |name| match value(name)? {
            Value::Bool(v) => Ok(v),
            _ => Err(format!("{name} not Bool")),
        };
        if !boolean("bCollideActors")? || !boolean("bBlockPlayers")? {
            continue;
        }
        let cylinder = boolean("bUseCylinderCollision")?;
        let vector = |name| match value(name)? {
            Value::Vector(v) => Ok(v),
            _ => Err(format!("{name} not Vector")),
        };
        let rotation = match value("Rotation")? {
            Value::Rotator(v) => v,
            _ => return Err("Invalid actor rotation".into()),
        };
        let scale = match value("DrawScale")? {
            Value::Float(v) => v,
            _ => return Err("Invalid actor scale".into()),
        };
        let transform = ActorTransform {
            location: vector("Location")?,
            rotation,
            scale: vector("DrawScale3D")?.map(|v| v * scale),
            pivot: vector("PrePivot")?,
        };
        if !cache.contains_key(&path) {
            let (p, data, export) = if index > 0 && mesh_value.stage == "instance_delta" {
                (
                    &pkg,
                    &bytes,
                    pkg.exports
                        .get(index as usize - 1)
                        .ok_or("Invalid local mesh")?,
                )
            } else {
                let (package, object) = path.split_once('.').ok_or("Incomplete mesh reference")?;
                let key = package.to_lowercase();
                if !loaded.contains_key(&key) {
                    let data = fs::read(files.get(&key).ok_or("Missing mesh package")?)
                        .map_err(|e| e.to_string())?;
                    loaded.insert(key.clone(), (read_package(&data)?, data));
                }
                let (p, data) = &loaded[&key];
                let export = p
                    .exports
                    .iter()
                    .enumerate()
                    .find_map(|(i, e)| {
                        p.object_path(i as i32 + 1)
                            .ok()
                            .filter(|s| s.eq_ignore_ascii_case(object))
                            .map(|_| e)
                    })
                    .ok_or("Missing mesh export")?;
                (p, data, export)
            };
            let mesh = mesh::read_static_mesh(p, data, export)?;
            let collision = mesh_collision::read(p, data, export, &mesh)?;
            let simple = if collision.simple_model > 0 {
                let e = &p.exports[collision.simple_model as usize - 1];
                let bsp = geometry::read_bsp(p, data, e)?;
                let solid = collision::read_model_collision(p, data, e, &bsp)?;
                Some(HullSet::read(&bsp, &solid)?)
            } else {
                None
            };
            cache.insert(path.clone(), (collision, simple));
        }
        let (collision, simple) = &cache[&path];
        let shape = match collision.box_route(cylinder, 0, !collision.nodes.is_empty()) {
            LineRoute::Clear => BodyShape::Clear,
            LineRoute::SimpleModel => BodyShape::Hulls(Arc::new(
                simple
                    .as_ref()
                    .ok_or("Missing box model")?
                    .transformed(&transform)?,
            )),
            _ => {
                return Err(format!(
                    "Unsupported collision for {}",
                    pkg.object_path(i as i32 + 1)?
                ))
            }
        };
        world.actors.push(BodyActor {
            name: pkg.object_path(i as i32 + 1)?,
            shape,
        });
    }
    let mut scene =
        rc_render::Scene::from_snapshot(&fs::read(snapshot).map_err(|e| e.to_string())?)?;
    scene.starts = starts.clone();
    rc_game::Game::new(
        scene,
        world,
        loaded_volumes.world,
        profile,
        start.location.map(f64::from),
        f64::from(start.rotation[1]) * std::f64::consts::TAU / 65536.,
    )
}

/// Bounded encounter placement on real support, reachable by a clear player-sized sweep.
pub fn place_encounter(game: &mut rc_game::Game) -> Result<Vec<rc_game::Point>, String> {
    use rc_package::movement::PlacementState;
    for _ in 0..120 {
        game.advance(rc_game::STEP, rc_game::Input::default())?;
    }
    let spawn = game.state.pawn.body.position;
    let mut candidates = vec![];
    for ring in [400., 650., 900., 1200.] {
        for a in 0..24 {
            let angle = game.state.yaw + (a as f64 - 12.) * std::f64::consts::TAU / 24.;
            let p = [
                spawn[0] + angle.cos() * ring,
                spawn[1] + angle.sin() * ring,
                spawn[2] + 100.,
            ];
            if game.world.body_placement(p, [22., 22., 60.])?.state == PlacementState::Penetrating {
                continue;
            }
            let Some(floor) = game.world.floor_contact(p, [22., 22., 60.], 400., 0.7)? else {
                continue;
            };
            let p = [p[0], p[1], p[2] - floor.hit.fraction * 400. + 0.5];
            if game.world.body_placement(p, [22., 22., 60.])?.state == PlacementState::Penetrating {
                continue;
            }
            let player_p = [p[0], p[1], p[2] + game.profile.physics.extent[2] - 60.];
            if !game
                .world
                .sweep_motion(spawn, player_p, game.profile.physics.extent)?
                .contacts
                .is_empty()
            {
                continue;
            }
            let muzzle = [p[0], p[1], p[2] + 35.];
            if game.wall_fraction(game.eye(), muzzle)? < 1. {
                continue;
            }
            if candidates
                .iter()
                .any(|q: &rc_game::Point| (q[0] - p[0]).hypot(q[1] - p[1]) < 250.)
            {
                continue;
            }
            candidates.push(p);
        }
    }
    if candidates.len() < 3 {
        return Err(format!(
            "Only {} reachable enemy positions",
            candidates.len()
        ));
    }
    // Prefer targets facing the original start direction, then separate them around it.
    candidates.sort_by(|a, b| {
        let cost = |p: &rc_game::Point| {
            let bearing = (p[1] - spawn[1]).atan2(p[0] - spawn[0]);
            ((bearing - game.state.yaw + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI)
                .abs()
        };
        cost(a).total_cmp(&cost(b))
    });
    let selected = candidates[..3].to_vec();
    let exit = [
        selected[0][0],
        selected[0][1],
        selected[0][2] + game.profile.physics.extent[2] - 60.,
    ];
    // Reset starts on the settled floor, with fresh counters and a safe, tested encounter.
    game.state.statistics = rc_game::Statistics::default();
    game.set_encounter(&selected, exit)?;
    Ok(selected)
}
