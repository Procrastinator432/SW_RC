//! Original class/state child lists linked with explicitly diagnostic global name IDs.
#[path = "../prop_probes.rs"]
mod prop_probes;
use rc_package::{
    event_lookup::EventNameSnapshot,
    notify_wall::{EmptyBaseNotifyWall, NotifyWallHandler},
    state_link::{link_states, StateLinkInput, StructLinkInput},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::Path,
};
struct Owner {
    path: String,
    parent: Option<String>,
    children: Vec<(String, String, bool, NotifyWallHandler)>,
    source: serde_json::Value,
    masks: Option<rc_package::state_masks::SerializedStateMasks>,
    defined_probe_bits: u64,
    is_state: bool,
    declaring_class: Option<String>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    let options = [
        "--native-hardcoded-names",
        "--original-masks",
        "--named-states",
        "--state-transitions",
        "--prop-transitions",
        "--anim-prop-transitions",
    ];
    if !((3..=3 + options.len()).contains(&args.len())
        && args[3..]
            .iter()
            .zip(options)
            .all(|(actual, expected)| actual == expected))
    {
        return Err(
            "Usage: rc-state-link-probe <GameData> <report.json> [--native-hardcoded-names [--original-masks [--named-states [--state-transitions [--prop-transitions [--anim-prop-transitions]]]]]]".into(),
        );
    }
    let fixed_names = args.len() >= 4;
    let original_masks = args.len() >= 5;
    let named_states = args.len() >= 6;
    let state_transitions = args.len() >= 7;
    let prop_transitions = args.len() >= 8;
    let anim_prop_transitions = args.len() == 9;
    let mut files = Vec::new();
    for folder in ["System", "Properties"] {
        for entry in fs::read_dir(Path::new(&args[1]).join(folder))? {
            let file = entry?.path();
            if file
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("u"))
            {
                files.push(file);
            }
        }
    }
    files.sort();
    let package_count = files.len();
    let mut owners = BTreeMap::new();
    let mut child_count = 0;
    let mut state_count = 0;
    for file in files {
        let data = fs::read(&file)?;
        let pkg = rc_package::read_package(&data)?;
        let package = file
            .file_stem()
            .and_then(|x| x.to_str())
            .ok_or("invalid filename")?;
        let qualify = |reference: i32| -> Result<String, String> {
            let path = pkg.object_path(reference)?;
            Ok(if reference > 0 {
                format!("{package}.{path}")
            } else {
                path
            })
        };
        for (i, export) in pkg.exports.iter().enumerate() {
            let is_state = export.class != 0
                && pkg
                    .object_path(export.class)?
                    .eq_ignore_ascii_case("Core.State");
            if export.class != 0 && !is_state {
                continue;
            }
            state_count += usize::from(is_state);
            let path = qualify(i as i32 + 1)?;
            let header = rc_package::state_children::read(&pkg, &data, i as i32 + 1)
                .map_err(|e| format!("{path}: {e}"))?;
            let declared: BTreeSet<_> = header.children.iter().map(|c| c.export_index).collect();
            // No omitted same-owner serialized UStruct export may hide an override.
            for (j, child) in pkg.exports.iter().enumerate() {
                if child.outer != i as i32 + 1 {
                    continue;
                }
                let kind = if child.class == 0 {
                    "Core.Class".to_string()
                } else {
                    pkg.object_path(child.class)?
                };
                if matches!(
                    kind.to_lowercase().as_str(),
                    "core.class" | "core.state" | "core.function" | "core.struct"
                ) && !declared.contains(&(j as i32 + 1))
                {
                    return Err(format!("{path}: omitted own struct {}", child.name).into());
                }
            }
            child_count += header.children.len();
            let mut children = Vec::new();
            let mut defined_probe_bits = 0;
            let mut probe_function_flags = Vec::new();
            for child in &header.children {
                if !child.is_struct {
                    continue;
                }
                let full = qualify(child.export_index)?;
                if original_masks && child.is_function {
                    if let Some(name) = rc_package::name_bindings::hardcoded_name(
                        &pkg.exports[child.export_index as usize - 1].name,
                    ) {
                        if (300..364).contains(&name.resolved_index) {
                            let flags = rc_package::state_masks::function_flags(
                                &pkg,
                                &data,
                                &pkg.exports[child.export_index as usize - 1],
                            )
                            .map_err(|e| format!("{full}: {e}"))?;
                            if flags & 2 != 0 {
                                defined_probe_bits |= 1u64 << (name.resolved_index - 300);
                            }
                            probe_function_flags.push(serde_json::json!({"function":full,"native_name_index":name.resolved_index,"flags":flags}));
                        }
                    }
                }
                let handler = if full.eq_ignore_ascii_case("Engine.Controller.NotifyHitWall") {
                    let function = rc_package::script::read_function(
                        &pkg,
                        &data,
                        &pkg.exports[child.export_index as usize - 1],
                    )?;
                    NotifyWallHandler::EmptyBase(EmptyBaseNotifyWall::verify(
                        &child.path,
                        &function,
                    )?)
                } else {
                    NotifyWallHandler::Unresolved
                };
                children.push((
                    full,
                    pkg.exports[child.export_index as usize - 1].name.clone(),
                    child.is_function,
                    handler,
                ));
            }
            let parent = if header.super_reference == 0 {
                None
            } else {
                Some(qualify(header.super_reference)?)
            };
            let masks = if original_masks {
                Some(
                    rc_package::state_masks::read(&pkg, &data, i as i32 + 1)
                        .map_err(|e| format!("{path}: {e}"))?,
                )
            } else {
                None
            };
            let declaring_class = if is_state {
                Some(qualify(export.outer)?)
            } else {
                None
            };
            let mut source = serde_json::json!({"path":path,"parent":parent,"package":file,"export_index":i+1,"ordered_children":header});
            if let Some(masks) = &masks {
                source["serialized_masks"] = serde_json::to_value(masks)?;
                source["own_defined_probe_bits"] = serde_json::json!(defined_probe_bits);
                source["probe_function_flags"] = serde_json::json!(probe_function_flags);
            }
            let owner = Owner {
                path: path.clone(),
                parent,
                children,
                source,
                masks,
                defined_probe_bits,
                is_state,
                declaring_class,
            };
            if owners.insert(path.to_lowercase(), owner).is_some() {
                return Err(format!("duplicate owner {path}").into());
            }
        }
    }
    let owners: Vec<_> = owners.into_values().collect();
    let indices: BTreeMap<_, _> = owners
        .iter()
        .enumerate()
        .map(|(i, o)| (o.path.to_lowercase(), i))
        .collect();
    let names: BTreeSet<_> = owners
        .iter()
        .flat_map(|o| o.children.iter().map(|c| c.1.to_lowercase()))
        .collect();
    let bindings = if fixed_names {
        Some(rc_package::name_bindings::diagnostic_bindings(&names)?)
    } else {
        None
    };
    let fixed_count = bindings
        .as_ref()
        .map_or(0, |b| b.values().filter(|v| v.fixed_native_index).count());
    let names: BTreeMap<_, _> = if let Some(bindings) = bindings {
        bindings
            .into_iter()
            .map(|(key, binding)| (key, binding.name))
            .collect()
    } else {
        names
            .into_iter()
            .enumerate()
            .map(|(i, n)| {
                (
                    n,
                    EventNameSnapshot {
                        handle: i as u32 + 1,
                        resolved_index: i as i32 + 1,
                    },
                )
            })
            .collect()
    };
    let mut states = Vec::new();
    let mut structs = Vec::new();
    let mut struct_paths = Vec::new();
    for owner in &owners {
        let parent = owner
            .parent
            .as_ref()
            .map(|p| {
                indices
                    .get(&p.to_lowercase())
                    .copied()
                    .ok_or_else(|| format!("missing parent {p}"))
            })
            .transpose()?;
        let mut direct_structs = Vec::new();
        for (path, name, is_function, handler) in &owner.children {
            let id = names[&name.to_lowercase()];
            direct_structs.push(structs.len());
            struct_paths.push(path.clone());
            structs.push(StructLinkInput {
                name: id,
                is_function_class: *is_function,
                handler: *handler,
            });
        }
        states.push(StateLinkInput {
            parent,
            direct_structs,
        });
    }
    let graph = link_states(&states, &structs)?;
    let controller = *indices
        .get("engine.controller")
        .ok_or("missing Controller")?;
    let notify_id = *names
        .get("notifyhitwall")
        .ok_or("missing NotifyHitWall name")?;
    let name = notify_id;
    let mut controllers = Vec::new();
    for (i, owner) in owners.iter().enumerate() {
        let mut next = Some(i);
        let mut derives = false;
        while let Some(index) = next {
            if index == controller {
                derives = true;
                break;
            }
            next = states[index].parent;
        }
        if !derives {
            continue;
        }
        let selection = graph
            .find_function(i, None, false, name)?
            .ok_or("Controller lookup missing NotifyHitWall")?;
        let selected = &struct_paths[selection.struct_index];
        if !selected.eq_ignore_ascii_case("Engine.Controller.NotifyHitWall") {
            return Err(format!("unexpected override {selected}").into());
        }
        let result = graph.notify_hit_wall(i, None, name, None)?;
        controllers
            .push(serde_json::json!({"class":owner.path,"selected":selected,"result":result}));
    }
    let mut report = serde_json::json!({"packages":package_count,"classes":owners.len()-state_count,"states":state_count,
        "ordered_fields":child_count,"direct_structs":structs.len(),"allocated_tables":graph.states.iter().filter(|s|s.hash_table.is_some()).count(),
        "diagnostic_global_names":names.len(),"controller_class_lookups":controllers,"owners":owners.iter().map(|o|&o.source).collect::<Vec<_>>(),
        "scope":"Original serialized ordered class/state children and parents; complete own UStruct export membership checked. Fresh tables use deterministic diagnostic global name IDs, NOT native FName indices/handles or hash layout. No active state or probe mask supplied. Base Controller.NotifyHitWall verified from original bytecode; all other handlers unresolved. No VM/native overrides/runtime mutation or Android coverage."});
    if fixed_names {
        let bit = 1u64 << (name.resolved_index - 300);
        let enabled = graph.notify_hit_wall(controller, None, name, Some(bit))?;
        let masked = graph.notify_hit_wall(controller, None, name, Some(0))?;
        let mut initialized_frames = Vec::new();
        for lookup in &controllers {
            let path = lookup["class"].as_str().ok_or("invalid controller path")?;
            let class = indices[&path.to_lowercase()];
            let frame = rc_package::probe_frame::ProbeFrame::init_execution(class);
            let selection = graph
                .find_function(class, Some(frame.state_node), false, name)?
                .ok_or("missing initialized handler")?;
            let result = frame.notify_hit_wall(&graph, name)?;
            let mut disabled = frame;
            disabled.disable(name.resolved_index)?;
            let disabled_result = disabled.notify_hit_wall(&graph, name)?;
            initialized_frames.push(serde_json::json!({"class":path,"initial_frame":frame,"source":selection.source,"selected":struct_paths[selection.struct_index],"result":result,"after_disable":disabled,"disabled_result":disabled_result}));
        }
        report["initialized_probe_frames"] = serde_json::json!(initialized_frames);
        report["name_bindings"] = serde_json::json!({"fixed_native_indices":fixed_count,"diagnostic_dynamic_indices":names.len()-fixed_count,"notify_index":name.resolved_index,"notify_bucket":name.resolved_index&127,"notify_mask_bit":name.resolved_index-300,"base_with_supplied_enabled_mask":enabled,"base_with_supplied_disabled_mask":masked,"handles":"opaque stable tokens; no native entry pointers"});
        report["scope"] = serde_json::json!("Original ordered class/state children and parents. Verified hardcoded name indices; remaining indices diagnostic and handles opaque. Additional initialized-frame probes use native InitExecution StateNode=Class/mask=all bits and Disable; no actual game instance. Other mask probes remain supplied. No dynamic native registration, actual state transitions/callbacks, VM/native overrides or Android coverage.");
    }
    if original_masks {
        let mut class_masks = vec![None; owners.len()];
        for start in 0..owners.len() {
            if owners[start].is_state {
                continue;
            }
            let mut chain = Vec::new();
            let mut next = Some(start);
            while let Some(i) = next {
                if class_masks[i].is_some() {
                    break;
                }
                if owners[i].is_state {
                    return Err("class inherits from state".into());
                }
                chain.push(i);
                next = states[i].parent;
            }
            for i in chain.into_iter().rev() {
                class_masks[i] = Some(
                    states[i].parent.and_then(|p| class_masks[p]).unwrap_or(0)
                        | owners[i].defined_probe_bits,
                );
            }
        }
        let mut compositions = Vec::new();
        let mut controller_states = Vec::new();
        for (i, owner) in owners.iter().enumerate() {
            let masks = owner.masks.as_ref().ok_or("missing original masks")?;
            let class = if let Some(path) = &owner.declaring_class {
                *indices
                    .get(&path.to_lowercase())
                    .ok_or("missing declaring class")?
            } else {
                i
            };
            let class_probe = class_masks[class].ok_or("missing rebuilt class mask")?;
            let state_probe = if owner.is_state {
                masks.probe_mask
            } else {
                class_probe
            };
            let frame = rc_package::probe_frame::ProbeFrame::init_execution(class)
                .resolved_state_masks(
                    i,
                    rc_package::probe_frame::StateProbeMasks {
                        class_probe,
                        state_probe,
                        state_ignore: masks.ignore_mask,
                    },
                );
            compositions.push(serde_json::json!({"owner":owner.path,"object_class":owners[class].path,"serialized_probe":masks.probe_mask,"runtime_class_probe":class_probe,"runtime_state_probe":state_probe,"ignore_mask":masks.ignore_mask,"composed_mask":frame.probe_mask}));
            if owner.is_state {
                let mut next = Some(class);
                let mut derives = false;
                while let Some(p) = next {
                    if p == controller {
                        derives = true;
                        break;
                    }
                    next = states[p].parent;
                }
                if derives {
                    controller_states.push(serde_json::json!({"state":owner.path,"class":owners[class].path,"frame":frame,"notify":frame.notify_hit_wall(&graph,name)?}));
                }
            }
        }
        report["original_mask_compositions"] = serde_json::json!(compositions);
        report["controller_declared_state_probes"] = serde_json::json!(controller_states);
        report["scope"]=serde_json::json!("Original serialized state masks and function flags traversed with bounded layout reader. Class probe masks rebuilt parent-first from direct functions with flags&2; stored class probe masks not used as runtime masks. Resolved-state mask compositions for every owner; Controller state probes use explicitly selected declaring class/state, not live/auto state selection or GotoState callbacks. Fixed names native, remaining names diagnostic, handles opaque. No VM, runtime overrides or Android execution.");
        if named_states {
            use rc_package::state_selection::{
                NamedStateLookup, StateStructType, StateTargetRoute,
            };
            let types: Vec<_> = struct_paths
                .iter()
                .map(|path| {
                    indices
                        .get(&path.to_lowercase())
                        .map_or(StateStructType::NotState, |&node| {
                            if owners[node].is_state {
                                StateStructType::StateNode(node)
                            } else {
                                StateStructType::Unresolved
                            }
                        })
                })
                .collect();
            let lookup = NamedStateLookup {
                graph: &graph,
                struct_types: &types,
            };
            let requested_names: BTreeSet<_> = owners
                .iter()
                .filter(|o| o.is_state)
                .map(|o| o.path.rsplit('.').next().unwrap().to_lowercase())
                .collect();
            let mut selected = Vec::new();
            let mut fallbacks = Vec::new();
            let mut misses = 0;
            for controller in &controllers {
                let path = controller["class"].as_str().ok_or("invalid controller")?;
                let class = indices[&path.to_lowercase()];
                for requested in &requested_names {
                    let requested_name =
                        *names.get(requested).ok_or("missing state name binding")?;
                    let target = lookup.resolve_named_target(class, requested_name)?;
                    if target.route == StateTargetRoute::ClassFallback {
                        misses += 1;
                        continue;
                    }
                    let node = target.state_node;
                    let state_masks = owners[node]
                        .masks
                        .as_ref()
                        .ok_or("missing selected masks")?;
                    let class_probe = class_masks[class].ok_or("missing object class mask")?;
                    let state_probe = if owners[node].is_state {
                        state_masks.probe_mask
                    } else {
                        class_masks[node].ok_or("missing class target mask")?
                    };
                    let frame = rc_package::probe_frame::ProbeFrame::init_execution(class)
                        .resolved_state_masks(
                            node,
                            rc_package::probe_frame::StateProbeMasks {
                                class_probe,
                                state_probe,
                                state_ignore: state_masks.ignore_mask,
                            },
                        );
                    selected.push(serde_json::json!({"class":path,"requested":requested,"selected":owners[node].path,"target":target,"frame":frame,"notify":frame.notify_hit_wall(&graph,name)?}));
                }
                for (request, requested_name) in [
                    (
                        "None",
                        rc_package::name_bindings::hardcoded_name("None").ok_or("missing None")?,
                    ),
                    ("NotifyHitWall", name),
                ] {
                    let target = lookup.resolve_named_target(class, requested_name)?;
                    if target.route != StateTargetRoute::ClassFallback || target.state_node != class
                    {
                        return Err("unexpected fallback target".into());
                    }
                    fallbacks.push(
                        serde_json::json!({"class":path,"requested":request,"target":target}),
                    );
                }
            }
            report["named_controller_state_lookups"] = serde_json::json!({"requested_state_names":requested_names,"matched":selected,"missing_name_class_fallbacks":misses,"none_and_nonstate_fallbacks":fallbacks});
            let mut direct_states = Vec::new();
            for owner in &states {
                let mut ordered = Vec::new();
                for &child in &owner.direct_structs {
                    match types[child] {
                        StateStructType::StateNode(node) => ordered.push(node),
                        StateStructType::NotState => {}
                        StateStructType::Unresolved => {
                            return Err("unresolved Auto iterator type".into())
                        }
                    }
                }
                direct_states.push(ordered);
            }
            let state_flags: Vec<_> = owners
                .iter()
                .map(|o| {
                    if o.is_state {
                        o.masks.as_ref().map(|m| m.state_flags)
                    } else {
                        None
                    }
                })
                .collect();
            let auto = rc_package::state_selection::AutoStateLookup {
                graph: &graph,
                direct_states: &direct_states,
                state_flags: &state_flags,
            };
            let controller_names: BTreeSet<_> = controllers
                .iter()
                .map(|c| c["class"].as_str().unwrap().to_lowercase())
                .collect();
            let mut auto_targets = Vec::new();
            let mut auto_controllers = Vec::new();
            let state_names: Vec<_> = owners
                .iter()
                .map(|o| {
                    if o.is_state {
                        names
                            .get(&o.path.rsplit('.').next().unwrap().to_lowercase())
                            .copied()
                    } else {
                        None
                    }
                })
                .collect();
            let mut transitions = Vec::new();
            let mut exit_probes = Vec::new();
            for (class, owner) in owners.iter().enumerate() {
                if owner.is_state {
                    continue;
                }
                let target = auto.resolve_auto_target(class)?;
                let node = target.state_node;
                let masks = owners[node]
                    .masks
                    .as_ref()
                    .ok_or("missing Auto target masks")?;
                let class_probe = class_masks[class].ok_or("missing Auto class probe mask")?;
                let state_probe = if owners[node].is_state {
                    masks.probe_mask
                } else {
                    class_masks[node].ok_or("missing fallback class mask")?
                };
                let frame = rc_package::probe_frame::ProbeFrame::init_execution(class)
                    .resolved_state_masks(
                        node,
                        rc_package::probe_frame::StateProbeMasks {
                            class_probe,
                            state_probe,
                            state_ignore: masks.ignore_mask,
                        },
                    );
                if state_transitions {
                    use rc_package::state_transition::{goto_state, StateEvent, StateExecution};
                    let selected = auto.resolve_target(
                        class,
                        rc_package::name_bindings::hardcoded_name("Auto").ok_or("missing Auto")?,
                        &types,
                        &state_names,
                    )?;
                    if selected.selection.state_node != node
                        || selected.selection.route != target.route
                    {
                        return Err("unified Auto resolver mismatch".into());
                    }
                    let mut execution = StateExecution {
                        probe: rc_package::probe_frame::ProbeFrame::init_execution(class),
                        node: class,
                        code: None,
                        latent_action: 7,
                        object_flags: 0,
                    };
                    let mut callback_request = None;
                    let result = goto_state(
                        Some(&mut execution),
                        &selected,
                        rc_package::probe_frame::StateProbeMasks {
                            class_probe,
                            state_probe,
                            state_ignore: masks.ignore_mask,
                        },
                        &state_names,
                        &mut |s, event| {
                            let event_name =
                                rc_package::name_bindings::hardcoded_name(match event {
                                    StateEvent::BeginState => "BeginState",
                                    StateEvent::EndState => "EndState",
                                })
                                .ok_or("missing lifecycle name")?;
                            let function = graph.find_function(
                                class,
                                Some(s.probe.state_node),
                                false,
                                event_name,
                            )?;
                            callback_request = Some(
                                serde_json::json!({"event":event,"selected_function":function.map(|f|&struct_paths[f.struct_index])}),
                            );
                            Err("unresolved lifecycle callback".into())
                        },
                    );
                    let outcome = match result {
                        Ok(value) => serde_json::to_value(value)?,
                        Err(error) if error == "unresolved lifecycle callback" => {
                            serde_json::json!("UnresolvedCallback")
                        }
                        Err(error) => return Err(error.into()),
                    };
                    transitions.push(serde_json::json!({"class":owner.path,"target":selected,"outcome":outcome,"callback_request":callback_request,"execution":execution}));
                    if target.route == StateTargetRoute::AutoState {
                        let none = rc_package::name_bindings::hardcoded_name("None")
                            .ok_or("missing None")?;
                        let selected = auto.resolve_target(class, none, &types, &state_names)?;
                        let fallback = owners[class]
                            .masks
                            .as_ref()
                            .ok_or("missing fallback masks")?;
                        // A separately supplied active-state snapshot, not a continuation
                        // past any unresolved BeginState callback above.
                        let mut execution = StateExecution {
                            probe: frame,
                            node,
                            code: Some(123),
                            latent_action: 7,
                            object_flags: rc_package::state_transition::STATE_CHANGED,
                        };
                        let mut callback_request = None;
                        let result = goto_state(
                            Some(&mut execution),
                            &selected,
                            rc_package::probe_frame::StateProbeMasks {
                                class_probe,
                                state_probe: class_probe,
                                state_ignore: fallback.ignore_mask,
                            },
                            &state_names,
                            &mut |s, event| {
                                if event != StateEvent::EndState {
                                    return Err("unexpected exit callback".into());
                                }
                                let event_name =
                                    rc_package::name_bindings::hardcoded_name("EndState")
                                        .ok_or("missing EndState")?;
                                let function = graph.find_function(
                                    class,
                                    Some(s.probe.state_node),
                                    false,
                                    event_name,
                                )?;
                                callback_request = Some(
                                    serde_json::json!({"event":event,"selected_function":function.map(|f|&struct_paths[f.struct_index])}),
                                );
                                Err("unresolved lifecycle callback".into())
                            },
                        );
                        let outcome = match result {
                            Ok(value) => serde_json::to_value(value)?,
                            Err(error) if error == "unresolved lifecycle callback" => {
                                serde_json::json!("UnresolvedCallback")
                            }
                            Err(error) => return Err(error.into()),
                        };
                        exit_probes.push(serde_json::json!({"class":owner.path,"supplied_state":owners[node].path,"outcome":outcome,"callback_request":callback_request,"execution":execution}));
                    }
                }
                auto_targets.push(serde_json::json!({"class":owner.path,"selected":owners[node].path,"target":target,"frame":frame}));
                if controller_names.contains(&owner.path.to_lowercase()) {
                    auto_controllers.push(serde_json::json!({"class":owner.path,"selected":owners[node].path,"target":target,"frame":frame,"notify":frame.notify_hit_wall(&graph,name)?}));
                }
            }
            report["auto_class_targets"] = serde_json::json!(auto_targets);
            report["auto_controller_probes"] = serde_json::json!(auto_controllers);
            report["scope"]=serde_json::json!("Named and Auto target phases from original ordered class/state children, StateFlags and masks. Auto selects first flags&2 State in forward own-then-parent iterator order, without named re-resolution. Core.State exports accepted, unknown metaclass candidates error. Target/mask selection only; no EndState/BeginState callbacks, actual running game state, VM/native overrides or Android execution. Fixed names native, remaining names diagnostic, handles opaque.");
            if state_transitions {
                report["auto_transition_probes"] = serde_json::json!(transitions);
                report["resolved_auto_exit_probes"] = serde_json::json!(exit_probes);
                report["scope"]=serde_json::json!("Base GotoState snapshot control flow with unified named/Auto target resolution. Original auto-target and mask metadata for every class. Start from InitExecution probe mask/class plus explicit diagnostic latent_action=7/object_flags=0; these two values are supplied, not inferred initialization. Stop at any requested lifecycle callback, recording its selected function without executing it. Synthetic tests cover callback recursion/preemption; no original lifecycle bytecode/native override execution, live game instance, mutable metadata, VM or Android coverage. Dynamic names diagnostic; handles opaque.");
                if prop_transitions {
                    let context = prop_probes::Context {
                        owners: &owners,
                        indices: &indices,
                        graph: &graph,
                        struct_paths: &struct_paths,
                        types: &types,
                        auto: &auto,
                        state_names: &state_names,
                        class_masks: &class_masks,
                    };
                    report["prop_transition_probes"] = context.run(Path::new(&args[1]))?;
                    report["scope"]=serde_json::json!("Additional verified base Prop BeginState branch callbacks on original lookup/mask metadata, nested script GotoState and proven missing label tables. Broadcast and other handlers remain unresolved; input scalars diagnostic, no generic VM/ProcessEvent/virtual override or Android execution. Existing probes retain their original scope.");
                    if anim_prop_transitions {
                        report["anim_prop_transition_probes"] =
                            context.run_anim(Path::new(&args[1]))?;
                        report["scope"]=serde_json::json!("Additional exact AnimProp BeginState wrapper: fixed Prop superhandler followed by healthy-animation tail even after state redirect. None completes; Broadcast and native PlayAnim/LoopAnim requests stop explicitly. Inputs supplied, no generic VM/actual animation/default/native override or Android execution. Previous probes retain their original scope.");
                    }
                }
            }
        }
    }
    fs::write(&args[2], serde_json::to_vec_pretty(&report)?)?;
    println!(
        "{} classes, {state_count} states, {child_count} ordered fields; {} Controller lookups",
        owners.len() - state_count,
        controllers.len()
    );
    Ok(())
}
