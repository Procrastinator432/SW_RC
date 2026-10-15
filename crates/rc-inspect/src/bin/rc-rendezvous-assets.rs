//! Compile only the selected original script fragments for the next rendezvous.
use rc_game::rendezvous::{Action, Assets, Effect, Operation, Program};
use rc_package::{
    properties::{self, Value},
    read_package,
};
use std::{collections::BTreeMap, env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 && !(args.len() == 4 && args[3] == "--encounter") {
        return Err("usage: rc-rendezvous-assets GAME_DATA OUTPUT".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir_all(out.join("sounds"))?;
    let data = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&data)?;
    let catalog = rc_inspect::defaults::load(root)?;
    let value = |id: usize, name| -> Result<Value, String> {
        let e = &pkg.exports[id - 1];
        let props = properties::read(&pkg, &data, e)?;
        Ok(catalog
            .instance(&pkg.object_path(e.class)?, &props, name, 0, "geo_01a.ctm")?
            .value)
    };
    let text = |id, name| -> Result<String, String> {
        match value(id, name)? {
            Value::Name(v) | Value::String(v) => Ok(v),
            Value::Object { path, .. } => Ok(path),
            _ => Err(format!("Expected text {id}.{name}")),
        }
    };
    let number = |id, name| -> Result<f64, String> {
        match value(id, name)? {
            Value::Float(v) => Ok(v as f64),
            Value::Int(v) => Ok(v as f64),
            Value::Byte(v) => Ok(v as f64),
            _ => Err(format!("Expected number {id}.{name}")),
        }
    };
    let boolean = |id, name| -> Result<bool, String> {
        match value(id, name)? {
            Value::Bool(v) => Ok(v),
            _ => Err(format!("Expected bool {id}.{name}")),
        }
    };
    let point = |id| -> Result<[f64; 3], String> {
        match value(id, "Location")? {
            Value::Vector(v) => Ok(v.map(f64::from)),
            _ => Err("Invalid location".into()),
        }
    };
    let mut waves = BTreeMap::new();
    let mut programs = vec![];
    let mut receipts = vec![];
    let mut fragments = vec![
        (195, "levelstart", 1746, 1746, 1745),
        (195, "ActivateDemoCharge", 1748, 1750, 1747),
        (325, "SpawnSmallBunker", 2092, 2103, 2087),
        (531, "Add62ToSquad", 2518, 2526, 2517),
        (847, "Conversation04", 3178, 3181, 3177),
        (1128, "DialogDemolition", 3539, 3549, 3538),
    ];
    if args.len() == 4 {
        fragments.extend([
            (511, "62Intro", 2447, 2452, 2446),
            (1120, "62Intro", 3520, 3525, 3519),
            (1134, "DialogBehindBunker", 3568, 3570, 3567),
        ]);
    }
    for (script, trigger, first, last, wait_id) in fragments {
        let props = properties::read(&pkg, &data, &pkg.exports[script - 1])?;
        let payload = pkg.payload(&data, &pkg.exports[script - 1])?;
        let a = props
            .values
            .iter()
            .find(|p| p.name == "Actions")
            .ok_or("Missing script actions")?;
        let ids =
            properties::object_array(&pkg, &payload[a.payload_offset..a.payload_offset + a.bytes])?
                .into_iter()
                .map(|a| a.0 as usize)
                .collect::<Vec<_>>();
        let expected = (first..=last).collect::<Vec<_>>();
        let at = ids
            .iter()
            .position(|id| *id == first)
            .ok_or("Missing script fragment")?;
        if ids.get(at..at + expected.len()) != Some(expected.as_slice()) {
            return Err("Original action order changed".into());
        }
        let wait_property = if wait_id == 2087 {
            "Event"
        } else {
            "ExternalEvent"
        };
        if text(wait_id, wait_property)? != trigger {
            return Err("Original event binding changed".into());
        }
        let mut actions = vec![];
        for id in first..=last {
            if boolean(id, "bDisabled")? {
                return Err("Selected action unexpectedly disabled".into());
            }
            let class = pkg.object_path(pkg.exports[id - 1].class)?;
            let op = match class.as_str() {
                "Gameplay.ACTION_WaitForTimer" => Operation::Wait {
                    seconds: number(id, "PauseTime")?,
                },
                "Gameplay.ACTION_PlaySound" => {
                    let waits = boolean(id, "bWaitForFinish")?;
                    let source = text(id, "Sound")?;
                    let (sound, seconds) = rc_inspect::mission_assets::sound(root, &source)?;
                    fs::write(
                        out.join("sounds")
                            .join(format!("{}.wav", source.rsplit('.').next().unwrap())),
                        &sound.wave,
                    )?;
                    waves.insert(
                        source.clone(),
                        serde_json::json!({"seconds":seconds,"bytes":sound.wave.len()}),
                    );
                    Operation::Sound {
                        source,
                        seconds: if waits { seconds } else { 0. },
                        owner: text(id, "SoundSourceTag")?,
                    }
                }
                _ => Operation::Emit {
                    effect: match class.as_str() {
                        "Gameplay.ACTION_TriggerEvent" => Effect::Event {
                            tag: text(id, "Event")?,
                        },
                        "CTGame.ACTION_ChangeSquadMember" => {
                            if !boolean(id, "bAddMember")? {
                                return Err("Unexpected squad removal".into());
                            }
                            Effect::AddSquadMember {
                                member: text(id, "MemberTag")?,
                                squad: text(id, "SquadTag")?,
                            }
                        }
                        "CTGame.ACTION_BlinkHUDSquad" => Effect::BlinkSquad {
                            seconds: number(id, "BlinkTime")?,
                        },
                        "CTGame.ACTION_StopFactorySpawning" => Effect::StopFactory {
                            tag: text(id, "FactoryTag")?,
                            permanent: boolean(id, "PermanentlyDisableFactory")?,
                            destroy_offspring: boolean(id, "DestroyFactoryOffspring")?,
                        },
                        "Gameplay.ACTION_HurtActor" => Effect::HurtActor {
                            tag: text(id, "TargetTag")?,
                            damage: number(id, "amount")?,
                            damage_type: text(id, "Type")?,
                        },
                        "Gameplay.ACTION_SpawnStatusEffect" => Effect::StatusEffect {
                            tag: text(id, "ActorTag")?,
                            class: text(id, "StatusEffectClass")?,
                            seconds: number(id, "MaxDuration")?,
                            damage_per_second: number(id, "DamagePerSecond")?,
                        },
                        "Gameplay.ACTION_ObjectiveUpdate" => {
                            if number(id, "UpdateAction")? != 0.
                                || number(id, "ObjectiveNum")? != 4.
                            {
                                return Err("Unexpected original objective update".into());
                            }
                            Effect::ObjectiveCompleted { objective: 4 }
                        }
                        "CTGame.ACTION_ShowActorObjectiveTracker" => Effect::Tracker {
                            tag: text(id, "ActorTag")?,
                        },
                        "Gameplay.ACTION_GotoState" => Effect::GotoState {
                            tag: text(id, "TargetTag")?,
                            state: text(id, "State")?,
                        },
                        "Gameplay.ACTION_SetHidden" => Effect::Event {
                            tag: if boolean(id, "bHidden")? {
                                "HideSpiderDroid"
                            } else {
                                "ShowSpiderDroid"
                            }
                            .into(),
                        },
                        "CTGame.ACTION_ShowMarker" => Effect::ShowMarker {
                            tag: text(id, "MarkerTag")?,
                        },
                        _ => return Err(format!("Unsupported selected action {id}:{class}").into()),
                    },
                },
            };
            receipts.push(
                serde_json::json!({"script":script,"original":id,"class":class,"operation":op}),
            );
            actions.push(Action {
                original: id as u32,
                operation: op,
            });
        }
        programs.push(Program {
            script: script as u32,
            trigger: trigger.into(),
            actions,
        });
    }
    if pkg.object_path(pkg.exports[457].class)? != "Engine.Trigger"
        || text(458, "Event")? != "Add62ToSquad"
        || number(458, "TriggerType")? != 0.
        || !boolean(458, "bTriggerOnceOnly")?
    {
        return Err("Original player rendezvous trigger changed".into());
    }
    let join = rc_game::crash::Trigger {
        original: 458,
        position: point(458)?,
        radius: number(458, "CollisionRadius")?,
        height: number(458, "CollisionHeight")?,
    };
    let assets = Assets {
        join,
        programs,
        provenance: serde_json::json!({"map":"geo_01a","objective":4,"original_title":"Rendezvous with Six-Two","fragments":[195,325,531,847,1128],"boundary":"Objective4 and enabling the demolition marker; later command/charge/warrior attack remains outside this host"}),
    };
    assets.validate()?;
    let targets=[802,957,1076,1151,1155,243,457,458,465,466,851].into_iter().map(|id|Ok(serde_json::json!({"original":id,"position":point(id)?,"class":pkg.object_path(pkg.exports[id-1].class)?,"tag":text(id,"Tag")?}))).collect::<Result<Vec<serde_json::Value>,String>>()?;
    fs::write(
        out.join("rendezvous-assets.json"),
        serde_json::to_vec_pretty(&assets)?,
    )?;
    fs::write(
        out.join("rendezvous-bindings.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"sound_bank":waves,"actions":receipts,"targets":targets,"scope":"Extracted bounded event host; not attached to the playable game or Android"}),
        )?,
    )?;
    println!(
        "Prepared {} original actions and {} PCM sounds",
        receipts.len(),
        waves.len()
    );
    Ok(())
}
