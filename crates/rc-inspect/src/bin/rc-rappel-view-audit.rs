//! Audit original first-person rappel mesh/animation candidates, without substitutions.
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("usage: rc-rappel-view-audit GAME_DATA OUT.json".into());
    }
    let root = Path::new(&args[1]);
    let mut rows = vec![];
    for file in ["hudarms.ukx", "clone.ukx", "miscanimprops.ukx"] {
        let bytes = fs::read(root.join("Animations").join(file))?;
        let pkg = rc_package::read_package(&bytes)?;
        for (i, e) in pkg.exports.iter().enumerate() {
            let class = pkg.object_path(e.class)?;
            if class == "Engine.MeshAnimation" {
                let a = rc_package::skeletal_animation::read_mesh_animation(&pkg, &bytes, e)?;
                for s in a.sequences {
                    if [
                        "entrancerope",
                        "rappel",
                        "cloneentrance600",
                        "climb",
                        "ladder",
                    ]
                    .iter()
                    .any(|v| s.name.name.to_lowercase().contains(v))
                    {
                        rows.push(serde_json::json!({"file":file,"animation":pkg.object_path(i as i32+1)?,"clip":s.name.name,"frames":s.frames,"rate":f32::from_bits(s.rate_bits),"flags":s.flags}));
                    }
                }
            }
            if file == "hudarms.ukx" && class == "Engine.SkeletalMesh" {
                let p = rc_package::properties::read(&pkg, &bytes, e)?;
                rows.push(serde_json::json!({"file":file,"mesh":pkg.object_path(i as i32+1)?,"properties":p}));
            }
        }
    }
    let bytes = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let pkg = rc_package::read_package(&bytes)?;
    for id in [
        10usize, 11, 1237, 1238, 1239, 1240, 1241, 1242, 1243, 1244, 1245, 1246, 1247, 1248, 1249,
    ] {
        let e = &pkg.exports[id - 1];
        rows.push(serde_json::json!({"file":"Maps/geo_01a.ctm","original":id,"class":pkg.object_path(e.class)?,"path":pkg.object_path(id as i32)?,"properties":rc_package::properties::read(&pkg,&bytes,e)?}));
    }
    let catalog = rc_inspect::defaults::load(root)?;
    for class in [
        "CTCharacters.PlayerCommando",
        "Properties.PlayerCommandoDefaults",
    ] {
        for field in ["Skins", "Mesh", "bOwnerNoSee"] {
            let resolved = catalog.resolve(class, field, 0);
            rows.push(serde_json::json!({"default_class":class,"field":field,"resolved":resolved}));
            if field == "Skins" {
                if let Ok(v) = resolved {
                    let bytes = fs::read(&v.source_package)?;
                    let pkg = rc_package::read_package(&bytes)?;
                    if let Some((_, name)) = v.declaring_class.split_once('.') {
                        if let Some(e) = pkg
                            .exports
                            .iter()
                            .find(|e| e.class == 0 && e.name.eq_ignore_ascii_case(name))
                        {
                            let props = rc_package::classes::read(&pkg, &bytes, e)?.properties;
                            let payload = pkg.payload(&bytes, e)?;
                            for p in props.values.iter().filter(|p| p.name == "Skins") {
                                rows.push(serde_json::json!({"default_class":class,"skins":rc_package::properties::object_array(&pkg,&payload[p.payload_offset..p.payload_offset+p.bytes])}));
                            }
                        }
                    }
                }
            }
        }
    }
    fs::write(&args[2], serde_json::to_vec_pretty(&rows)?)?;
    for row in &rows {
        if row.get("clip").is_some() {
            println!("{row}");
        }
    }
    Ok(())
}
