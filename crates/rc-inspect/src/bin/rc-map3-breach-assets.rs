use rc_package::{properties, read_package};
use std::{fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new("D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData");
    let out = Path::new("analysis/mission24-map3-breach");
    fs::create_dir_all(out)?;
    let bacta = fs::read(root.join("Animations/BactaDispensers.ukx"))?;
    let bp = read_package(&bacta)?;
    let mut bacta_info = vec![];
    for (i, e) in bp.exports.iter().enumerate() {
        let class = bp.object_path(e.class)?;
        if class == "Engine.MeshAnimation" {
            let a = rc_package::skeletal_animation::read_mesh_animation(&bp, &bacta, e)?;
            bacta_info.push(serde_json::json!({"set":bp.object_path(i as i32+1)?,"sequences":a.sequences.iter().map(|s|&s.name.name).collect::<Vec<_>>()}));
        }
    }
    fs::write(
        out.join("bacta-animations.json"),
        serde_json::to_vec_pretty(&bacta_info)?,
    )?;
    let mut clips = vec![];
    let data = fs::read(root.join("Animations/Clone.ukx"))?;
    let p = read_package(&data)?;
    for (i, e) in p.exports.iter().enumerate() {
        if p.object_path(e.class)? != "Engine.MeshAnimation" {
            continue;
        }
        let a = rc_package::skeletal_animation::read_mesh_animation(&p, &data, e)?;
        for s in a.sequences {
            if !s.name.name.to_lowercase().contains("breach")
                && !s.name.name.to_lowercase().contains("door")
            {
                continue;
            }
            let mut notifies = vec![];
            for n in &s.notifies {
                let e = &p.exports[n.object_index as usize - 1];
                notifies.push(serde_json::json!({"original":n.object_index,"time_bits":n.time_bits,"class":p.object_path(e.class)?,"properties":properties::read(&p,&data,e)?}));
            }
            println!(
                "{} {} frames={} flags={}",
                p.object_path(i as i32 + 1)?,
                s.name.name,
                s.frames,
                s.flags
            );
            clips.push(serde_json::json!({"set":p.object_path(i as i32+1)?,"sequence":s,"notifies":notifies}));
        }
    }
    let mut classes = vec![];
    for file in [
        "Properties/markerproperties.u",
        "System/ctmarkers.u",
        "System/ctgame.u",
    ] {
        let data = fs::read(root.join(file))?;
        let p = read_package(&data)?;
        for (i, e) in p.exports.iter().enumerate() {
            if e.class != 0 {
                continue;
            }
            let name = p.object_path(i as i32 + 1)?;
            if ![
                "MarkerDoorBreach",
                "MarkerDoorBreachNative",
                "BactaDispenserGEO",
                "BactaDispenser",
            ]
            .contains(&name.as_str())
            {
                continue;
            }
            let a = rc_package::classes::read(&p, &data, e)?;
            let payload = p.payload(&data, e)?;
            let mut nested = vec![];
            for v in &a.properties.values {
                if v.name == "Participants" {
                    nested.push(serde_json::json!({"property":v.name,"slot":v.array_index,"decoded":properties::tagged_struct(&p,&payload[v.payload_offset..v.payload_offset+v.bytes])}));
                }
                if v.name == "MarkerAnims" {
                    let raw = &payload[v.payload_offset..v.payload_offset + v.bytes];
                    let mut offset = 1;
                    let mut names = vec![];
                    for _ in 0..raw[0] {
                        let first = raw[offset];
                        offset += 1;
                        let mut id = (first & 63) as i32;
                        let mut shift = 6;
                        let mut more = first & 64 != 0;
                        while more {
                            let b = raw[offset];
                            offset += 1;
                            id |= ((b & 127) as i32) << shift;
                            shift += 7;
                            more = b & 128 != 0;
                        }
                        names.push(p.name(id)?);
                    }
                    assert_eq!(offset, raw.len());
                    nested.push(serde_json::json!({"property":v.name,"name_array":names}));
                }
            }
            let fields = rc_package::classes::fields(&p, &data, e)?;
            classes.push(serde_json::json!({"file":file,"class":name,"parent":p.object_path(e.super_class)?,"defaults":a,"fields":fields,"nested":nested}));
        }
    }
    let mut resolver = rc_inspect::assets::Assets::new(&[
        &root.join("Textures"),
        &root.join("Animations"),
        &root.join("System"),
    ])?;
    let mesh = rc_inspect::visual_assets::load_from_animation(
        root,
        "Clone.ukx",
        "CloneCommando",
        &[
            "DoorBreachLoudSM1Doorman",
            "DoorBreachLoudSM2Grenademan",
            "DoorBreachLoudSM3Coverman",
        ],
        &mut resolver,
        Some("Clone.mDoorBreachSet"),
    )?;
    let mesh = mesh.prepare_reference()?;
    let mut roots = vec![];
    for (i, c) in mesh.mesh.clips.iter().enumerate() {
        let last = (c.sequence.frames - 1) as f32 / c.sequence.frames as f32;
        roots.push(serde_json::json!({"clip":c.sequence.name.name,"first":mesh.root_matrix(i,0.)?.map(f32::from_bits),"last":mesh.root_matrix(i,last)?.map(f32::from_bits)}));
    }
    fs::write(
        out.join("breach-clips.json"),
        serde_json::to_vec(&mesh.mesh.clips)?,
    )?;
    fs::write(
        out.join("root-anchors.json"),
        serde_json::to_vec_pretty(&roots)?,
    )?;
    let map_data = fs::read(root.join("Maps/geo_01c.ctm"))?;
    let mp = read_package(&map_data)?;
    let e = &mp.exports[124];
    let props = properties::read(&mp, &map_data, e)?;
    let payload = mp.payload(&map_data, e)?;
    let damage = props
        .values
        .iter()
        .find(|v| v.name == "DoorDamagedState")
        .unwrap();
    fs::write(
        out.join("door-damaged-state.json"),
        serde_json::to_vec_pretty(&properties::tagged_struct(
            &mp,
            &payload[damage.payload_offset..damage.payload_offset + damage.bytes],
        )?)?,
    )?;
    fs::write(
        out.join("original-assets.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"clips":clips,"classes":classes}))?,
    )?;
    Ok(())
}
