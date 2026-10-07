//! Original Prop target/handler chain; supplied health/display inputs, no Broadcast.
use crate::Owner;
use rc_package::{
    event_lookup::{EventLookupSnapshot, EventNameSnapshot},
    probe_frame::{ProbeFrame, StateProbeMasks},
    prop_state::{
        AnimPropBegin, HealthyAnimationSnapshot, PropBegin, PropBeginAction, PropBeginSnapshot,
    },
    script_state::{script_goto_state, ScriptStateRequest},
    state_selection::{AutoStateLookup, StateStructType},
    state_transition::{goto_state, StateEvent, StateExecution},
};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::Path,
};

pub struct Context<'a> {
    pub owners: &'a [Owner],
    pub indices: &'a BTreeMap<String, usize>,
    pub graph: &'a EventLookupSnapshot,
    pub struct_paths: &'a [String],
    pub types: &'a [StateStructType],
    pub auto: &'a AutoStateLookup<'a>,
    pub state_names: &'a [Option<EventNameSnapshot>],
    pub class_masks: &'a [Option<u64>],
}
impl Context<'_> {
    fn masks(&self, class: usize, node: usize) -> Result<StateProbeMasks, String> {
        let owner = &self.owners[node];
        let raw = owner.masks.as_ref().ok_or("missing Prop state masks")?;
        Ok(StateProbeMasks {
            class_probe: self.class_masks[class].ok_or("missing Prop class mask")?,
            state_probe: if owner.is_state {
                raw.probe_mask
            } else {
                self.class_masks[node].ok_or("missing Prop fallback mask")?
            },
            state_ignore: raw.ignore_mask,
        })
    }
    fn handler(&self, class: usize, state: usize, event: StateEvent) -> Result<&str, String> {
        let name = rc_package::name_bindings::hardcoded_name(match event {
            StateEvent::BeginState => "BeginState",
            StateEvent::EndState => "EndState",
        })
        .ok_or("missing Prop event name")?;
        let selected = self
            .graph
            .find_function(class, Some(state), false, name)?
            .ok_or("missing Prop event function")?;
        Ok(&self.struct_paths[selected.struct_index])
    }
    fn missing_label(
        &self,
        s: Option<&mut StateExecution>,
        name: EventNameSnapshot,
    ) -> Result<bool, String> {
        let Some(s) = s else {
            return Ok(false);
        };
        s.latent_action = 0;
        if name.handle != 0 {
            let mut next = Some(s.probe.state_node);
            let mut seen = HashSet::new();
            while let Some(node) = next {
                if !seen.insert(node) || seen.len() > 4096 {
                    return Err("Prop label ancestry cycle".into());
                }
                let raw = self
                    .owners
                    .get(node)
                    .and_then(|o| o.masks.as_ref())
                    .ok_or("missing Prop label metadata")?;
                if raw.label_table_offset != u16::MAX && raw.logical_script_bytes > 0 {
                    return Err("unresolved Prop label table".into());
                }
                next = self.graph.states[node].parent;
            }
        }
        s.code = None;
        Ok(false)
    }
    pub fn run(&self, game_data: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        self.run_mode(game_data, false)
    }
    pub fn run_anim(
        &self,
        game_data: &Path,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        self.run_mode(game_data, true)
    }
    fn run_mode(
        &self,
        game_data: &Path,
        anim_mode: bool,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let data = fs::read(game_data.join("System/engine.u"))?;
        let pkg = rc_package::read_package(&data)?;
        let find = |path: &str| -> Result<usize, String> {
            for i in 0..pkg.exports.len() {
                if pkg.object_path(i as i32 + 1)? == path {
                    return Ok(i);
                }
            }
            Err(format!("missing Prop export {path}"))
        };
        let fields = rc_package::classes::fields(&pkg, &data, &pkg.exports[find("Prop")?])?;
        let health = fields
            .iter()
            .find(|f| f.name == "Health")
            .ok_or("missing Prop.Health")?;
        let display = fields
            .iter()
            .find(|f| f.name == "bDisplayStateMessages")
            .ok_or("missing Prop display field")?;
        let inv_function = rc_package::script::read_function(
            &pkg,
            &data,
            &pkg.exports[find("Prop.Invulnerable.BeginState")?],
        )?;
        let dam_function = rc_package::script::read_function(
            &pkg,
            &data,
            &pkg.exports[find("Prop.Damagable.BeginState")?],
        )?;
        let inv = PropBegin::verify(
            "Prop.Invulnerable.BeginState",
            &inv_function,
            health,
            display,
        )?;
        let dam = PropBegin::verify("Prop.Damagable.BeginState", &dam_function, health, display)?;
        let (anim_handler, anim_metadata) = if anim_mode {
            let anim_fields =
                rc_package::classes::fields(&pkg, &data, &pkg.exports[find("AnimProp")?])?;
            let struct_fields = rc_package::classes::fields(
                &pkg,
                &data,
                &pkg.exports[find("AnimProp.PropAnimInfo")?],
            )?;
            let healthy = anim_fields
                .iter()
                .find(|f| f.name == "AnimHealthy")
                .ok_or("missing AnimHealthy")?;
            let name = struct_fields
                .iter()
                .find(|f| f.name == "Anim")
                .ok_or("missing PropAnimInfo.Anim")?;
            let looping = struct_fields
                .iter()
                .find(|f| f.name == "bLoop")
                .ok_or("missing PropAnimInfo.bLoop")?;
            let function = rc_package::script::read_function(
                &pkg,
                &data,
                &pkg.exports[find("AnimProp.Invulnerable.BeginState")?],
            )?;
            let token = AnimPropBegin::verify(
                "AnimProp.Invulnerable.BeginState",
                &function,
                healthy,
                name,
                looping,
            )?;
            (
                Some(token),
                Some(
                    serde_json::json!({"function":function,"healthy_field":healthy,"name_field":name,"loop_field":looping}),
                ),
            )
        } else {
            (None, None)
        };
        let selected_handler = if anim_mode {
            "engine.AnimProp.Invulnerable.BeginState"
        } else {
            "engine.Prop.Invulnerable.BeginState"
        };
        let dam_name = self.state_names[*self
            .indices
            .get("engine.prop.damagable")
            .ok_or("missing Damagable state")?]
        .ok_or("missing Damagable name")?;
        let auto_name = rc_package::name_bindings::hardcoded_name("Auto").ok_or("missing Auto")?;
        let mut probes = Vec::new();
        for (class, owner) in self.owners.iter().enumerate() {
            if owner.is_state {
                continue;
            }
            let selected =
                self.auto
                    .resolve_target(class, auto_name, self.types, self.state_names)?;
            if selected.selection.route
                == rc_package::state_selection::StateTargetRoute::ClassFallback
            {
                continue;
            }
            let node = selected.selection.state_node;
            let masks = self.masks(class, node)?;
            if masks.allowed() & (1 << 16) == 0
                || self.handler(class, node, StateEvent::BeginState)? != selected_handler
            {
                continue;
            }
            let mut cases = Vec::new();
            let scalar_inputs = [
                (i32::MIN, false),
                (0, false),
                (1, false),
                (i32::MAX, false),
                (0, true),
                (1, true),
            ];
            let animation_inputs = if anim_mode {
                vec![
                    Some(HealthyAnimationSnapshot {
                        name: rc_package::name_bindings::hardcoded_name("None")
                            .ok_or("missing None")?,
                        looping: false,
                    }),
                    Some(HealthyAnimationSnapshot {
                        name: rc_package::name_bindings::hardcoded_name("Tick")
                            .ok_or("missing Tick")?,
                        looping: false,
                    }),
                    Some(HealthyAnimationSnapshot {
                        name: rc_package::name_bindings::hardcoded_name("Tick")
                            .ok_or("missing Tick")?,
                        looping: true,
                    }),
                ]
            } else {
                vec![None]
            };
            for (health, display_state_messages, animation_input) in scalar_inputs
                .into_iter()
                .flat_map(|(h, d)| animation_inputs.iter().copied().map(move |a| (h, d, a)))
            {
                let input = PropBeginSnapshot {
                    health,
                    display_state_messages,
                };
                let mut execution = StateExecution {
                    probe: ProbeFrame::init_execution(class),
                    node: class,
                    code: None,
                    latent_action: 7,
                    object_flags: 0,
                };
                let mut events = Vec::new();
                let mut inner_switches = Vec::new();
                let mut labels = Vec::new();
                let mut animation_requests = Vec::new();
                let result = goto_state(
                    Some(&mut execution),
                    &selected,
                    masks,
                    self.state_names,
                    &mut |s, event| {
                        let handler = self.handler(class, s.probe.state_node, event)?;
                        events.push(serde_json::json!({"event":event,"function":handler}));
                        if event != StateEvent::BeginState || handler != selected_handler {
                            return Err("unresolved Prop dependency".into());
                        }
                        let mut super_begin = || {
                            if anim_mode {
                                events.push(serde_json::json!({"event":"DirectSuperBeginState","function":"engine.Prop.Invulnerable.BeginState"}));
                            }
                            match inv.action(input) {
                                PropBeginAction::Return => Ok(()),
                                PropBeginAction::BroadcastRequired(_) => {
                                    Err("unresolved Prop Broadcast".into())
                                }
                                PropBeginAction::GotoDamagable => {
                                    let target = self.auto.resolve_target(
                                        class,
                                        dam_name,
                                        self.types,
                                        self.state_names,
                                    )?;
                                    let masks = self.masks(class, target.selection.state_node)?;
                                    let inner = script_goto_state(
                                        Some(s),
                                        ScriptStateRequest {
                                            state: dam_name,
                                            label: None,
                                        },
                                        &target,
                                        masks,
                                        self.state_names,
                                        &mut |s, event| {
                                            let handler =
                                                self.handler(class, s.probe.state_node, event)?;
                                            events.push(
                                            serde_json::json!({"event":event,"function":handler}),
                                        );
                                            if event != StateEvent::BeginState
                                                || handler != "engine.Prop.Damagable.BeginState"
                                            {
                                                return Err("unresolved Prop dependency".into());
                                            }
                                            match dam.action(input) {
                                                PropBeginAction::Return => Ok(()),
                                                _ => Err("unresolved Prop Broadcast".into()),
                                            }
                                        },
                                        &mut |s, name| {
                                            let found = self.missing_label(s, name)?;
                                            labels.push(
                                                serde_json::json!({"name":name,"found":found}),
                                            );
                                            Ok(found)
                                        },
                                    )?;
                                    inner_switches.push(serde_json::json!({"selected":self.owners[target.selection.state_node].path,"target":target,"result":inner}));
                                    Ok(())
                                }
                            }
                        };
                        if let Some(anim) = anim_handler {
                            anim.run(
                                || {
                                    super_begin()?;
                                    animation_input
                                        .ok_or("missing diagnostic healthy animation".into())
                                },
                                &mut |request| {
                                    animation_requests.push(request);
                                    Err("unresolved AnimProp animation".into())
                                },
                            )
                        } else {
                            super_begin()
                        }
                    },
                );
                let outcome = match result {
                    Ok(value) => serde_json::to_value(value)?,
                    Err(error) if error == "unresolved Prop Broadcast" => {
                        serde_json::json!("UnresolvedBroadcast")
                    }
                    Err(error) if error == "unresolved Prop dependency" => {
                        serde_json::json!("UnresolvedHandler")
                    }
                    Err(error) if error == "unresolved AnimProp animation" => {
                        serde_json::json!("UnresolvedAnimation")
                    }
                    Err(error) => return Err(error.into()),
                };
                let mut case = serde_json::json!({"input":input,"outcome":outcome,"execution":execution,"events":events,"inner_switches":inner_switches,"labels":labels,"final_state":self.owners[execution.probe.state_node].path});
                if anim_mode {
                    case["animation_input"] = serde_json::to_value(animation_input)?;
                    case["animation_requests"] = serde_json::to_value(animation_requests)?;
                }
                cases.push(case);
            }
            probes.push(serde_json::json!({"class":owner.path,"auto_state":self.owners[node].path,"cases":cases}));
        }
        let mut report = serde_json::json!({"health_field":health,"display_field":display,"invulnerable_function":inv_function,"damagable_function":dam_function,"probes":probes,
            "scope":"Original freshly linked class/state lookup and masks. Only classes selecting base Prop.Invulnerable.BeginState, not AnimProp override. Six supplied health/display inputs per class. Verified branch adapters and nested base/script GotoState; label miss accepted only where original metadata proves no label tables in the whole active-state ancestry. Broadcast and other handlers stop explicitly. No actual defaults/game instances, Broadcast/animation/VM/virtual override execution or Android coverage."});
        if anim_mode {
            report["anim_metadata"] = anim_metadata.ok_or("missing verified AnimProp metadata")?;
            report["scope"]=serde_json::json!("Original AnimProp.Invulnerable.BeginState selected by fresh lookup/mask metadata. Fixed direct Prop superhandler runs first; healthy animation fields read afterwards even after redirect. Six supplied health/display inputs times None/Play/Loop each class; Tick is only a supplied name token, not a proven clip. None completes without animation. Broadcast and PlayAnim/LoopAnim stop explicitly, never treated as played. Label misses proven from metadata. No generic VM, native animation/optional parameter defaults, real actor defaults, virtual overrides or Android execution.");
        }
        Ok(report)
    }
}
