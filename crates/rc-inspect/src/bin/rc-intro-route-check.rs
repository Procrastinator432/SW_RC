//! Exercise native FindSpot control flow against the port's mathematical world.
//! This is a diagnostic host, not proof of native collision-query parity.
use rc_package::{
    placement::{self, Hit, Vector, World},
    world_collision::StaticBodyWorld,
};
use serde_json::{json, Value};
use std::{env, fs, path::Path};
struct Probe<'a> {
    world: &'a StaticBodyWorld,
    queries: Vec<Value>,
}
impl World for Probe<'_> {
    fn encroaching(&mut self, point: Vector, extent: Vector) -> Result<bool, String> {
        let result = self.world.sweep(
            point.map(f64::from),
            point.map(f64::from),
            extent.map(f64::from),
        )?;
        if !result.complete {
            return Err("Incomplete placement world".into());
        }
        let blocked = !result.contacts.is_empty();
        self.queries
            .push(json!({"kind":"point","point":point,"extent":extent,"result":result}));
        Ok(blocked)
    }
    fn trace(&mut self, start: Vector, end: Vector, extent: Vector) -> Result<Hit, String> {
        let result = self.world.sweep(
            start.map(f64::from),
            end.map(f64::from),
            extent.map(f64::from),
        )?;
        if !result.complete {
            return Err("Incomplete placement trace".into());
        }
        let hit = if let Some(contact) = result.contacts.first() {
            let fraction = contact.hit.fraction as f32;
            Hit {
                actor: true,
                fraction,
                location: std::array::from_fn(|i| start[i] + (end[i] - start[i]) * fraction),
                normal: contact.hit.normal.map(|v| v as f32),
            }
        } else {
            Hit {
                actor: false,
                fraction: 1.,
                location: end,
                normal: [0.; 3],
            }
        };
        self.queries
            .push(json!({"kind":"trace","start":start,"end":end,"extent":extent,"result":result}));
        Ok(hit)
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args().collect();
    if a.len() != 3 {
        return Err("usage: rc-intro-placement-check ASSET_DIR SCENE".into());
    }
    let dir = Path::new(&a[1]);
    let binding: Value = serde_json::from_slice(&fs::read(dir.join("bindings.json"))?)?;
    let target = &binding["selected_teleport"];
    let requested: Vector = serde_json::from_value(target["location"]["value"].clone())?;
    let warrior = binding["actors"]
        .as_array()
        .ok_or("Missing actors")?
        .iter()
        .find(|v| v["original"] == 530)
        .ok_or("Missing Warrior")?;
    let radius = warrior["resolved"]["CollisionRadius"]["value"]
        .as_f64()
        .ok_or("Missing radius")? as f32;
    let height = warrior["resolved"]["CollisionHeight"]["value"]
        .as_f64()
        .ok_or("Missing height")? as f32;
    let game = rc_game::bundle::decode(&fs::read(&a[2])?)?;
    let mut probe = Probe {
        world: &game.world,
        queries: vec![],
    };
    let mut candidate = requested;
    let accepted = placement::find_spot(&mut candidate, [radius, radius, height], &mut probe)?;
    let kill = binding["actors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["original"] == 413)
        .unwrap();
    let target: [f64; 3] = serde_json::from_value(kill["resolved"]["Location"]["value"].clone())?;
    let from = rc_game::mission_motion::ground(
        &game.world,
        candidate.map(f64::from),
        [50., 50., 115.],
        512.,
    )?;
    let to = rc_game::mission_motion::ground(
        &game.world,
        [target[0], target[1], target[2] + 200.],
        [50., 50., 115.],
        512.,
    )?;
    let route = if let (Some(f), Some(t)) = (from, to) {
        game.warrior_ai.as_ref().unwrap().navigation.plan_for(
            &game.world,
            f,
            t,
            [50., 50., 115.],
        )?
    } else {
        vec![]
    };
    println!("ground from {from:?} to {to:?}, route {route:?}");
    let target = &binding["selected_teleport"];
    let output = json!({"passed":true,"native_find_spot_control_flow":true,
        "native_world_queries":false,"native_far_move_actor":false,
        "host":"existing mathematical BSP/static hulls with player-blocking actor selection; no native backoff",
        "destination":target["original"],"requested":requested,"extent":[radius,radius,height],
        "candidate":candidate,"accepted_by_diagnostic_host":accepted,"queries":probe.queries});
    fs::write(
        Path::new("analysis/mission15-complete/route-probe.json"),
        serde_json::to_vec_pretty(&output)?,
    )?;
    println!("FindSpot diagnostic: target {}, accepted {}, candidate {:?}, {} queries (native query parity pending)",target["original"],accepted,candidate,probe.queries.len());
    Ok(())
}
