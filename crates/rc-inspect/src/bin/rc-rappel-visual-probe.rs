//! Original rope receivers and authored mesh pose previews for the selected opening.
use rc_package::{properties, read_package};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args().collect();
    if a.len() != 4 {
        return Err("usage: rc-rappel-visual-probe GAME_DATA SCENE OUT".into());
    }
    let root = Path::new(&a[1]);
    let out = Path::new(&a[3]);
    fs::create_dir_all(out)?;
    let cat = rc_inspect::defaults::load(root)?;
    let bytes = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&bytes)?;
    let mut actors = vec![];
    for field in [
        "bSetBriefingWeapon",
        "BriefingWeapon",
        "bOkToChangeWeapon",
        "bDrawHUD",
    ] {
        actors.push(serde_json::json!({"briefing_default":field,"value":cat.resolve("Gameplay.ACTION_Briefing",field,0)}));
    }
    let defaults_bytes = fs::read(root.join("Properties/animpropproperties.u"))?;
    let defaults_pkg = read_package(&defaults_bytes)?;
    let rope_class = defaults_pkg
        .exports
        .iter()
        .find(|e| e.class == 0 && e.name == "Rope")
        .ok_or("Rope class")?;
    let rope_props =
        rc_package::classes::read(&defaults_pkg, &defaults_bytes, rope_class)?.properties;
    let raw = defaults_pkg.payload(&defaults_bytes, rope_class)?;
    for field in &rope_props.values {
        if field.name == "AnimHealthy" || field.name == "AnimTriggered" {
            actors.push(serde_json::json!({"rope_class_struct":field.name,"value":properties::tagged_struct(&defaults_pkg,&raw[field.payload_offset..field.payload_offset+field.bytes])?}));
        }
    }
    for field in [
        "AnimHealthy",
        "AnimTriggered",
        "InitialState",
        "bAnimByOwner",
        "bHidden",
        "Mesh",
    ] {
        actors.push(serde_json::json!({"rope_default":field,"value":cat.resolve("AnimPropProperties.Rope",field,0)}));
    }
    for (i, e) in pkg.exports.iter().enumerate() {
        let class = pkg.object_path(e.class)?;
        let Ok(p) = properties::read(&pkg, &bytes, e) else {
            continue;
        };
        let mut nested = vec![];
        for field in p.values.iter().filter(|v| v.name == "AnimTriggered") {
            let raw = pkg.payload(&bytes, e)?;
            nested.push(properties::tagged_struct(
                &pkg,
                &raw[field.payload_offset..field.payload_offset + field.bytes],
            )?);
        }
        let tag = cat
            .instance(&class, &p, "Tag", 0, "geo_01a")
            .ok()
            .map(|v| v.value);
        let text = serde_json::to_string(&p)?;
        let mut waits_for_rappel = false;
        for field in p.values.iter().filter(|v| v.name == "Actions") {
            let raw = pkg.payload(&bytes, e)?;
            let ids = properties::object_array(
                &pkg,
                &raw[field.payload_offset..field.payload_offset + field.bytes],
            )?;
            waits_for_rappel = ids.iter().any(|(id, _)| [1257, 1766, 2493].contains(id));
        }
        if waits_for_rappel
            || i + 1 == 457
            || text.contains("Rappel")
            || text.contains("CloneRope07")
            || class.ends_with(".Rope")
        {
            let mut actions = vec![];
            for v in p.values.iter().filter(|v| v.name == "Actions") {
                let raw = pkg.payload(&bytes, e)?;
                for (id, _) in properties::object_array(
                    &pkg,
                    &raw[v.payload_offset..v.payload_offset + v.bytes],
                )? {
                    if id > 0 {
                        let e = &pkg.exports[id as usize - 1];
                        actions.push(serde_json::json!({"original":id,"class":pkg.object_path(e.class)?,"properties":properties::read(&pkg,&bytes,e)?}));
                    }
                }
            }
            actors.push(serde_json::json!({"original":i+1,"path":pkg.object_path(i as i32+1)?,"class":class,"tag":tag,"properties":p,"nested":nested,"actions":actions}));
        }
    }
    let game = rc_game::bundle::decode(&fs::read(&a[2])?)?;
    let rope = &game
        .opening
        .as_ref()
        .ok_or("Opening")?
        .rappel
        .as_ref()
        .ok_or("Rappel")?
        .rope;
    let player = &game
        .opening
        .as_ref()
        .unwrap()
        .rappel
        .as_ref()
        .unwrap()
        .player;
    let clone = fs::read(root.join("Animations/clone.ukx"))?;
    let cp = read_package(&clone)?;
    for n in &player.mesh.clips[0].sequence.notifies {
        let e = &cp.exports[n.object_index as usize - 1];
        actors.push(serde_json::json!({"player_notify":n.object_index,"time":f32::from_bits(n.time_bits),"class":cp.object_path(e.class)?,"properties":properties::read(&cp,&clone,e)?}));
    }
    let mut poses = vec![];
    for (c, clip) in rope.mesh.clips.iter().enumerate() {
        for frame in [0., 0.5, 0.99] {
            let mut s = rope.geometry(&rope.channel(c, frame)?)?;
            s.textures = rope
                .mesh
                .materials
                .iter()
                .map(|m| rc_render::Texture {
                    width: m.width,
                    height: m.height,
                    pixels: m.pixels.clone(),
                })
                .collect();
            let name = format!("{}-{frame}.ppm", clip.sequence.name.name);
            let pixels = s.render(512, 512, 0.65, 0.1, 1.)?;
            let mut image = b"P6\n512 512\n255\n".to_vec();
            for p in pixels {
                image.extend([(p >> 16) as u8, (p >> 8) as u8, p as u8]);
            }
            fs::write(out.join(&name), image)?;
            poses.push(serde_json::json!({"clip":clip.sequence.name.name,"frame":frame,"triangles":s.triangles.len(),"center":s.center,"radius":s.radius,"image":name}));
        }
    }
    fs::write(
        out.join("original-receivers.json"),
        serde_json::to_vec_pretty(&actors)?,
    )?;
    fs::write(out.join("poses.json"), serde_json::to_vec_pretty(&poses)?)?;
    println!(
        "{} original receiver/default/notify records; {} pose previews",
        actors.len(),
        poses.len()
    );
    Ok(())
}
