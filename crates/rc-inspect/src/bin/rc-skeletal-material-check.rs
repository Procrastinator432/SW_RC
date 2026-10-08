use rc_inspect::assets::Assets;
use rc_package::{
    read_package, skeletal_lod::read_skeletal_lods, skeletal_material::select_lod_material,
    skeletal_mesh::read_skeletal_mesh_prefix,
};
use std::{
    collections::{BTreeSet, HashMap},
    env, fs,
    path::Path,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-skeletal-material-check LINKUPS GAME_DATA OUTPUT".into());
    }
    let links: serde_json::Value = serde_json::from_slice(&fs::read(&args[1])?)?;
    let game = Path::new(&args[2]);
    let dirs = [
        game.join("Textures"),
        game.join("StaticMeshes"),
        game.join("Animations"),
        game.join("System"),
    ];
    let mut assets = Assets::new(&dirs.iter().map(|p| p.as_path()).collect::<Vec<_>>())?;
    let mut textures = vec![];
    let mut cache: HashMap<String, Result<usize, String>> = HashMap::new();
    let mut objects = vec![];
    let (mut resolved, mut omitted) = (0, 0);
    for (index, original) in links["objects"]
        .as_array()
        .ok_or("objects")?
        .iter()
        .enumerate()
    {
        if original["status"] == "UnsupportedLegacyPackage" {
            continue;
        }
        let file = Path::new(original["file"].as_str().ok_or("file")?);
        let bytes = fs::read(file)?;
        let pkg = read_package(&bytes)?;
        let export = &pkg.exports[original["export_index"].as_u64().ok_or("export")? as usize - 1];
        let prefix = read_skeletal_mesh_prefix(&pkg, &bytes, export)?;
        let mesh = read_skeletal_lods(&pkg, &bytes, export)?;
        let package = file.file_stem().ok_or("package")?.to_string_lossy();
        let qualify = |reference: i32| -> Result<String, String> {
            let path = pkg.object_path(reference)?;
            Ok(if reference > 0 {
                format!("{package}.{path}")
            } else {
                path
            })
        };
        let stored: Vec<_> = prefix
            .materials
            .iter()
            .map(|&r| {
                Ok(serde_json::json!({"index":r,"path":if r==0 {None} else {Some(qualify(r)?)} }))
            })
            .collect::<Result<_, String>>()?;
        let refs: Vec<_> = prefix
            .materials
            .iter()
            .map(|&i| (i != 0).then_some(i))
            .collect();
        let slots: BTreeSet<_> = mesh
            .lods
            .iter()
            .flat_map(|l| l.sections.iter().flatten())
            .filter(|s| s[8] != 0)
            .map(|s| s[0] as usize)
            .collect();
        let mut bindings = vec![];
        for slot in slots {
            let reference = select_lod_material(slot, &refs, None, |_| None);
            let material = reference.map(qualify).transpose()?;
            let result = if let Some(path) = &material {
                cache.entry(path.to_lowercase()).or_insert_with(|| {
                    let d=assets.diffuse_at_size(path,256)?;let i=textures.len();
                    textures.push(serde_json::json!({"material":path,"source":d.source,"chain":d.chain,"uv_scale":d.scale,"format":d.format,"source_mip":d.source_mip,
                        "width":d.texture.width,"height":d.texture.height,"pixels":d.texture.pixels}));Ok(i)
                }).clone()
            } else {
                Err("No selected mesh material".into())
            };
            let (texture, error) = match result {
                Ok(i) => {
                    resolved += 1;
                    (Some(i), None)
                }
                Err(e) => {
                    omitted += 1;
                    (None, Some(e))
                }
            };
            bindings.push(serde_json::json!({"slot":slot,"selected_reference":reference,"material":material,"texture":texture,"error":error}));
        }
        objects.push(serde_json::json!({"source_index":index,"object":original["object"],"materials_offset":prefix.materials_offset,"stored":stored,"bindings":bindings}));
    }
    fs::write(
        &args[3],
        serde_json::to_vec(
            &serde_json::json!({"objects":objects,"textures":textures,"resolved_slots":resolved,"omitted_slots":omitted,
        "scope":"Original mesh material references and nonnegative ULodMeshInstance::GetMaterial selection with no actor supplied. Original base diffuse textures/mips/palettes and supported material chains/UV scale; shader effects, actor instance overrides and unhandled chains remain explicit omissions."}),
        )?,
    )?;
    println!(
        "{resolved} resolved slots, {omitted} omitted slots, {} unique diffuse textures",
        textures.len()
    );
    Ok(())
}
