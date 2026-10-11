//! Compare prepared Rust planning with bounded original instruction recordings.
use rc_package::{defensive_jump as d, geonosian_attack};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{env, fs};
#[derive(Deserialize)]
struct SearchCase {
    nodes: Vec<d::Node>,
    query: d::Query,
    expected: Value,
}
#[derive(Deserialize)]
struct PlanInput {
    origin: [f32; 3],
    perch: Option<d::Target>,
    jump: Option<d::Target>,
    clear: bool,
    reachable: bool,
    anchor: Option<u32>,
}
#[derive(Deserialize)]
struct PlanCase {
    input: PlanInput,
    expected: Value,
}
#[derive(Deserialize)]
struct PostCase {
    player: bool,
    probability: f32,
    player_probability: f32,
    random: u16,
    expected: Value,
}
#[derive(Deserialize)]
struct Recording {
    provenance: Value,
    searches: Vec<SearchCase>,
    plans: Vec<PlanCase>,
    post_melee: Vec<PostCase>,
}
struct Host<'a> {
    input: &'a PlanInput,
    calls: Vec<Value>,
}
impl d::Host for Host<'_> {
    fn find_nearest(&mut self, kind: d::Kind) -> Option<d::Target> {
        self.calls.push(json!({"Find":kind}));
        match kind {
            d::Kind::Perch => self.input.perch,
            d::Kind::JumpDest => self.input.jump,
        }
    }
    fn reserve(&mut self, node: u32) {
        self.calls.push(json!({"Reserve":node}));
    }
    fn clear_to(&mut self, node: u32) -> bool {
        self.calls.push(json!({"Clear":node}));
        self.input.clear
    }
    fn reachable_to_enemy(&mut self, node: u32) -> bool {
        self.calls.push(json!({"Reachable":node}));
        self.input.reachable
    }
    fn own_anchor(&mut self) -> Option<u32> {
        self.calls.push(json!("OwnAnchor"));
        self.input.anchor
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-defensive-check RECORDING OUTPUT".into());
    }
    let recording: Recording = serde_json::from_slice(&fs::read(&args[1])?)?;
    if recording.searches.is_empty()
        || recording.plans.is_empty()
        || recording.post_melee.is_empty()
    {
        return Err("Empty defensive recording".into());
    }
    let mut selected = 0;
    for (i, c) in recording.searches.iter().enumerate() {
        let actual = d::nearest(&c.nodes, c.query);
        selected += usize::from(actual.selected.is_some());
        if serde_json::to_value(actual)? != c.expected {
            return Err(format!("Native nearest-node mismatch at case {i}").into());
        }
    }
    for (i, c) in recording.plans.iter().enumerate() {
        let mut flags = d::Flags {
            must_jump: true,
            is_melee_attacking: true,
        };
        let mut host = Host {
            input: &c.input,
            calls: vec![],
        };
        let actual = d::plan(&mut flags, c.input.origin, &mut host);
        let actual = json!({"selected":actual.selected,"goals":actual.goals,"clear_strafe":actual.clear_strafe,"flags":flags,"calls":host.calls});
        if actual != c.expected {
            return Err(format!(
                "Native goal plan mismatch at case {i}: actual={actual}; expected={}",
                c.expected
            )
            .into());
        }
    }
    for (i, c) in recording.post_melee.iter().enumerate() {
        let mut draws = 0;
        let requested = geonosian_attack::request_defensive_jump(
            c.player,
            c.probability,
            c.player_probability,
            || {
                draws += 1;
                c.random
            },
        );
        if json!({"requested":requested,"draws":draws}) != c.expected {
            return Err(format!("Native post-melee random mismatch at case {i}").into());
        }
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &json!({"passed":true,"searches":recording.searches.len(),"searches_with_destination":selected,"plans":recording.plans.len(),"post_melee":recording.post_melee.len(),"provenance":recording.provenance,"scope":"Prepared host snapshots and successful allocations; exact query/goal order and flags; no live navigation, executable goal queue, flight or Android integration."}),
        )?,
    )?;
    println!(
        "Prepared native defensive searches, goal order, flags and post-melee random draws passed"
    );
    Ok(())
}
