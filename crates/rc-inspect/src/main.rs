use serde_json::{json, Value};
use std::{env, fs, path::Path, process};
mod pe;
fn write(path: &Path, value: &Value) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}
fn scan(
    root: &Path,
    dir: &Path,
    out: &Path,
    records: &mut Vec<Value>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut files: Vec<_> = fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    files.sort_by_key(|f| f.file_name());
    for f in files {
        let p = f.path();
        let m = f.metadata()?;
        if m.is_symlink() {
            continue;
        }
        if m.is_dir() {
            scan(root, &p, out, records)?;
            continue;
        }
        let rel = p.strip_prefix(root)?.to_string_lossy().replace('\\', "/");
        let ext = p
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let mut record = json!({"path":rel,"bytes":m.len()});
        if ["u", "utx", "usx", "ukx", "uax", "ctm", "unr", "ums"].contains(&ext.as_str()) {
            let data = fs::read(&p)?;
            match rc_package::read_package(&data) {
                Ok(pkg) => {
                    record["package"] = serde_json::to_value(&pkg.summary)?;
                    let dest = out
                        .join("packages")
                        .join(format!("{}.json", rel.replace('/', "__")));
                    let mut detail = serde_json::to_value(&pkg)?;
                    let mut source_count = 0;
                    let mut source_errors = Vec::new();
                    for (i, e) in pkg.exports.iter().enumerate() {
                        detail["exports"][i]["object_path"] = json!(pkg.object_path(i as i32 + 1)?);
                        detail["exports"][i]["class_path"] = json!(if e.class == 0 {
                            "Class".to_string()
                        } else {
                            pkg.object_path(e.class)?
                        });
                        if detail["exports"][i]["class_path"] == "Core.TextBuffer" {
                            match pkg.text_buffer(&data, e) {
                                Ok(source) => {
                                    let object = pkg.object_path(i as i32 + 1)?;
                                    let class =
                                        object.strip_suffix(".ScriptText").unwrap_or(&object);
                                    let safe: String = class
                                        .chars()
                                        .map(|c| {
                                            if c.is_ascii_alphanumeric() || c == '.' || c == '_' {
                                                c
                                            } else {
                                                '_'
                                            }
                                        })
                                        .collect();
                                    let dest = out.join("sources").join(rel.replace('/', "__"));
                                    fs::create_dir_all(&dest)?;
                                    fs::write(dest.join(format!("{safe}.uc")), source)?;
                                    source_count += 1;
                                }
                                Err(e) => source_errors.push(json!({"export":i+1,"error":e})),
                            }
                        }
                    }
                    record["recovered_text_buffers"] = json!(source_count);
                    if !source_errors.is_empty() {
                        record["source_errors"] = json!(source_errors);
                    }
                    write(&dest, &detail)?;
                }
                Err(e) => record["error"] = json!(e),
            }
        } else if ["exe", "dll"].contains(&ext.as_str()) {
            match pe::inspect(&fs::read(&p)?) {
                Ok(info) => {
                    write(
                        &out.join("binaries")
                            .join(format!("{}.json", rel.replace('/', "__"))),
                        &info,
                    )?;
                    record["pe"] = info;
                }
                Err(e) => record["error"] = json!(e),
            }
        }
        records.push(record);
    }
    Ok(())
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 3 {
        return Err("Usage: rc-inspect <GameData-directory> <report-directory>".into());
    }
    let root = Path::new(&args[1]).canonicalize()?;
    let out = Path::new(&args[2]);
    fs::create_dir_all(out)?;
    let out = out.canonicalize()?;
    if out.starts_with(&root) {
        return Err("report directory must be outside GameData".into());
    }
    fs::create_dir_all(out.join("packages"))?;
    fs::create_dir_all(out.join("binaries"))?;
    let mut records = Vec::new();
    scan(&root, &root, &out, &mut records)?;
    let errors = records.iter().filter(|r| r.get("error").is_some()).count();
    write(
        &out.join("inventory.json"),
        &json!({"source":root,"files":records,"parse_errors":errors}),
    )?;
    println!(
        "Inspected {} files; {} parse errors. Reports: {}",
        records.len(),
        errors,
        out.display()
    );
    if errors != 0 {
        return Err("one or more files failed parsing; see inventory.json".into());
    }
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        process::exit(1);
    }
}
