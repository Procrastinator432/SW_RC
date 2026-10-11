//! Freeze installed single-player difficulty config and map neutral-volume flags.
use rc_game::{bundle, combat, game_rules, melee};
use rc_inspect::defaults;
use rc_package::{
    properties::{self, Value},
    read_package,
};
use std::{collections::BTreeMap, env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-play-game-rules GAME_DATA LUNGE_SCENE OUTPUT_DIRECTORY".into());
    }
    let data = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let bytes = fs::read(&args[2])?;
    if !bytes.starts_with(b"RCPLAY03") || bytes.len() < 12 {
        return Err("Need combat scene".into());
    }
    let size = u32::from_le_bytes(bytes[8..12].try_into()?) as usize;
    let mut assets: combat::Assets =
        serde_json::from_slice(bytes.get(12..12 + size).ok_or("Truncated combat")?)?;
    if assets.game_rules.is_some() || assets.lunge.is_none() {
        return Err("Need unextended lunge scene".into());
    }
    let game = bundle::decode(&bytes)?;
    let catalog = defaults::load(data)?;
    let mut player_defaults = BTreeMap::new();
    for field in [
        "Health",
        "Shields",
        "ShieldRechargeDelay",
        "ShieldRechargeRate",
        "ArmorFactor",
        "MyDamageVariance",
        "ShieldDamageVariance",
    ] {
        let player = catalog.resolve("CTCharacters.PlayerCommando", field, 0)?;
        if field.ends_with("DamageVariance") {
            let Value::Object { ref path, .. } = player.value else {
                return Err("Invalid variance reference".into());
            };
            if field == "ShieldDamageVariance" {
                if !path.eq_ignore_ascii_case("None") {
                    return Err("Separate player shield variance unsupported".into());
                }
            } else {
                let melee = assets.melee.as_mut().ok_or("Missing prepared damage")?;
                let classes = melee.provenance["damage_report"]["raw_classes"]
                    .as_object()
                    .ok_or("Missing damage catalog")?;
                let table: BTreeMap<_, _> = classes
                    .iter()
                    .map(|(key, value)| (key.to_lowercase(), value))
                    .collect();
                let class = table
                    .get(&path.to_lowercase())
                    .ok_or("Missing player damage variance class")?;
                let ids = melee.provenance["class_ids"]
                    .as_object()
                    .ok_or("Missing damage class ids")?;
                let id = |name: &str| -> Result<u32, String> {
                    ids.get(&name.to_lowercase())
                        .and_then(|v| v.as_u64())
                        .and_then(|v| v.try_into().ok())
                        .ok_or_else(|| format!("Missing damage class id {name}"))
                };
                let mut include = vec![];
                for row in class["decoded_arrays"]["IncludeTypes"]
                    .as_array()
                    .ok_or("Missing player include array")?
                {
                    let fields: BTreeMap<_, _> = row["values"]
                        .as_array()
                        .ok_or("Missing variance row")?
                        .iter()
                        .map(|p| (p["name"].as_str().unwrap_or(""), &p["value"]["value"]))
                        .collect();
                    let object = fields
                        .get("DamageTypeClass")
                        .ok_or("Missing variance damage class")?;
                    include.push(melee::DamageRule {
                        class: if object["index"].as_i64() == Some(0) {
                            None
                        } else {
                            Some(id(object["path"]
                                .as_str()
                                .ok_or("Missing damage class path")?)?)
                        },
                        multiplier: fields
                            .get("Multiplier")
                            .and_then(|v| v.as_f64())
                            .ok_or("Missing damage multiplier")?
                            as f32,
                    });
                }
                // Selected player class and all ancestors have no exclusion
                // array; reject a different original instead of guessing.
                let mut parent = Some(path.to_lowercase());
                let mut visited = std::collections::BTreeSet::new();
                while let Some(key) = parent {
                    if !visited.insert(key.clone()) {
                        return Err("Damage variance ancestry cycle".into());
                    }
                    let Some(class) = table.get(&key) else {
                        if key != "core.object" {
                            return Err("Incomplete variance ancestry".into());
                        }
                        break;
                    };
                    if class["decoded_arrays"]["ExcludeTypes"].is_array() {
                        return Err("Player exclusion arrays need explicit resolution".into());
                    }
                    parent = Some(
                        class["parent"]
                            .as_str()
                            .ok_or("Missing variance parent")?
                            .to_lowercase(),
                    );
                }
                melee.player.include = include;
                melee.player.exclude.clear();
            }
        } else {
            let value = match player.value {
                Value::Float(value) => value,
                Value::Int(value) => value as f32,
                _ => return Err("Invalid PlayerCommando defense scalar".into()),
            };
            let melee = assets.melee.as_mut().ok_or("Missing prepared damage")?;
            match field {
                "Health" => melee.player_health = value,
                "Shields" => melee.player_shields = value,
                "ShieldRechargeDelay" => melee.shield_delay = value,
                "ShieldRechargeRate" => melee.shield_rate = value,
                "ArmorFactor" => melee.player.armor = value,
                _ => unreachable!(),
            }
        }
        player_defaults.insert(field, player);
    }
    let god_mode = catalog.resolve("CTGame.CTPlayer", "bGodMode", 0)?;
    if !matches!(god_mode.value, Value::Bool(false)) || god_mode.external_config_or_localization {
        return Err("Unsupported default god mode".into());
    }
    let config = fs::read_to_string(data.join("System/user.ini"))?;
    let mut section = false;
    let mut values = BTreeMap::new();
    for (line, text) in config.lines().enumerate() {
        let text = text.trim();
        if text.starts_with('[') {
            section = text.eq_ignore_ascii_case("[Engine.PlayerController]");
        }
        if !section || text.starts_with(';') {
            continue;
        }
        if let Some((key, value)) = text.split_once('=') {
            if [
                "DamageModifierEasy",
                "DamageModifierMedium",
                "DamageModifierHard",
                "DifficultyDamageModifier",
            ]
            .contains(&key)
                && values
                    .insert(key.to_string(), (value.parse::<f32>()?, line + 1))
                    .is_some()
            {
                return Err("Duplicate difficulty config key".into());
            }
        }
    }
    let get = |name: &str| {
        values
            .get(name)
            .map(|v| v.0)
            .ok_or("Missing difficulty config key")
    };
    let modifiers = [
        get("DamageModifierEasy")?,
        get("DamageModifierMedium")?,
        get("DamageModifierHard")?,
    ];
    let configured = get("DifficultyDamageModifier")?;
    let selected = modifiers.iter().position(|&m| m == configured).unwrap_or(1);
    let mut engine_section = false;
    let mut default_game = None;
    for (line, text) in fs::read_to_string(data.join("System/system.ini"))?
        .lines()
        .enumerate()
    {
        let text = text.trim();
        if text.starts_with('[') {
            engine_section = text.eq_ignore_ascii_case("[Engine.Engine]");
        }
        if engine_section && !text.starts_with(';') {
            if let Some((key, value)) = text.split_once('=') {
                if key.eq_ignore_ascii_case("DefaultGame")
                    && default_game
                        .replace((value.to_string(), line + 1))
                        .is_some()
                {
                    return Err("Duplicate DefaultGame".into());
                }
            }
        }
    }
    if !default_game
        .as_ref()
        .is_some_and(|(value, _)| value.eq_ignore_ascii_case("CTGame.CTGameInfoSP"))
    {
        return Err("Unsupported installed single-player game class".into());
    }
    let configured_fields = values
        .keys()
        .map(|key| {
            let resolved = catalog.resolve("CTGame.CTPlayer", key, 0)?;
            if !resolved.external_config_or_localization {
                return Err("Difficulty field is not external config".to_string());
            }
            Ok((key.clone(), resolved))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let mut neutral_volumes = BTreeMap::new();
    let default = catalog.resolve("Engine.DefaultPhysicsVolume", "bNeutralZone", 0)?;
    let Value::Bool(default_neutral) = default.value else {
        return Err("Invalid default neutral flag".into());
    };
    neutral_volumes.insert("DefaultPhysicsVolume".into(), default_neutral);
    let map = fs::read(data.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&map)?;
    let mut evidence =
        vec![serde_json::json!({"object":"DefaultPhysicsVolume","resolved":default})];
    let mut level_game = None;
    let mut map_rules = vec![];
    for (i, export) in pkg.exports.iter().enumerate() {
        if export.class == 0 {
            continue;
        }
        let class = pkg.object_path(export.class)?;
        if !catalog.names().any(|n| n.eq_ignore_ascii_case(&class)) {
            continue;
        }
        if catalog.derives_from(&class, "Engine.GameRules")? {
            map_rules.push(pkg.object_path(i as i32 + 1)?);
        }
        if class == "Engine.LevelInfo" {
            let props = properties::read(&pkg, &map, export)?;
            level_game = Some(catalog.instance(&class, &props, "DefaultGameType", 0, "geo_01a")?);
        }
        let name = pkg.object_path(i as i32 + 1)?;
        if !game.volumes.volumes.iter().any(|v| v.name == name) {
            continue;
        }
        if !catalog.derives_from(&class, "Engine.PhysicsVolume")? {
            return Err("Volume is not PhysicsVolume".into());
        }
        let props = properties::read(&pkg, &map, export)?;
        let flag = catalog.instance(&class, &props, "bNeutralZone", 0, "geo_01a")?;
        let Value::Bool(neutral) = flag.value else {
            return Err("Invalid neutral flag".into());
        };
        neutral_volumes.insert(name.clone(), neutral);
        evidence.push(serde_json::json!({"object":name,"class":class,"resolved":flag}));
    }
    if neutral_volumes.len() != game.volumes.volumes.len() + 1 {
        return Err("Unresolved playable volume".into());
    }
    if !map_rules.is_empty() {
        return Err("Map has unsupported GameRules actors".into());
    }
    let game_info = [
        "DefaultPlayerClassName",
        "PlayerControllerClassName",
        "GameRulesModifiers",
    ]
    .into_iter()
    .map(|name| Ok((name, catalog.resolve("CTGame.CTGameInfoSP", name, 0)?)))
    .collect::<Result<BTreeMap<_, _>, String>>()?;
    if !matches!(&game_info["DefaultPlayerClassName"].value, Value::String(v) if v == "CTCharacters.PlayerCommando")
        || !matches!(&game_info["PlayerControllerClassName"].value, Value::String(v) if v == "CTGame.CTPlayer")
        || !matches!(
            &game_info["GameRulesModifiers"].value,
            Value::Object { index: 0, .. }
        )
        || !catalog.derives_from("CTGame.CTPlayer", "Engine.PlayerController")?
    {
        return Err("Unsupported selected single-player damage classes".into());
    }
    if !level_game.as_ref().is_some_and(|g| matches!(&g.value, Value::String(v) if v.is_empty() || v.eq_ignore_ascii_case("CTGame.CTGameInfoSP"))) {
        return Err("Map selects unsupported game class".into());
    }
    let provenance = serde_json::json!({
        "config":"System/user.ini:[Engine.PlayerController]", "config_values_and_lines":values,
        "configured_fields":configured_fields,"engine_default_game_and_line":default_game,
        "player_defense_defaults":player_defaults,"god_mode_default":god_mode,
        "level_game_type":level_game,"game_info_defaults":game_info,"map_game_rules":map_rules,
        "neutral_volume_evidence":evidence,
        "source_path":"PlayerCommando.CalculateDamageFrom -> Pawn.TakeDamage -> GameInfo.ReduceDamage -> GameRules.NetDamage",
        "boundaries":"selected hostile Geonosian hits on controlled PlayerCommando; no arbitrary URL GameRules option, dynamically spawned subclass, multiplayer scaling, ally hit, god-mode toggle or full script VM"
    });
    for (index, name) in ["easy", "normal", "hard"].into_iter().enumerate() {
        assets.game_rules = Some(game_rules::Assets {
            provenance: provenance.clone(),
            modifiers,
            selected: index,
            neutral_volumes: neutral_volumes.clone(),
        });
        let scene =
            bundle::encode_combat(bytes.get(12 + size..).ok_or("Missing scene base")?, &assets)?;
        bundle::decode(&scene)?;
        fs::write(out.join(format!("geo_01a-{name}.rcplay")), scene)?;
    }
    fs::write(
        out.join("game-rules-receipt.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"provenance":provenance,"modifiers":modifiers,"installed_selected":selected,"scene_choices":["easy","normal","hard"]}),
        )?,
    )?;
    println!("Three single-player difficulty scenes: {modifiers:?}, installed level {selected}");
    Ok(())
}
