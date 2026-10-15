//! Extract original landing actors, animation curves and notify dependencies.
//! This is evidence preparation, not a replacement camera or gameplay controller.
use rc_package::{properties, read_package};
use std::{env, fs, path::Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: rc-rappel-audit GAME_DATA OUT".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let catalog = rc_inspect::defaults::load(root)?;
    let data = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let pkg = read_package(&data)?;
    let mut actors = vec![];
    for (i, e) in pkg.exports.iter().enumerate() {
        let class = pkg.object_path(e.class)?;
        let props = properties::read(&pkg, &data, e)?;
        let tag = catalog.instance(&class, &props, "Tag", 0, "geo_01a");
        let tag_text = serde_json::to_string(&tag)?;
        if !["Rope", "PlayerStart", "GeonosianWarrior", "CloneTrooper"]
            .iter()
            .any(|s| class.ends_with(s))
            && !["IntroWarrior", "GunshipWarrior", "PlayerStand"]
                .iter()
                .any(|s| tag_text.contains(s))
        {
            continue;
        }
        let mut resolved = serde_json::Map::new();
        for name in [
            "Tag",
            "Location",
            "Rotation",
            "Mesh",
            "MeshSets",
            "DrawScale",
            "Physics",
            "BaseEyeHeight",
            "EyeHeight",
            "bHidden",
            "bCollideActors",
            "bBlockActors",
        ] {
            if let Ok(v) = catalog.instance(&class, &props, name, 0, "geo_01a") {
                resolved.insert(name.into(), serde_json::to_value(v)?);
            }
        }
        actors.push(serde_json::json!({"original":i+1,"class":class,"properties":props,"resolved":resolved}));
    }
    let empty = properties::Properties {
        values: vec![],
        native_offset: 0,
    };
    let mut defaults = vec![];
    for class in [
        "CTCharacters.PlayerCommando",
        "Gameplay.ACTION_PlayPropAnim",
        "Gameplay.ACTION_Briefing",
    ] {
        for name in [
            "Mesh",
            "BaseEyeHeight",
            "EyeHeight",
            "Physics",
            "AnimRate",
            "StartFrame",
            "Channel",
            "MaxPitch",
            "MinPitch",
            "MaxYaw",
            "MinYaw",
        ] {
            if let Ok(v) = catalog.instance(class, &empty, name, 0, class) {
                defaults.push(serde_json::json!({"class":class,"property":name,"resolved":v}));
            }
        }
    }
    fs::write(
        out.join("actors.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"actors":actors,"defaults":defaults}))?,
    )?;
    let mut inventory = vec![];
    for file in ["Clone.ukx", "miscanimprops.ukx", "Geonosian.ukx"] {
        let data = fs::read(root.join("Animations").join(file))?;
        let pkg = read_package(&data)?;
        for (i, e) in pkg.exports.iter().enumerate() {
            if pkg.object_path(e.class)? != "Engine.MeshAnimation" {
                continue;
            }
            let animation = rc_package::skeletal_animation::read_mesh_animation(&pkg, &data, e)?;
            for s in animation.sequences {
                if file != "miscanimprops.ukx"
                    && !["Rope", "WarriorIntro"]
                        .iter()
                        .any(|n| s.name.name.contains(n))
                {
                    continue;
                }
                let mut notifies = vec![];
                for n in &s.notifies {
                    let e = pkg
                        .exports
                        .get(usize::try_from(n.object_index - 1)?)
                        .ok_or("Missing notify export")?;
                    notifies.push(serde_json::json!({"original":n.object_index,"normalized_time":f32::from_bits(n.time_bits),"seconds":f32::from_bits(n.time_bits)*s.frames as f32/f32::from_bits(s.rate_bits),"class":pkg.object_path(e.class)?,"properties":properties::read(&pkg,&data,e)?}));
                }
                inventory.push(serde_json::json!({"file":file,"animation":pkg.object_path(i as i32+1)?,"name":s.name.name,"frames":s.frames,"rate":f32::from_bits(s.rate_bits),"flags":s.flags,"notifies":notifies}));
            }
        }
    }
    fs::write(
        out.join("animation-inventory.json"),
        serde_json::to_vec_pretty(&inventory)?,
    )?;
    let player = inventory
        .iter()
        .find(|v| v["animation"] == "CloneCommandoSPSet" && v["name"] == "EntranceRope600")
        .ok_or("Missing original player sequence")?;
    fs::write(
        out.join("player-notifies.json"),
        serde_json::to_vec_pretty(&player["notifies"])?,
    )?;
    let mut resolver = rc_inspect::assets::Assets::new(&[
        &root.join("Textures"),
        &root.join("Animations"),
        &root.join("System"),
    ])?;
    let mesh = rc_inspect::visual_assets::load_from_animation(
        root,
        "Clone.ukx",
        "CloneCommando",
        &["EntranceRope600"],
        &mut resolver,
        Some("Clone.CloneCommandoSPSet"),
    )?;
    fs::write(out.join("player-mesh.json"), serde_json::to_vec(&mesh)?)?;
    let prepared = mesh.prepare_reference()?;
    let clip = prepared.clip("EntranceRope600")?;
    let seq = &prepared.mesh.clips[clip].sequence;
    let duration = seq.frames as f32 / f32::from_bits(seq.rate_bits);
    let mut samples = vec![];
    for frame in 0..seq.frames {
        let normalized = frame as f32 / seq.frames as f32;
        let channel = prepared.channel(clip, normalized)?;
        let head = prepared.attachment_transform(&channel, "Head", false)?;
        samples.push(serde_json::json!({"frame":frame,"seconds":frame as f32/f32::from_bits(seq.rate_bits),"root":prepared.root_position(clip,normalized)?,"root_delta":prepared.cinematic_root_delta(clip,normalized)?,"head":head}));
    }
    fs::write(
        out.join("player-curves.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"source":prepared.mesh.clips[clip].source,"frames":seq.frames,"rate":f32::from_bits(seq.rate_bits),"duration":duration,"flags":seq.flags,"samples":samples,"camera_parity":false,"note":"Head is a sampled skeletal attachment, not an established camera contract. Native root-motion and first-person view binding remain to be established."}),
        )?,
    )?;
    let mut bindings = vec![];
    for (label, file, object, filter, clips) in [
        (
            "rope",
            "miscanimprops.ukx",
            "Rope",
            Some("miscanimprops.RopeSet"),
            vec!["ActionBreathe", "CloneEntrance600"],
        ),
        (
            "trooper",
            "Clone.ukx",
            "CloneTrooper",
            Some("Clone.cGEO01CloneSet"),
            vec!["WarriorIntroBreathe", "WarriorIntro"],
        ),
        (
            "warrior",
            "Geonosian.ukx",
            "GeonosianWarrior",
            Some("Geonosian.cGEO01WarriorSet"),
            vec!["WarriorIntroBreathe", "WarriorIntro"],
        ),
    ] {
        let mesh = rc_inspect::visual_assets::load_from_animation(
            root,
            file,
            object,
            &clips,
            &mut resolver,
            filter,
        )?;
        fs::write(
            out.join(format!("{label}-mesh.json")),
            serde_json::to_vec(&mesh)?,
        )?;
        let prepared = mesh.prepare_reference()?;
        for c in 0..prepared.mesh.clips.len() {
            let seq = &prepared.mesh.clips[c].sequence;
            for frame in [0., 0.5, 1. - 1. / seq.frames as f32] {
                let geometry = prepared.geometry_with_root(&prepared.channel(c, frame)?, false)?;
                let mut bounds = [[f32::INFINITY; 3], [f32::NEG_INFINITY; 3]];
                for v in geometry.triangles.iter().flat_map(|t| t.points) {
                    for i in 0..3 {
                        bounds[0][i] = bounds[0][i].min(v[i]);
                        bounds[1][i] = bounds[1][i].max(v[i]);
                    }
                }
                bindings.push(serde_json::json!({"label":label,"source":prepared.mesh.clips[c].source,"frame":frame,"bounds":bounds,"root_delta":prepared.cinematic_root_delta(c,frame)?,"triangles":geometry.triangles.len()}));
            }
        }
    }
    fs::write(
        out.join("mesh-bindings.json"),
        serde_json::to_vec_pretty(&bindings)?,
    )?;
    println!(
        "{} EntranceRope600 samples, {duration}s, seven original notify exports",
        samples.len()
    );
    Ok(())
}
