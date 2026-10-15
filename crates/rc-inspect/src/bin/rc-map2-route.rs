//! Continuous controller-input route runner; never assigns live pose/resources/events.
//! Navigation probes use a separate world with automatic doors open for planning only.
use rc_game::{
    navigation::{Assets, Edge, Node},
    Game, Input, Point, STEP,
};
use rc_package::world_collision::{BodyActor, BodyShape, StaticBodyWorld};
use std::{collections::BTreeSet, fs, path::Path};
const OUT: &str = "analysis/mission18-map2-route";
fn distance(a: Point, b: Point) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}
fn clone_world(g: &Game, all: bool) -> StaticBodyWorld {
    let p = g.map.as_ref().unwrap().gameplay.as_ref().unwrap();
    let clear: BTreeSet<_> = p
        .gates
        .iter()
        .zip(&p.assets.gates)
        .filter(|(_, a)| all || a.event.starts_with("OpenDoor."))
        .filter_map(|(p, _)| p.collider.as_ref().map(|(i, _)| *i))
        .collect();
    StaticBodyWorld {
        world: g.world.world.clone(),
        actors: g
            .world
            .actors
            .iter()
            .enumerate()
            .map(|(i, a)| BodyActor {
                name: a.name.clone(),
                shape: if clear.contains(&i) {
                    BodyShape::Clear
                } else {
                    match &a.shape {
                        BodyShape::Clear => BodyShape::Clear,
                        BodyShape::Hulls(h) => BodyShape::Hulls(h.clone()),
                        BodyShape::Unsupported(s) => BodyShape::Unsupported(s.clone()),
                    }
                },
            })
            .collect(),
    }
}
fn graph(g: &Game) -> Result<Assets, Box<dyn std::error::Error>> {
    let cached = Path::new(OUT).join("standing-graph.json");
    if cached.exists() {
        let mut a: Assets = serde_json::from_slice(&fs::read(&cached)?)?;
        if a.provenance["neighbor_distance"].as_u64() != Some(1500) {
            let world = clone_world(g, true);
            for i in 0..a.nodes.len() {
                let mut near: Vec<_> = a
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i)
                    .map(|(j, n)| (j, distance(a.nodes[i].position, n.position)))
                    .filter(|(_, d)| *d >= 900. && *d < 1500.)
                    .collect();
                near.sort_by(|a, b| a.1.total_cmp(&b.1));
                for (j, d) in near.into_iter().take(12) {
                    if rc_game::squad::corridor_with_tolerance(
                        &world,
                        a.nodes[i].position,
                        a.nodes[j].position,
                        g.profile.physics.extent,
                        4.,
                    )?
                    .is_some()
                    {
                        a.edges.push(Edge {
                            original: a.edges.len() as i32 + 1,
                            start: i,
                            end: j,
                            distance: d,
                        });
                    }
                }
                if i % 40 == 0 {
                    println!("extend navigation: {i}/{}", a.nodes.len());
                }
            }
            a.provenance["neighbor_distance"] = serde_json::json!(1500);
            fs::write(&cached, serde_json::to_vec(&a)?)?;
        }
        if a.provenance["lower_probe"].as_bool() != Some(true) {
            let world = clone_world(g, true);
            let rows: Vec<serde_json::Value> = serde_json::from_slice(&fs::read(
                "analysis/mission17-map2-gameplay/original-map-inventory.json",
            )?)?;
            let old = a.nodes.len();
            for row in rows {
                let id = row["original"].as_i64().unwrap() as i32;
                if a.nodes.iter().any(|n| n.original == id)
                    || ![
                        "Engine.PathNode",
                        "Engine.CoverPoint",
                        "Engine.CombatPoint",
                        "Engine.AutoDoor",
                        "Engine.SquadMarkerSpot",
                    ]
                    .contains(&row["class"].as_str().unwrap_or(""))
                {
                    continue;
                }
                let p: Point =
                    serde_json::from_value(row["resolved"]["Location"]["value"]["value"].clone())?;
                for z in [0., 40., 80., -40.] {
                    if let Some(position) = rc_game::mission_motion::ground(
                        &world,
                        [p[0], p[1], p[2] + z],
                        g.profile.physics.extent,
                        512.,
                    )? {
                        a.nodes.push(Node {
                            original: id,
                            name: row["name"].as_str().unwrap().into(),
                            position,
                            extra_cost: 0.,
                        });
                        break;
                    }
                }
            }
            println!(
                "Low-ceiling probes: {} additional original nodes",
                a.nodes.len() - old
            );
            for i in 0..a.nodes.len() {
                let mut near: Vec<_> = a
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i && (i >= old || *j >= old))
                    .map(|(j, n)| (j, distance(a.nodes[i].position, n.position)))
                    .filter(|(_, d)| *d < 1500.)
                    .collect();
                near.sort_by(|a, b| a.1.total_cmp(&b.1));
                for (j, d) in near.into_iter().take(24) {
                    if rc_game::squad::corridor_with_tolerance(
                        &world,
                        a.nodes[i].position,
                        a.nodes[j].position,
                        g.profile.physics.extent,
                        4.,
                    )?
                    .is_some()
                    {
                        a.edges.push(Edge {
                            original: a.edges.len() as i32 + 1,
                            start: i,
                            end: j,
                            distance: d,
                        });
                    }
                }
            }
            a.provenance["lower_probe"] = serde_json::json!(true);
            fs::write(&cached, serde_json::to_vec(&a)?)?;
        }
        if a.provenance["ramp_projection_tolerance"].as_u64() != Some(4) {
            let world = clone_world(g, true);
            let mut groups: Vec<usize> = (0..a.nodes.len()).collect();
            for _ in 0..a.nodes.len() {
                let mut changed = false;
                for e in &a.edges {
                    let v = groups[e.start].min(groups[e.end]);
                    if groups[e.start] != v || groups[e.end] != v {
                        groups[e.start] = v;
                        groups[e.end] = v;
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            }
            let mut added = 0;
            for i in 0..a.nodes.len() {
                for j in i + 1..a.nodes.len() {
                    let d = distance(a.nodes[i].position, a.nodes[j].position);
                    if groups[i] == groups[j] || d > 1500. {
                        continue;
                    }
                    for (start, end) in [(i, j), (j, i)] {
                        if rc_game::squad::corridor_with_tolerance(
                            &world,
                            a.nodes[start].position,
                            a.nodes[end].position,
                            g.profile.physics.extent,
                            4.,
                        )?
                        .is_some()
                        {
                            a.edges.push(Edge {
                                original: a.edges.len() as i32 + 1,
                                start,
                                end,
                                distance: d,
                            });
                            added += 1;
                        }
                    }
                }
            }
            println!("Slope projection: {added} additional directed corridors");
            a.provenance["ramp_projection_tolerance"] = serde_json::json!(4);
            fs::write(&cached, serde_json::to_vec(&a)?)?;
        }
        if a.provenance["reverse_projection"].as_bool() != Some(true) {
            let world = clone_world(g, true);
            let pairs: BTreeSet<_> = a.edges.iter().map(|e| (e.start, e.end)).collect();
            let candidates: Vec<_> = a
                .edges
                .iter()
                .filter(|e| !pairs.contains(&(e.end, e.start)))
                .map(|e| (e.end, e.start, e.distance))
                .collect();
            let mut count = 0;
            for (start, end, d) in candidates {
                if rc_game::squad::corridor_with_tolerance(
                    &world,
                    a.nodes[start].position,
                    a.nodes[end].position,
                    g.profile.physics.extent,
                    4.,
                )?
                .is_some()
                {
                    a.edges.push(Edge {
                        original: a.edges.len() as i32 + 1,
                        start,
                        end,
                        distance: d,
                    });
                    count += 1;
                }
            }
            println!("Verified reverse slope corridors: {count}");
            a.provenance["reverse_projection"] = serde_json::json!(true);
            fs::write(&cached, serde_json::to_vec(&a)?)?;
        }
        if a.provenance["hidden_wreck_lifecycle"].as_bool() != Some(true)
            && g.map
                .as_ref()
                .unwrap()
                .gameplay
                .as_ref()
                .unwrap()
                .assets
                .gates
                .len()
                == 30
        {
            let world = clone_world(g, true);
            let pairs: BTreeSet<_> = a.edges.iter().map(|e| (e.start, e.end)).collect();
            let mut added = 0;
            for i in 0..a.nodes.len() {
                let mut near: Vec<_> = a
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i && !pairs.contains(&(i, *j)))
                    .map(|(j, n)| (j, distance(a.nodes[i].position, n.position)))
                    .filter(|(_, d)| *d < 1500.)
                    .collect();
                near.sort_by(|a, b| a.1.total_cmp(&b.1));
                for (j, d) in near.into_iter().take(12) {
                    if rc_game::squad::corridor_with_tolerance(
                        &world,
                        a.nodes[i].position,
                        a.nodes[j].position,
                        g.profile.physics.extent,
                        4.,
                    )?
                    .is_some()
                    {
                        a.edges.push(Edge {
                            original: a.edges.len() as i32 + 1,
                            start: i,
                            end: j,
                            distance: d,
                        });
                        added += 1;
                    }
                }
                if i % 80 == 0 {
                    println!("Retired-collision navigation: {i}/{}", a.nodes.len());
                }
            }
            println!("Hidden wreck retirement: {added} additional directed corridors");
            a.provenance["hidden_wreck_lifecycle"] = serde_json::json!(true);
            fs::write(&cached, serde_json::to_vec(&a)?)?;
        }
        if a.provenance["all_nav_probes_after_retirement"].as_bool() != Some(true)
            && g.map
                .as_ref()
                .unwrap()
                .gameplay
                .as_ref()
                .unwrap()
                .assets
                .gates
                .len()
                == 30
        {
            let world = clone_world(g, true);
            let old = a.nodes.len();
            let rows: Vec<serde_json::Value> = serde_json::from_slice(&fs::read(
                "analysis/mission17-map2-gameplay/original-map-inventory.json",
            )?)?;
            for row in rows {
                let id = row["original"].as_i64().unwrap() as i32;
                if a.nodes.iter().any(|n| n.original == id)
                    || ![
                        "Engine.PathNode",
                        "Engine.CoverPoint",
                        "Engine.CombatPoint",
                        "Engine.AutoDoor",
                        "Engine.SquadMarkerSpot",
                    ]
                    .contains(&row["class"].as_str().unwrap_or(""))
                {
                    continue;
                }
                let p: Point =
                    serde_json::from_value(row["resolved"]["Location"]["value"]["value"].clone())?;
                for z in [0., 40., 80., 160.] {
                    if let Some(position) = rc_game::mission_motion::ground(
                        &world,
                        [p[0], p[1], p[2] + z],
                        g.profile.physics.extent,
                        512.,
                    )? {
                        a.nodes.push(Node {
                            original: id,
                            name: row["name"].as_str().unwrap().into(),
                            position,
                            extra_cost: 0.,
                        });
                        break;
                    }
                }
            }
            for i in 0..a.nodes.len() {
                let mut near: Vec<_> = a
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i && (i >= old || *j >= old))
                    .map(|(j, n)| (j, distance(a.nodes[i].position, n.position)))
                    .filter(|(_, d)| *d < 1500.)
                    .collect();
                near.sort_by(|a, b| a.1.total_cmp(&b.1));
                for (j, d) in near.into_iter().take(12) {
                    if rc_game::squad::corridor_with_tolerance(
                        &world,
                        a.nodes[i].position,
                        a.nodes[j].position,
                        g.profile.physics.extent,
                        4.,
                    )?
                    .is_some()
                    {
                        a.edges.push(Edge {
                            original: a.edges.len() as i32 + 1,
                            start: i,
                            end: j,
                            distance: d,
                        });
                    }
                }
            }
            println!(
                "Original navigation points recovered after wreck retirement: {}",
                a.nodes.len() - old
            );
            a.provenance["all_nav_probes_after_retirement"] = serde_json::json!(true);
            fs::write(&cached, serde_json::to_vec(&a)?)?;
        }
        return Ok(a);
    }
    let rows: Vec<serde_json::Value> = serde_json::from_slice(&fs::read(
        "analysis/mission17-map2-gameplay/original-map-inventory.json",
    )?)?;
    let world = clone_world(g, true);
    let mut nodes = vec![];
    for row in rows {
        if ![
            "Engine.PathNode",
            "Engine.CoverPoint",
            "Engine.CombatPoint",
            "Engine.AutoDoor",
            "Engine.SquadMarkerSpot",
        ]
        .contains(&row["class"].as_str().unwrap_or(""))
        {
            continue;
        }
        let p: Point =
            serde_json::from_value(row["resolved"]["Location"]["value"]["value"].clone())?;
        if let Some(position) = rc_game::mission_motion::ground(
            &world,
            [p[0], p[1], p[2] + 160.],
            g.profile.physics.extent,
            512.,
        )? {
            nodes.push(Node {
                original: row["original"].as_i64().unwrap() as i32,
                name: row["name"].as_str().unwrap().into(),
                position,
                extra_cost: 0.,
            });
        }
    }
    let mut edges = vec![];
    for (i, n) in nodes.iter().enumerate() {
        let mut near: Vec<_> = nodes
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(j, m)| (j, distance(n.position, m.position)))
            .filter(|(_, d)| *d < 900.)
            .collect();
        near.sort_by(|a, b| a.1.total_cmp(&b.1));
        for (j, d) in near.into_iter().take(20) {
            if rc_game::squad::corridor_with_tolerance(
                &world,
                n.position,
                nodes[j].position,
                g.profile.physics.extent,
                4.,
            )?
            .is_some()
            {
                edges.push(Edge {
                    original: edges.len() as i32 + 1,
                    start: i,
                    end: j,
                    distance: d,
                });
            }
        }
        if i % 40 == 0 {
            println!(
                "navigation: {i}/{} nodes, {} edges",
                nodes.len(),
                edges.len()
            );
        }
    }
    let a = Assets {
        nodes,
        edges,
        lunge_distance: 0.,
        jump_distance: 0.,
        offensive_jump_probability: 0.,
        provenance: serde_json::json!({"source":"Original GEO_01B navigation actor locations; geometric standing-neighbor probes","extent":g.profile.physics.extent,"planning_only":"All selected gate colliders clear during graph extraction, live input collision remains authoritative"}),
    };
    fs::write(cached, serde_json::to_vec(&a)?)?;
    Ok(a)
}
struct Driver {
    g: Game,
    nav: Assets,
    inputs: Vec<u8>,
    stage: String,
}
impl Driver {
    fn tick(&mut self, mut i: Input) -> Result<(), String> {
        // Quantize to the same 20-byte ABI/replay representation used on Android.
        i.forward = i.forward as f32 as f64;
        i.strafe = i.strafe as f32 as f64;
        i.look = i.look.map(|v| v as f32 as f64);
        for v in [i.forward, i.strafe, i.look[0], i.look[1]] {
            self.inputs.extend((v as f32).to_le_bytes());
        }
        let flags = u32::from(i.jump)
            | u32::from(i.fire) << 1
            | u32::from(i.reload) << 2
            | u32::from(i.walking) << 3
            | u32::from(i.use_action) << 4
            | u32::from(i.throw_grenade) << 5
            | u32::from(i.crouch) << 6;
        self.inputs.extend(flags.to_le_bytes());
        self.g.advance(STEP, i)?;
        if self.g.state.phase == rc_game::Phase::Lost {
            return Err(format!(
                "Lost during {} at {:?}",
                self.stage, self.g.state.pawn.body.position
            ));
        }
        if self.inputs.len() / 20 > 100000 {
            return Err("Route tick budget exceeded".into());
        }
        Ok(())
    }
    fn aimed(&self, target: Point) -> [f64; 2] {
        let eye = self.g.eye();
        let d: Point = std::array::from_fn(|i| target[i] - eye[i]);
        let yaw = d[1].atan2(d[0]);
        let pitch = d[2].atan2(d[0].hypot(d[1])).clamp(-1.4, 1.4);
        [
            (self.g.state.yaw - yaw + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI,
            self.g.state.pitch - pitch,
        ]
    }
    fn control(
        &self,
        goal: Option<Point>,
        use_target: Option<Point>,
        fire_override: bool,
    ) -> Result<Input, String> {
        let pos = self.g.state.pawn.body.position;
        let mut target = use_target;
        let mut fire = fire_override;
        if use_target.is_none()
            && !self
                .g
                .state
                .map
                .as_ref()
                .unwrap()
                .gameplay
                .as_ref()
                .unwrap()
                .armed
                .iter()
                .zip(
                    &self
                        .g
                        .state
                        .map
                        .as_ref()
                        .unwrap()
                        .gameplay
                        .as_ref()
                        .unwrap()
                        .used,
                )
                .any(|(a, u)| *a && !*u)
        {
            let mut enemies: Vec<_> = self
                .g
                .state
                .enemies
                .iter()
                .filter(|e| e.health > 0)
                .map(|e| {
                    (
                        distance(pos, e.position),
                        [e.position[0], e.position[1], e.position[2] + 35.],
                    )
                })
                .filter(|(d, _)| *d < 1700.)
                .collect();
            enemies.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (_, p) in enemies {
                if self.g.wall_fraction(self.g.eye(), p)? > 0.999 {
                    target = Some(p);
                    fire = true;
                    break;
                }
            }
        }
        let look = self.aimed(
            target
                .or(goal)
                .unwrap_or_else(|| self.g.eye().map(|v| v + 1.)),
        );
        let yaw = self.g.state.yaw - look[0];
        let (sin, cos) = yaw.sin_cos();
        let (forward, strafe) = if fire && use_target.is_none() {
            (0., 0.)
        } else if let Some(goal) = goal {
            let dx = goal[0] - pos[0];
            let dy = goal[1] - pos[1];
            let d = dx.hypot(dy).max(1.);
            let speed = 0.5_f64.min(d / 70.);
            let f = (dx * cos + dy * sin) / d;
            let side = (dx * sin - dy * cos) / d;
            (
                speed * f
                    / if f < 0. {
                        self.g.profile.ratios.back
                    } else {
                        1.
                    },
                speed * side / self.g.profile.ratios.side,
            )
        } else {
            (0., 0.)
        };
        Ok(Input {
            forward: forward.clamp(-1., 1.),
            strafe: strafe.clamp(-1., 1.),
            look,
            fire,
            reload: self.g.state.ammo < 4 && self.g.state.reserve > 0,
            use_action: use_target.is_some(),
            ..Input::default()
        })
    }
    fn plan(&self, to: Point) -> Result<Vec<Point>, String> {
        let world = clone_world(&self.g, false);
        let from = self.g.state.pawn.body.position;
        if rc_game::squad::corridor_with_tolerance(
            &world,
            from,
            to,
            self.g.profile.physics.extent,
            4.,
        )?
        .is_some()
        {
            return Ok(vec![to]);
        }
        let connect = |p: Point| -> Result<Vec<(usize, f64)>, String> {
            let mut ns: Vec<_> = self
                .nav
                .nodes
                .iter()
                .enumerate()
                .map(|(i, n)| (i, distance(p, n.position)))
                .filter(|(_, d)| *d < 1200.)
                .collect();
            ns.sort_by(|a, b| a.1.total_cmp(&b.1));
            let mut v = vec![];
            for (i, d) in ns.into_iter().take(16) {
                if rc_game::squad::corridor_with_tolerance(
                    &world,
                    p,
                    self.nav.nodes[i].position,
                    self.g.profile.physics.extent,
                    4.,
                )?
                .is_some()
                {
                    v.push((i, d));
                }
            }
            Ok(v)
        };
        let mut nav = Assets {
            nodes: self
                .nav
                .nodes
                .iter()
                .map(|n| Node {
                    original: n.original,
                    name: n.name.clone(),
                    position: n.position,
                    extra_cost: 0.,
                })
                .collect(),
            edges: vec![],
            lunge_distance: 0.,
            jump_distance: 0.,
            offensive_jump_probability: 0.,
            provenance: serde_json::Value::Null,
        };
        let p = self.g.map.as_ref().unwrap().gameplay.as_ref().unwrap();
        let s = self
            .g
            .state
            .map
            .as_ref()
            .unwrap()
            .gameplay
            .as_ref()
            .unwrap();
        for e in &self.nav.edges {
            let blocked = p
                .gates
                .iter()
                .zip(&p.assets.gates)
                .filter(|(_, a)| !a.event.starts_with("OpenDoor.") && !s.events.contains(&a.event))
                .filter_map(|(g, _)| g.collider.as_ref())
                .any(|(_, h)| {
                    h.sweep_motion(
                        self.nav.nodes[e.start].position,
                        self.nav.nodes[e.end].position,
                        self.g.profile.physics.extent,
                    )
                    .is_ok_and(|h| h.is_some())
                });
            if !blocked {
                nav.edges.push(Edge {
                    original: e.original,
                    start: e.start,
                    end: e.end,
                    distance: e.distance,
                });
            }
        }
        let path = nav
            .shortest(&connect(from)?, &connect(to)?)
            .ok_or_else(|| format!("No standing route {:?} -> {:?}", from, to))?;
        Ok(path
            .into_iter()
            .map(|i| self.nav.nodes[i].position)
            .chain(std::iter::once(to))
            .collect())
    }
    fn go(&mut self, p: Point, label: &str) -> Result<(), String> {
        if self.g.state.phase == rc_game::Phase::Won {
            return Ok(());
        }
        self.stage = label.into();
        self.save(&Err("In progress".into()))
            .map_err(|e| e.to_string())?;
        let world = clone_world(&self.g, false);
        let mut candidates = vec![];
        for radius in [0., 60., 120., 180., 240.] {
            for angle in 0..16 {
                let angle = angle as f64 * std::f64::consts::TAU / 16.;
                for height in [0., 40., 80., 160.] {
                    if let Some(q) = rc_game::mission_motion::ground(
                        &world,
                        [
                            p[0] + radius * angle.cos(),
                            p[1] + radius * angle.sin(),
                            p[2] + height,
                        ],
                        self.g.profile.physics.extent,
                        512.,
                    )? {
                        if let Ok(route) = self.plan(q) {
                            candidates.push((radius, q, route));
                            break;
                        }
                    }
                }
                if radius == 0. {
                    break;
                }
            }
            if !candidates.is_empty() {
                break;
            }
        }
        let (_, goal, route) = candidates
            .into_iter()
            .next()
            .ok_or_else(|| format!("No reachable supported goal near {label} {:?}", p))?;
        println!("{}: {} waypoints to {:?}", label, route.len(), goal);
        fs::write(
            Path::new(OUT).join("current-route.json"),
            serde_json::to_vec_pretty(&route).unwrap(),
        )
        .map_err(|e| e.to_string())?;
        for waypoint in route {
            let mut stuck = 0;
            let mut previous = self.g.state.pawn.body.position;
            for _ in 0..3600 {
                let pos = self.g.state.pawn.body.position;
                if self.g.state.phase == rc_game::Phase::Won {
                    break;
                }
                if (pos[0] - waypoint[0]).hypot(pos[1] - waypoint[1]) < 14.
                    && (pos[2] - waypoint[2]).abs() < 40.
                {
                    break;
                }
                let input = self.control(Some(waypoint), None, false)?;
                let fighting = input.fire || self.g.state.reload_remaining > 0.;
                self.tick(input)?;
                let moved = distance(previous, self.g.state.pawn.body.position);
                previous = self.g.state.pawn.body.position;
                if moved < 0.05 && !fighting {
                    stuck += 1;
                } else {
                    stuck = 0;
                }
                if stuck > 150 {
                    return Err(format!(
                        "Blocked {label} at {:?}, goal {:?}",
                        previous, waypoint
                    ));
                }
            }
            if self.g.state.phase != rc_game::Phase::Won
                && distance(self.g.state.pawn.body.position, waypoint) > 60.
            {
                return Err(format!("Could not reach {label}"));
            }
        }
        println!(
            "{} reached: HP{} ammo{}/{} enemies{} tick{}",
            label,
            self.g.state.health,
            self.g.state.ammo,
            self.g.state.reserve,
            self.g.state.enemies.iter().filter(|e| e.health > 0).count(),
            self.g.state.statistics.ticks
        );
        fs::write(
            Path::new(OUT).join(format!("{label}.json")),
            serde_json::to_vec_pretty(&self.g.state).unwrap(),
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }
    fn hold(&mut self, n: usize, target: Option<Point>) -> Result<(), String> {
        for _ in 0..n {
            self.tick(self.control(None, target, false)?)?;
        }
        Ok(())
    }
    fn interaction(&mut self, index: usize) -> Result<(), String> {
        let p = self
            .g
            .map
            .as_ref()
            .unwrap()
            .gameplay
            .as_ref()
            .unwrap()
            .assets
            .interactions[index]
            .position;
        let mut candidates = vec![];
        let world = clone_world(&self.g, false);
        for r in [100., 150., 190.] {
            for i in 0..24 {
                let a = i as f64 * std::f64::consts::TAU / 24.;
                if let Some(q) = rc_game::mission_motion::ground(
                    &world,
                    [p[0] + r * a.cos(), p[1] + r * a.sin(), p[2] + 160.],
                    self.g.profile.physics.extent,
                    512.,
                )? {
                    let eye = [q[0], q[1], q[2] + self.g.profile.physics.extent[2] * 0.65];
                    let actor = &self
                        .g
                        .map
                        .as_ref()
                        .unwrap()
                        .gameplay
                        .as_ref()
                        .unwrap()
                        .assets
                        .interactions[index]
                        .actor;
                    let t = self.g.world.sweep_motion(eye, p, [0.; 3])?;
                    if distance(eye, p) < 230.
                        && t.contacts
                            .iter()
                            .all(|c| c.object == *actor || c.hit.fraction > 0.96)
                    {
                        if let Ok(route) = self.plan(q) {
                            let cost = route.windows(2).map(|w| distance(w[0], w[1])).sum::<f64>()
                                + distance(self.g.state.pawn.body.position, route[0]);
                            candidates.push((cost, q));
                        }
                    }
                }
            }
        }
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
        let q = candidates
            .first()
            .ok_or(format!("No usable approach {index}"))?
            .1;
        let retreat = self.g.state.pawn.body.position;
        self.go(q, &format!("interaction-{index}"))?;
        for _ in 0..430 {
            self.tick(self.control(None, Some(p), false)?)?;
            let s = self
                .g
                .state
                .map
                .as_ref()
                .unwrap()
                .gameplay
                .as_ref()
                .unwrap();
            if s.armed[index] || s.used[index] {
                break;
            }
        }
        let s = self
            .g
            .state
            .map
            .as_ref()
            .unwrap()
            .gameplay
            .as_ref()
            .unwrap();
        if s.armed[index] && !s.used[index] {
            self.go(retreat, &format!("retreat-{index}"))?;
            self.tick(self.control(None, Some(p), true)?)?;
        }
        if !self
            .g
            .state
            .map
            .as_ref()
            .unwrap()
            .gameplay
            .as_ref()
            .unwrap()
            .used[index]
        {
            return Err(format!("Interaction {index} did not complete"));
        }
        self.hold(600, None)?;
        Ok(())
    }
    fn run(&mut self) -> Result<(), String> {
        self.go([-10000., -2697., -808.], "entrance-door")?;
        let station = self
            .g
            .map
            .as_ref()
            .unwrap()
            .gameplay
            .as_ref()
            .unwrap()
            .assets
            .stations[2];
        self.go(station, "entrance-bacta")?;
        self.hold(120, Some(station))?;
        let target = |g: &Game, key: &str| {
            g.map
                .as_ref()
                .unwrap()
                .gameplay
                .as_ref()
                .unwrap()
                .assets
                .targets[key]
        };
        self.go(target(&self.g, "TrackerTarget01"), "entrance-hall")?;
        self.go(target(&self.g, "TrackerTarget02"), "fixer-door-dialog")?;
        self.hold(600, None)?;
        self.interaction(0)?;
        self.interaction(1)?;
        self.go(target(&self.g, "TrackerTarget04"), "control-room")?;
        self.interaction(2)?;
        self.go(target(&self.g, "TrackerTarget06"), "hangar-one-entry")?;
        self.hold(600, None)?;
        self.interaction(3)?;
        let station = self
            .g
            .map
            .as_ref()
            .unwrap()
            .gameplay
            .as_ref()
            .unwrap()
            .assets
            .stations[0];
        self.go(station, "hangar-one-bacta")?;
        self.hold(120, Some(station))?;
        self.go(target(&self.g, "TargetTracker08"), "hangar-one-exit")?;
        self.hold(180, None)?;
        self.go(target(&self.g, "TargetTracker09"), "middle-hallway")?;
        self.go(target(&self.g, "TargetTracker10"), "hangar-two-entry")?;
        self.hold(600, None)?;
        self.interaction(4)?;
        self.go(target(&self.g, "TargetTracker11"), "hangar-two-exit")?;
        self.go(target(&self.g, "TargetTracker12"), "final-hallway")?;
        self.hold(600, None)?;
        self.go(target(&self.g, "TargetTracker13"), "balcony")?;
        let exit = self
            .g
            .map
            .as_ref()
            .unwrap()
            .gameplay
            .as_ref()
            .unwrap()
            .assets
            .exit;
        self.go(exit, "exit")?;
        if self.g.state.phase != rc_game::Phase::Won {
            return Err("No completion".into());
        }
        Ok(())
    }
    fn save(&self, result: &Result<(), String>) -> Result<(), Box<dyn std::error::Error>> {
        let mut bytes = b"RCINPUT1".to_vec();
        bytes.extend((self.inputs.len() as u32 / 20).to_le_bytes());
        bytes.extend(&self.inputs);
        fs::write(Path::new(OUT).join("input-replay.bin"), bytes)?;
        fs::write(
            Path::new(OUT).join("final-state.json"),
            serde_json::to_vec_pretty(&self.g.state)?,
        )?;
        fs::write(
            Path::new(OUT).join("validation.json"),
            serde_json::to_vec_pretty(
                &serde_json::json!({"passed":result.is_ok(),"error":result.as_ref().err(),"stage":self.stage,"inputs":self.inputs.len()/20,"pose_resource_event_assignments":false,"entry":"Actual GEO_01A handoff fixture","continuous_map2_controller_inputs":true}),
            )?,
        )?;
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(OUT)?;
    let scene = if Path::new("analysis/mission18-map2-route/delivery/geo_01b.rcplay").exists() {
        "analysis/mission18-map2-route/delivery/geo_01b.rcplay"
    } else {
        "analysis/mission17-map2-gameplay/delivery/geo_01b.rcplay"
    };
    let bytes = fs::read(scene)?;
    let mut g = rc_game::bundle::decode(&bytes)?;
    let h = serde_json::from_slice(&fs::read(
        "analysis/mission15-complete/delivery/playthrough/map-handoff.json",
    )?)?;
    g.restore_handoff(&h)?;
    let nav = graph(&g)?;
    println!("Graph: {} nodes {} edges", nav.nodes.len(), nav.edges.len());
    if std::env::args().any(|a| a == "--graph-only") {
        return Ok(());
    }
    if std::env::args().any(|a| a == "--probe-hangar") {
        let world = clone_world(&g, true);
        for (a, b) in [(174, 54), (411, 547), (1435, 54)] {
            let from = nav.nodes.iter().find(|n| n.original == a).unwrap().position;
            let to = nav.nodes.iter().find(|n| n.original == b).unwrap().position;
            println!("Hangar {a} -> {b}: {:?} -> {:?}", from, to);
            println!(
                "{}",
                serde_json::to_string_pretty(&world.sweep_motion(
                    from,
                    to,
                    g.profile.physics.extent
                )?)?
            );
        }
        for a in &g.world.actors {
            if a.name.contains("K_GEO4FighterDestroyed") {
                println!("Hidden wreck collider {}", a.name);
            }
        }
        return Ok(());
    }
    if std::env::args().any(|a| a == "--probe-stairs") {
        let world = clone_world(&g, true);
        let from = nav
            .nodes
            .iter()
            .find(|n| n.original == 481)
            .unwrap()
            .position;
        let to = nav
            .nodes
            .iter()
            .find(|n| n.original == 482)
            .unwrap()
            .position;
        println!("Stair segment {:?} -> {:?}", from, to);
        println!(
            "Straight sweep {}",
            serde_json::to_string_pretty(&world.sweep_motion(
                from,
                to,
                g.profile.physics.extent
            )?)?
        );
        let steps = (distance(from, to) / 16.).ceil() as usize;
        let mut previous = from;
        for i in 1..=steps {
            let p: Point =
                std::array::from_fn(|k| from[k] + (to[k] - from[k]) * i as f64 / steps as f64);
            let body = rc_package::physics::BodyState {
                position: previous,
                velocity: [(p[0] - previous[0]) / STEP, (p[1] - previous[1]) / STEP, 0.],
                grounded: true,
            };
            let options = rc_package::physics::PhysicsOptions {
                extent: g.profile.physics.extent,
                gravity: 950.,
                terminal_speed: 4000.,
                skin: 0.5,
                support_distance: 35.,
                minimum_up: 0.7,
                max_iterations: 8,
                step_height: 35.,
            };
            match world.body_tick(&body, STEP, options) {
                Ok(frame) => {
                    println!(
                        "sample {i}: desired {:?} actual {:?} grounded {}",
                        p, frame.body.position, frame.body.grounded
                    );
                    previous = frame.body.position;
                }
                Err(e) => {
                    println!("sample {i}: {e}");
                    break;
                }
            }
        }
        return Ok(());
    }

    if std::env::args().any(|a| a == "--view-last") {
        let input = fs::read(Path::new(OUT).join("input-replay.bin"))?;
        for r in input[12..].chunks_exact(20) {
            let v = |i| f32::from_le_bytes(r[i..i + 4].try_into().unwrap()) as f64;
            let b = u32::from_le_bytes(r[16..].try_into().unwrap());
            g.advance(
                STEP,
                Input {
                    forward: v(0),
                    strafe: v(4),
                    look: [v(8), v(12)],
                    fire: b & 2 != 0,
                    reload: b & 4 != 0,
                    use_action: b & 16 != 0,
                    ..Input::default()
                },
            )?;
        }
        let mut ppm = b"P6\n640 400\n255\n".to_vec();
        for p in g.render(640, 400)? {
            ppm.extend([(p >> 16) as u8, (p >> 8) as u8, p as u8]);
        }
        fs::write(Path::new(OUT).join("route-view.ppm"), ppm)?;
        return Ok(());
    }
    let mut d = Driver {
        g,
        nav,
        inputs: vec![],
        stage: "start".into(),
    };
    let result = d.run();
    d.save(&result)?;
    result?;
    Ok(())
}
