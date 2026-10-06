use rc_package::{
    classes,
    defaults::{Catalog, Class},
    read_package,
};
use std::{fs, path::Path};
pub fn load(game_data: &Path) -> Result<Catalog, String> {
    let mut files = Vec::new();
    for directory in ["System", "Properties"] {
        for entry in fs::read_dir(game_data.join(directory)).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("u"))
            {
                files.push(path);
            }
        }
    }
    files.sort();
    let mut catalog = Catalog::default();
    for file in files {
        let data = fs::read(&file).map_err(|e| e.to_string())?;
        let pkg = read_package(&data)?;
        let package = file
            .file_stem()
            .and_then(|p| p.to_str())
            .ok_or("invalid package filename")?;
        for (i, export) in pkg.exports.iter().enumerate() {
            if export.class != 0 {
                continue;
            }
            let path = |index| -> Result<String, String> {
                let path = pkg.object_path(index)?;
                Ok(if index > 0 {
                    format!("{package}.{path}")
                } else {
                    path
                })
            };
            catalog.insert(Class {
                name: path(i as i32 + 1)?,
                parent: if export.super_class == 0 {
                    None
                } else {
                    Some(path(export.super_class)?)
                },
                package: package.into(),
                source: file.to_string_lossy().into_owned(),
                properties: classes::read(&pkg, &data, export)?.properties,
                fields: classes::fields(&pkg, &data, export)?,
            })?;
        }
    }
    Ok(catalog)
}
