//! Compile selected objective5 transitions/dialogue from decoded original receipts.
use rc_game::demolition::{Action, Assets, Effect, Operation, Program};
use serde_json::Value;
use std::{env, fs, path::Path};
fn property(row: &Value, name: &str) -> Result<Value, String> {
    row["resolved"][name]
        .get("value")
        .and_then(|v| v.get("value"))
        .cloned()
        .ok_or_else(|| format!("Missing original {name}"))
}
fn structure(row: &Value, name: &str, field: &str) -> Result<Value, String> {
    row["nested"]
        .as_array()
        .ok_or("Missing structures")?
        .iter()
        .find(|v| v["name"] == name)
        .and_then(|v| v.get("struct"))
        .and_then(|v| v.get("Ok"))
        .and_then(|v| v["values"].as_array())
        .and_then(|v| v.iter().find(|v| v["name"] == field))
        .map(|v| v["value"]["value"].clone())
        .ok_or_else(|| format!("Missing {name}.{field}"))
}
fn string(v: Value) -> Result<String, String> {
    if let Some(s) = v.as_str() {
        return Ok(s.into());
    }
    v["path"]
        .as_str()
        .map(str::to_owned)
        .ok_or("Missing original text".into())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-demolition-assets GAME_DATA OUTPUT".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir_all(out.join("sounds"))?;
    let original: Value = serde_json::from_slice(&fs::read(out.join("original-bindings.json"))?)?;
    let rows = original["actors_and_actions"]
        .as_array()
        .ok_or("Missing original actions")?;
    let at = |id| {
        rows.iter()
            .find(|r| r["original"] == id)
            .ok_or("Missing original actor/action")
    };
    for (script, first, last) in [(195, 1745, 1760), (536, 2535, 2557)] {
        let actual = at(script)?["actions"]
            .as_array()
            .ok_or("Missing original action array")?
            .iter()
            .map(|v| v[0].as_u64().unwrap() as u32)
            .collect::<Vec<_>>();
        assert_eq!(actual, (first..=last).collect::<Vec<_>>());
    }
    for id in [2543, 2548, 2553] {
        let row = at(id)?;
        assert_eq!(row["class"], "Gameplay.ACTION_GotoLabel");
        assert!(row["properties"]["values"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["name"] == "LabelName" && p["value"]["value"] == "EndOfScript"));
        assert_eq!(at(id + 1)?["class"], "Gameplay.ACTION_EndSection");
    }
    assert!(at(2556)?["properties"]["values"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["name"] == "LabelName" && p["value"]["value"] == "EndOfScript"));
    let charge = at(465)?;
    let marker = at(466)?;
    assert_eq!(property(marker, "Anchor")?["index"], 465);
    assert_eq!(property(charge, "PropToDestroy")?["index"], 851);
    for field in ["bExplodeOnTimer", "bTriggeredByDamage", "bIsUsableByPlayer"] {
        assert_eq!(property(charge, field)?, false);
    }
    assert_eq!(property(charge, "bNonPlayerUsesTimer")?, true);
    assert_eq!(property(charge, "bSavesProgress")?, true);
    assert_eq!(property(marker, "PlaceEvent")?, "MDplaceCharge");
    assert_eq!(property(marker, "EventUnderway")?, "SpawnDemoDroids");
    assert_eq!(property(charge, "EventUnderway")?, "DemoUnderway");
    assert_eq!(
        structure(charge, "ChargeArmed", "Event")?,
        "DemoChargeArmed"
    );
    assert_eq!(
        structure(charge, "ChargeExplode", "Event")?,
        "BunkerDoorDestroyed"
    );
    let class = original["defaults"]
        .as_array()
        .ok_or("Missing defaults")?
        .iter()
        .find(|c| c["class"] == "DemolitionCharge")
        .ok_or("Missing charge defaults")?;
    let clips = original["clips"].as_array().ok_or("Missing charge clips")?;
    let start = clips
        .iter()
        .find(|c| c["name"] == "DemolitionSetChargeStart")
        .ok_or("Missing start clip")?;
    let stop = clips
        .iter()
        .find(|c| c["name"] == "DemolitionSetChargeStop")
        .ok_or("Missing stop clip")?;
    let notify = start["notifies"]
        .as_array()
        .ok_or("Missing notifies")?
        .iter()
        .find(|n| n["class"] == "Engine.AnimNotify_Trigger")
        .ok_or("Missing placement notify")?;
    assert!(notify["properties"]["values"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["name"] == "EventName" && p["value"]["value"] == "MDplaceCharge"));
    let duration = |c: &Value| -> Result<f64, String> {
        Ok(c["frames"].as_f64().ok_or("Missing frames")?
            / c["rate"].as_f64().ok_or("Missing rate")?)
    };
    let navigation = charge["nested"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["name"] == "NavPts")
        .and_then(|n| n["objects"]["Ok"][0][0].as_u64())
        .ok_or("Missing original navigation release")? as u32;
    let mut bank = std::collections::BTreeMap::new();
    let mut sound = |source: String| -> Result<f64, Box<dyn std::error::Error>> {
        let (sound, seconds) = rc_inspect::mission_assets::sound(root, &source)?;
        fs::write(
            out.join("sounds")
                .join(format!("{}.wav", source.rsplit('.').next().unwrap())),
            &sound.wave,
        )?;
        bank.insert(
            source,
            serde_json::json!({"seconds":seconds,"bytes":sound.wave.len()}),
        );
        Ok(seconds)
    };
    let placed_sound = string(structure(class, "ChargePlaced", "TransitionSound")?)?;
    let armed_sound = string(structure(charge, "ChargeArmed", "TransitionSound")?)?;
    let explosion_sound = string(structure(charge, "ChargeExplode", "TransitionSound")?)?;
    for s in [&placed_sound, &armed_sound, &explosion_sound] {
        sound(s.clone())?;
    }
    let mut programs = vec![];
    for (trigger, ids) in [
        (
            "DemoUnderway",
            vec![
                2536, 2537, 2538, 2539, 2540, 2541, 2542, 2545, 2546, 2547, 2550, 2551, 2552, 2555,
                2557,
            ],
        ),
        ("BunkerDoorDestroyed", (1752..=1760).collect()),
    ] {
        let mut actions = vec![];
        for id in ids {
            let row = at(id)?;
            let p = |name| property(row, name);
            let operation = match row["class"].as_str().ok_or("Missing action class")? {
                "Gameplay.ACTION_PlaySound" => {
                    let source = string(p("Sound")?)?;
                    let seconds = sound(source.clone())?;
                    Operation::Sound {
                        source,
                        seconds: if p("bWaitForFinish")? == true {
                            seconds
                        } else {
                            0.
                        },
                    }
                }
                "Gameplay.ACTION_WaitForTimer" => Operation::Wait {
                    seconds: p("PauseTime")?.as_f64().ok_or("Missing timer")?,
                },
                "Gameplay.ACTION_WaitForEvent" => {
                    assert_eq!(p("ExternalEvent")?, "DemoChargeArmed");
                    Operation::WaitArmed
                }
                "Gameplay.ACTION_IfCondition" => {
                    assert!(row["properties"]["values"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|p| p["name"] == "TriggeredConditionTag"
                            && p["value"]["value"] == "BunkerDoorDestroyed"));
                    Operation::IfExplodedEnd
                }
                class => Operation::Emit {
                    effect: match class {
                        "Gameplay.ACTION_EnableAIVoice" => Effect::Voice {
                            enabled: p("bEnable")?.as_bool().ok_or("Missing voice flag")?,
                        },
                        "Gameplay.ACTION_TriggerEvent" => Effect::Event {
                            tag: string(p("Event")?)?,
                        },
                        "CTGame.ACTION_ShowActorObjectiveTracker" => Effect::Tracker {
                            tag: string(p("ActorTag")?)?,
                        },
                        "Gameplay.ACTION_ObjectiveUpdate" => {
                            assert_eq!(p("ObjectiveNum")?, 5);
                            assert_eq!(p("UpdateAction")?, 0);
                            Effect::ObjectiveCompleted { objective: 5 }
                        }
                        "CTGame.ACTION_StopFactorySpawning" => Effect::StopFactory {
                            tag: string(p("FactoryTag")?)?,
                            permanent: p("PermanentlyDisableFactory")?
                                .as_bool()
                                .ok_or("Missing stop flag")?,
                            destroy_offspring: p("DestroyFactoryOffspring")?
                                .as_bool()
                                .ok_or("Missing offspring flag")?,
                        },
                        _ => return Err(format!("Unsupported objective5 action {class}").into()),
                    },
                },
            };
            actions.push(Action {
                original: id as u32,
                operation,
            });
        }
        programs.push(Program {
            trigger: trigger.into(),
            actions,
        });
    }
    let a = Assets {
        marker: 466,
        charge: 465,
        prop: 851,
        navigation,
        setup_seconds: structure(class, "ChargePlaced", "Duration")?
            .as_f64()
            .ok_or("Missing setup duration")?,
        start_seconds: duration(start)?,
        place_fraction: notify["time"].as_f64().ok_or("Missing placement time")?,
        stop_seconds: duration(stop)?,
        player_range: property(charge, "PlayerRange")?
            .as_f64()
            .ok_or("Missing range")?,
        damage: structure(charge, "ChargeExplode", "Damage")?
            .as_f64()
            .ok_or("Missing damage")?,
        damage_radius: structure(charge, "ChargeExplode", "DamageRadius")?
            .as_f64()
            .ok_or("Missing radius")?,
        placed_sound,
        armed_sound,
        explosion_sound,
        programs,
    };
    a.validate()?;
    fs::write(
        out.join("demolition-assets.json"),
        serde_json::to_vec_pretty(&a)?,
    )?;
    fs::write(
        out.join("sound-bindings.json"),
        serde_json::to_vec_pretty(&bank)?,
    )?;
    println!(
        "Prepared objective5: {} actions, {} original PCM sounds, {}s setup, navigation {}",
        a.programs.iter().map(|p| p.actions.len()).sum::<usize>(),
        bank.len(),
        a.setup_seconds,
        a.navigation
    );
    Ok(())
}
