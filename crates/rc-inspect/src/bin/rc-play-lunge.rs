//! Enable selected original lunge root curves over the navigation scene.
use rc_game::{bundle, combat, lunge};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-play-lunge NAVIGATION_SCENE OUTPUT_DIRECTORY".into());
    }
    let bytes = fs::read(&args[1])?;
    if !bytes.starts_with(b"RCPLAY03") || bytes.len() < 12 {
        return Err("Need combat layer".into());
    }
    let size = u32::from_le_bytes(bytes[8..12].try_into()?) as usize;
    let mut assets: combat::Assets =
        serde_json::from_slice(bytes.get(12..12 + size).ok_or("Truncated combat")?)?;
    if assets.navigation.is_none() || assets.lunge.is_some() {
        return Err("Need unextended navigation scene".into());
    }
    let game = bundle::decode(&bytes)?;
    let mesh = &game
        .visuals
        .as_ref()
        .ok_or("Missing original animation")?
        .enemy;
    let mut curves = vec![];
    for name in ["Lunge", "Lunge2"] {
        let index = mesh.clip(name)?;
        let sequence = &mesh.mesh.clips[index].sequence;
        let last = 1. - 1. / sequence.frames as f32;
        let samples: Vec<_> = (0..=120)
            .map(|i| lunge::offset(mesh, index, last * i as f32 / 120., 0.))
            .collect::<Result<_, _>>()?;
        curves.push(serde_json::json!({"clip":name,"frames":sequence.frames,"rate":f32::from_bits(sequence.rate_bits),"root_track":mesh.mesh.clips[index].mapping[0],"notify":assets.melee.as_ref().unwrap().clips.iter().find(|c|c.name==name).unwrap().hit_time,"samples":samples}));
    }
    assets.lunge = Some(lunge::Assets {
        seed: 8192,
        provenance: serde_json::json!({"source":"GeonosianWarriorSet compressed Move track; existing original GetRotPos decoding and clip tick","native_boundaries":"StateGeonosianWarriorAttack lunge capability/choice; root-translation endpoint transformed by pawn rotation; physRootMotion applies animation transform with moveSmooth", "adapters":"full sampled trajectory collision/floor gate, prepared mesh scale, frozen heading, translation-only actor motion, two alternating clips, seeded encounter MSVCRT-style RNG, vertical settle on interruption, no complete goal queue/native PHYS_RootMotion"}),
    });
    let result =
        bundle::encode_combat(bytes.get(12 + size..).ok_or("Missing base layer")?, &assets)?;
    bundle::decode(&result)?;
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    fs::write(
        out.join("lunge-receipt.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"profile":assets.lunge,"curves":curves}))?,
    )?;
    fs::write(out.join("geo_01a-lunge.rcplay"), result)?;
    println!("Original Lunge/Lunge2 root translation enabled");
    Ok(())
}
