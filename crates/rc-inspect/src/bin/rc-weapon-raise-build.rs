//! Bind selected original Briefing.EndState -> DC15 Load clip and its Foley.
use rc_package::properties::Value;
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args().collect();
    if a.len() != 5 {
        return Err("usage: rc-weapon-raise-build GAME_DATA INPUT OUTPUT AUDIT.json".into());
    }
    let root = Path::new(&a[1]);
    let cat = rc_inspect::defaults::load(root)?;
    if !matches!(cat.resolve("CTInventory.DC15s","LoadAnim",0)?.value,Value::Name(ref v)if v=="Load")
        || !matches!(
            cat.resolve("CTInventory.DC15s", "WeaponType", 0)?.value,
            Value::Byte(2)
        )
    {
        return Err("Unsupported original DC15 selection branch".into());
    }
    let bytes = fs::read(&a[2])?;
    let game = rc_game::bundle::decode(&bytes)?;
    if game.opening.is_none() {
        return Err("Selected opening required".into());
    }
    let weapon = &game.visuals.as_ref().ok_or("Weapon mesh")?.weapon.mesh;
    let mut updated: rc_game::visual::Mesh = serde_json::from_value(serde_json::to_value(weapon)?)?;
    if !updated.clips.iter().any(|c| c.sequence.name.name == "Load") {
        let mut assets =
            rc_inspect::assets::Assets::new(&[&root.join("Textures"), &root.join("System")])?;
        let extra = rc_inspect::visual_assets::load_from_animation(
            root,
            "hudarms.ukx",
            "HudArmsDC15s",
            &["Load"],
            &mut assets,
            Some("hudarms.HudArmsDC15sSet"),
        )?;
        if extra.bones.len() != weapon.bones.len()
            || extra
                .bones
                .iter()
                .zip(&weapon.bones)
                .any(|(a, b)| a.name.name != b.name.name)
        {
            return Err("Load clip skeleton differs".into());
        }
        updated.clips.extend(extra.clips);
    }
    let (foley, audit) = rc_inspect::weapon_notifies::load(root, &updated)?;
    let previous = game
        .dc15
        .as_ref()
        .and_then(|a| a.animation.as_ref())
        .and_then(|a| a.foley.as_ref())
        .ok_or("Existing Foley required")?;
    if foley.sounds.len() < previous.sounds.len()
        || foley
            .sounds
            .iter()
            .zip(&previous.sounds)
            .any(|(a, b)| a.source != b.source || a.wave != b.wave)
    {
        return Err("Existing weapon sound identities changed".into());
    }
    let mut layers = vec![];
    let mut at = 0;
    loop {
        let head = bytes.get(at..at + 12).ok_or("Short scene")?;
        if &head[..6] != b"RCPLAY" {
            return Err("Unexpected wrapper".into());
        }
        let n = u32::from_le_bytes(head[8..12].try_into()?) as usize;
        let mut json: serde_json::Value = serde_json::from_slice(&bytes[at + 12..at + 12 + n])?;
        if &head[..8] == b"RCPLAY02" {
            json["weapon"] = serde_json::to_value(&updated)?;
        }
        if &head[..8] == b"RCPLAY01" {
            let profile = &mut json["dc15"]["animation"];
            profile["raise_after_briefing"] = true.into();
            profile["foley"] = serde_json::to_value(&foley)?;
            json["provenance"]["briefing_weapon_raise"] = serde_json::json!({"original":"PlayerController.Briefing.EndState -> Weapon.BringUp -> Active.BeginState -> PlaySelect(LoadAnim)","load_anim":cat.resolve("CTInventory.DC15s","LoadAnim",0)?,"weapon_type":cat.resolve("CTInventory.DC15s","WeaponType",0)?,"foley":audit,"boundary":"Selected visual transition; existing gameplay fire/reload/throw requests can interrupt. Not full native bWeaponUp/queued-input gate."});
        }
        let meta = serde_json::to_vec(&json)?;
        layers.extend(&head[..8]);
        layers.extend((meta.len() as u32).to_le_bytes());
        layers.extend(meta);
        at += 12 + n;
        if &head[..8] == b"RCPLAY01" {
            layers.extend(&bytes[at..]);
            break;
        }
    }
    rc_game::bundle::decode(&layers)?;
    fs::write(&a[3], layers)?;
    fs::write(&a[4], serde_json::to_vec_pretty(&audit)?)?;
    println!("Original Load clip and Foley bound");
    Ok(())
}
