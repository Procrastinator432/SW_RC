//! Bind selected original bot BurstCount and native melee-goal constructor to scenes.
use rc_game::{bot, bundle, combat};
use rc_inspect::defaults;
use rc_package::{classes, properties::Value, read_package};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err(
            "usage: rc-play-bot-goals GAME_DATA DIFFICULTY_SCENE_DIRECTORY OUTPUT_DIRECTORY".into(),
        );
    }
    let data = Path::new(&args[1]);
    let input = Path::new(&args[2]);
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let catalog = defaults::load(data)?;
    let controller = catalog.resolve("CTCharacters.GeonosianWarrior", "ControllerClass", 0)?;
    let Value::Object { ref path, .. } = controller.value else {
        return Err("Missing Geonosian controller".into());
    };
    if !path.eq_ignore_ascii_case("CTCharacters.GeonosianWarriorBot") {
        return Err("Unsupported Geonosian controller".into());
    }
    let count = catalog.resolve(path, "BurstCount", 0)?;
    if count.external_config_or_localization {
        return Err("Bot BurstCount requires external configuration".into());
    }
    let Value::Int(burst_count) = count.value else {
        return Err("Invalid bot BurstCount".into());
    };
    let bytes = fs::read(data.join("System/ctgame.u"))?;
    let package = read_package(&bytes)?;
    let class = package
        .exports
        .iter()
        .find(|e| e.class == 0 && e.name == "CTBot")
        .ok_or("Missing CTBot class")?;
    let fields = classes::fields(&package, &bytes, class)?;
    let shot = fields
        .iter()
        .find(|f| f.name == "ShotCounter")
        .ok_or("Missing ShotCounter field")?;
    let burst = fields
        .iter()
        .find(|f| f.name == "BurstCount")
        .ok_or("Missing BurstCount field")?;
    if shot.kind != "Core.IntProperty"
        || burst.kind != "Core.IntProperty"
        || shot.dimension != 1
        || burst.dimension != 1
    {
        return Err("Bot count fields no longer scalar integers".into());
    }
    let provenance = serde_json::json!({"controller_class":controller,"burst_count":count,
        "field_definitions":[shot,burst],
        "native_mapping":"SetShotCounter 1000aac0 writes CTBot+408; CTBot.uc declares scalar int BurstCount immediately after ShotCounter; warrior UpdateSelf 1003a2ba reads +40c, passes it to GOAL_MeleeAttack and multiplies by 2.0 at 1004d91c",
        "clock":"prepared Level.TimeSeconds snapshot converted to f32; constructor/OnActivate/PreUpdate clock arithmetic; portable geometry/animation/reachability inputs",
        "boundaries":"bounded existing walking/goal scheduler; no general GoalQueue, defensive jump, after-melee random consumption, original animation selector or native RSQRT geometry parity"});
    for level in ["easy", "normal", "hard"] {
        let bytes = fs::read(input.join(format!("geo_01a-{level}.rcplay")))?;
        if !bytes.starts_with(b"RCPLAY03") || bytes.len() < 12 {
            return Err("Need combat scene".into());
        }
        let size = u32::from_le_bytes(bytes[8..12].try_into()?) as usize;
        let mut assets: combat::Assets =
            serde_json::from_slice(bytes.get(12..12 + size).ok_or("Truncated combat")?)?;
        if assets.bot.is_some() || assets.game_rules.is_none() || assets.lunge.is_none() {
            return Err("Need unextended difficulty scene".into());
        }
        assets.bot = Some(bot::Assets {
            provenance: provenance.clone(),
            burst_count,
        });
        let scene =
            bundle::encode_combat(bytes.get(12 + size..).ok_or("Missing scene base")?, &assets)?;
        bundle::decode(&scene)?;
        fs::write(out.join(format!("geo_01a-{level}.rcplay")), scene)?;
    }
    fs::write(
        out.join("bot-goals-receipt.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"provenance":provenance,"burst_count":burst_count,"max_attack_time":rc_package::melee_goal::attack_time(burst_count)}),
        )?,
    )?;
    println!(
        "Original Geonosian bot BurstCount={burst_count}; native melee budget {} seconds",
        rc_package::melee_goal::attack_time(burst_count)
    );
    Ok(())
}
