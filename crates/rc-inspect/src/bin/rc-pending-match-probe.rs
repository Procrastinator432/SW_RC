//! Verify and run the exact PendingMatch scalar handler on saved lookup snapshots.
use rc_package::{
    pending_match::{PendingMatchBegin, PendingMatchSnapshot},
    probe_frame::{ProbeFrame, StateProbeMasks},
    state_selection::{NamedStateTarget, ResolvedStateTarget, StateTargetRoute},
    state_transition::{goto_state, GotoStateResult, StateEvent, StateExecution},
};
use std::{env, fs, path::Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 4 {
        return Err(
            "Usage: rc-pending-match-probe <GameData> <state-transitions.json> <report.json>"
                .into(),
        );
    }
    let data_root = Path::new(&args[1]);
    let original: serde_json::Value = serde_json::from_slice(&fs::read(&args[2])?)?;
    let mp_data = fs::read(data_root.join("System/mpgame.u"))?;
    let mp = rc_package::read_package(&mp_data)?;
    let engine_data = fs::read(data_root.join("System/engine.u"))?;
    let engine = rc_package::read_package(&engine_data)?;
    let find = |pkg: &rc_package::Package, path: &str| -> Result<usize, String> {
        for i in 0..pkg.exports.len() {
            if pkg.object_path(i as i32 + 1)? == path {
                return Ok(i);
            }
        }
        Err(format!("missing export {path}"))
    };
    let function_index = find(&mp, "DMGame.PendingMatch.BeginState")?;
    let function = rc_package::script::read_function(&mp, &mp_data, &mp.exports[function_index])?;
    let waiting_fields = rc_package::classes::fields(
        &engine,
        &engine_data,
        &engine.exports[find(&engine, "GameInfo")?],
    )?;
    let startup_fields =
        rc_package::classes::fields(&mp, &mp_data, &mp.exports[find(&mp, "DMGame")?])?;
    let waiting = waiting_fields
        .iter()
        .find(|f| f.name == "bWaitingToStartMatch")
        .ok_or("missing waiting field")?;
    let startup = startup_fields
        .iter()
        .find(|f| f.name == "StartupStage")
        .ok_or("missing startup field")?;
    let handler = PendingMatchBegin::verify(
        "mpgame.DMGame.PendingMatch.BeginState",
        &function,
        waiting,
        startup,
    )?;
    let probes = original["auto_transition_probes"]
        .as_array()
        .ok_or("missing original transition probes")?;
    let owners = original["owners"].as_array().ok_or("missing owners")?;
    let mut executed = Vec::new();
    for probe in probes {
        if probe["callback_request"]["selected_function"] != "mpgame.DMGame.PendingMatch.BeginState"
        {
            continue;
        }
        if probe["callback_request"]["event"] != "BeginState"
            || probe["outcome"] != "UnresolvedCallback"
        {
            return Err("unexpected PendingMatch input phase".into());
        }
        let class = probe["execution"]["probe"]["object_class"]
            .as_u64()
            .ok_or("missing class index")? as usize;
        let node = probe["execution"]["probe"]["state_node"]
            .as_u64()
            .ok_or("missing target index")? as usize;
        if owners.get(class).and_then(|o| o["path"].as_str()) != probe["class"].as_str()
            || !owners
                .get(node)
                .and_then(|o| o["path"].as_str())
                .is_some_and(|path| {
                    matches!(
                        path,
                        "mpgame.DMGame.PendingMatch" | "mpgame.TDGame.PendingMatch"
                    )
                })
        {
            return Err("inconsistent original target identity".into());
        }
        let target = ResolvedStateTarget {
            selection: NamedStateTarget {
                state_node: node,
                route: StateTargetRoute::AutoState,
            },
            name: rc_package::event_lookup::EventNameSnapshot {
                handle: probe["target"]["name"]["handle"]
                    .as_u64()
                    .ok_or("missing name handle")?
                    .try_into()?,
                resolved_index: probe["target"]["name"]["resolved_index"]
                    .as_i64()
                    .ok_or("missing name index")?
                    .try_into()?,
            },
        };
        let mask = probe["execution"]["probe"]["probe_mask"]
            .as_u64()
            .ok_or("missing original mask")?;
        let mut cases = Vec::new();
        for (waiting, stage) in [(false, 0), (false, 255), (true, 7), (true, 255)] {
            let before = PendingMatchSnapshot {
                waiting_to_start_match: waiting,
                startup_stage: stage,
            };
            let mut scalar = before;
            let mut execution = StateExecution {
                probe: ProbeFrame::init_execution(class),
                node: class,
                code: None,
                latent_action: 7,
                object_flags: 0,
            };
            let mut calls = 0;
            // Use the independently validated composed mask from the saved report.
            let result = goto_state(
                Some(&mut execution),
                &target,
                StateProbeMasks {
                    class_probe: 0,
                    state_probe: mask,
                    state_ignore: u64::MAX,
                },
                &[],
                &mut |_, event| {
                    if event != StateEvent::BeginState {
                        return Err("unexpected PendingMatch event".into());
                    }
                    calls += 1;
                    handler.execute(&mut scalar);
                    Ok(())
                },
            )?;
            if result != GotoStateResult::Success || calls != 1 {
                return Err("PendingMatch callback did not complete".into());
            }
            cases.push(serde_json::json!({"before":before,"after":scalar,"outcome":result,"callback_calls":calls,"execution":execution}));
        }
        executed.push(serde_json::json!({"class":probe["class"],"selected_state":owners[node]["path"],"cases":cases}));
    }
    if executed.len() != 5 {
        return Err("unexpected PendingMatch class count".into());
    }
    fs::write(
        &args[3],
        serde_json::to_vec_pretty(&serde_json::json!({
            "input_lookup_report":args[2],"handler":"mpgame.DMGame.PendingMatch.BeginState","export_index":function_index+1,
            "function":function,"waiting_field":waiting,"startup_field":startup,"probes":executed,
            "scope":"Exact reviewed scalar callback on five saved original Auto lookup/mask snapshots, four supplied scalar inputs per class. Original MPGame function and Engine/MPGame property types checked; replicated properties rejected. Sets host waiting=true/startup=0 and completes base transition. No generic VM/ProcessEvent/native override/network hooks, live startup/defaults/label execution or Android coverage. Other original callbacks remain unresolved."
        }))?,
    )?;
    println!(
        "{} PendingMatch classes, {} completed scalar callback cases",
        executed.len(),
        executed.len() * 4
    );
    Ok(())
}
