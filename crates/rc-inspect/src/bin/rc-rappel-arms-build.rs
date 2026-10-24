//! Explicit arm-only port visual adapter with original Delta-38 material.
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args().collect();
    if a.len() != 4 {
        return Err("usage: rc-rappel-arms-build GAME_DATA INPUT OUTPUT".into());
    }
    let root = Path::new(&a[1]);
    let b = fs::read(&a[2])?;
    if !b.starts_with(b"RCPLAY13") {
        return Err("Expected opening wrapper".into());
    }
    let n = u32::from_le_bytes(b[8..12].try_into()?) as usize;
    let mut opening: rc_game::opening::Assets = serde_json::from_slice(&b[12..12 + n])?;
    let rappel = opening.rappel.as_mut().ok_or("Missing rappel")?;
    let player = rappel.player.as_mut().ok_or("Missing player mesh")?;
    let skin = "CloneTextures.CloneTextures.CloneCommando38_Shader";
    if player.materials.len() != 1
        || !["CloneTextures.CloneTextures.CloneCommandoProxy", skin]
            .contains(&player.materials[0].source.as_str())
    {
        return Err("Unexpected player material binding".into());
    }
    let mut assets = rc_inspect::assets::Assets::new(&[
        &root.join("Textures"),
        &root.join("System"),
        &root.join("Properties"),
    ])?;
    let d = assets.diffuse_at_time(skin, 256, 0.)?;
    let opacity = assets.opacity_evidence(skin)?;
    if assets.opaque(skin) != Ok(true) {
        return Err("Expected proven opaque player armor".into());
    }
    let mut pixels = d.texture.pixels;
    for p in &mut pixels {
        *p |= 0xff000000;
    }
    player.materials[0] = rc_game::visual::Material {
        source: skin.into(),
        chain: d.chain,
        opacity,
        width: d.texture.width,
        height: d.texture.height,
        pixels,
        uv_scale: d.scale,
        uv_transform: d.uv_transform,
    };
    let mask = rc_game::rappel_arms::triangle_mask(player)?;
    let kept = mask.iter().filter(|v| **v).count();
    rappel.show_arms = true;
    opening.provenance["rappel_arm_adapter"] = serde_json::json!({"policy":"Arm-only visibility from original skin influence bones, pose at current collision body with root translation locked; original Pawn MeshToWorld height adjustment.","skin":skin,"skin_selection":"Explicit Delta-38 port appearance; original own-body skin assignment not established.","kept_triangles":kept,"total_triangles":mask.len(),"original_first_person_body_binding_proven":false,"camera":"unchanged"});
    let meta = serde_json::to_vec(&opening)?;
    let result = [
        b"RCPLAY13".as_slice(),
        &(meta.len() as u32).to_le_bytes(),
        &meta,
        &b[12 + n..],
    ]
    .concat();
    rc_game::bundle::decode(&result)?;
    fs::write(&a[3], result)?;
    println!(
        "Arm adapter: {kept}/{} triangles, original Delta-38 material",
        mask.len()
    );
    Ok(())
}
