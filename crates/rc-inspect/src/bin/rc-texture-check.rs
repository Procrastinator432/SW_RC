use rc_package::{read_package, texture::read_texture};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("Usage: rc-texture-check <Textures-directory> <report.json>".into());
    }
    let mut files = fs::read_dir(&args[1])?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;
    files.sort();
    let mut results = Vec::new();
    let (mut textures, mut failures) = (0, 0);
    for file in files
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("utx")))
    {
        let data = fs::read(&file)?;
        let pkg = read_package(&data)?;
        for (i, e) in pkg.exports.iter().enumerate() {
            if pkg.object_path(e.class)? != "Engine.Texture" {
                continue;
            }
            textures += 1;
            match read_texture(&pkg,&data,e) {
                Ok(t)=>results.push(serde_json::json!({"file":file,"object":pkg.object_path(i as i32+1)?,"format":t.format,"mips":t.mips.iter().map(|m|serde_json::json!({"width":m.width,"height":m.height,"bytes":m.data.len()})).collect::<Vec<_>>()})),
                Err(error)=>{failures+=1;results.push(serde_json::json!({"file":file,"object":pkg.object_path(i as i32+1)?,"error":error}));}
            }
        }
    }
    if let Some(parent) = Path::new(&args[2]).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"textures":textures,"failures":failures,"results":results}),
        )?,
    )?;
    println!("{textures} textures; {failures} read failures");
    if failures > 0 {
        return Err("Texture scan has failures; see report".into());
    }
    Ok(())
}
