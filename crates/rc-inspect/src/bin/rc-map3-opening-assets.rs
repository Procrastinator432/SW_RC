use rc_inspect::visual_assets;
use rc_package::{properties, read_package};
use std::{fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new("D:/SteamLibrary/steamapps/common/Star Wars Republic Commando/GameData");
    let out = Path::new("analysis/mission20-map3-opening");
    let catalog = rc_inspect::defaults::load(root)?;
    for class in [
        "CTCharacters.PlayerCommando",
        "CTCharacters.Commando07Delta",
        "Properties.GeonosianSunFac",
        "AnimPropProperties.Rope",
    ] {
        println!("{class}: {:?}", catalog.resolve(class, "Mesh", 0)?);
    }
    println!(
        "Sun Fac skins {:?}",
        catalog.resolve("Properties.GeonosianSunFac", "Skins", 0)
    );
    let data = fs::read(root.join("Maps/geo_01c.ctm"))?;
    let pkg = read_package(&data)?;
    let mut rope_receipts = vec![];
    for id in [100, 102] {
        let e = &pkg.exports[id - 1];
        let props = properties::read(&pkg, &data, e)?;
        let payload = pkg.payload(&data, e)?;
        let a = props
            .values
            .iter()
            .find(|p| p.name == "AnimTriggered")
            .ok_or("Missing original rope receiver")?;
        rope_receipts.push(serde_json::json!({"original":id,"property":a,"decoded":properties::tagged_struct(&pkg,&payload[a.payload_offset..a.payload_offset+a.bytes])}));
    }
    fs::write(
        out.join("rope-receivers.json"),
        serde_json::to_vec_pretty(&rope_receipts)?,
    )?;
    let mut inventory = vec![];
    for file in ["Clone.ukx", "Geonosian.ukx", "miscanimprops.ukx"] {
        let data = fs::read(root.join("Animations").join(file))?;
        let p = read_package(&data)?;
        for (i, e) in p.exports.iter().enumerate() {
            if p.object_path(e.class)? != "Engine.MeshAnimation" {
                continue;
            }
            let a = rc_package::skeletal_animation::read_mesh_animation(&p, &data, e)?;
            for seq in a.sequences {
                if [
                    "EntranceRope1760",
                    "SevIntro",
                    "SevIntroNoPike",
                    "SevIntroWithPike",
                    "ActionBreathe",
                    "GestureSignalOK",
                ]
                .contains(&seq.name.name.as_str())
                {
                    let mut notifies = vec![];
                    for n in &seq.notifies {
                        let e = &p.exports[n.object_index as usize - 1];
                        notifies.push(serde_json::json!({"original":n.object_index,"time_bits":n.time_bits,"class":p.object_path(e.class)?,"properties":properties::read(&p,&data,e)?}));
                    }
                    inventory.push(serde_json::json!({"file":file,"set":p.object_path(i as i32+1)?,"name":seq.name.name,"frames":seq.frames,"rate":f32::from_bits(seq.rate_bits),"flags":seq.flags,"notifies":notifies}));
                }
            }
        }
    }
    fs::write(
        out.join("animation-inventory.json"),
        serde_json::to_vec_pretty(&inventory)?,
    )?;
    let mut assets = rc_inspect::assets::Assets::new(&[
        &root.join("Textures"),
        &root.join("Animations"),
        &root.join("System"),
    ])?;
    let player = visual_assets::load_from_animation(
        root,
        "Clone.ukx",
        "CloneCommando",
        &["EntranceRope1760"],
        &mut assets,
        Some("Clone.CloneCommandoSPSet"),
    )?;
    let prepared = player.prepare_reference()?;
    let seq = &prepared.mesh.clips[0].sequence;
    let mut curves = vec![];
    for i in 0..seq.frames {
        let frame = i as f32 / seq.frames as f32;
        curves.push(serde_json::json!({"frame":i,"matrix":prepared.root_matrix(0,frame)?.map(f32::from_bits),"delta":prepared.cinematic_root_delta(0,frame)?}));
    }
    fs::write(
        out.join("player-mesh.json"),
        serde_json::to_vec(&prepared.mesh)?,
    )?;
    fs::write(
        out.join("player-curves.json"),
        serde_json::to_vec_pretty(&curves)?,
    )?;
    println!(
        "Player root first {:?}, last {:?}",
        curves.first(),
        curves.last()
    );
    Ok(())
}
