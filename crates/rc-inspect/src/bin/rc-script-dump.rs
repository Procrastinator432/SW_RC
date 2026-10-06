//! Bounded function AST inspection; explicit errors, no source-text fallback.
use std::{env, fs};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() < 4 {
        return Err("Usage: rc-script-dump <package.u> <report.json> <function-path> ...".into());
    }
    let data = fs::read(&args[1])?;
    let pkg = rc_package::read_package(&data)?;
    let mut functions = Vec::new();
    let mut errors = 0;
    for path in &args[3..] {
        let mut found = false;
        for (i, e) in pkg.exports.iter().enumerate() {
            if !pkg.object_path(i as i32 + 1)?.eq_ignore_ascii_case(path) {
                continue;
            }
            found = true;
            let decoded = match rc_package::script::read_function(&pkg, &data, e) {
                Ok(value) => serde_json::to_value(value)?,
                Err(error) => {
                    errors += 1;
                    serde_json::json!({"error":error})
                }
            };
            functions.push(serde_json::json!({"path":path,"export_index":i+1,
                "serial_offset":e.serial_offset,"serial_size":e.serial_size,"decoded":decoded}));
        }
        if !found {
            errors += 1;
            functions.push(serde_json::json!({"path":path,"error":"function not found"}));
        }
    }
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(&serde_json::json!({"package":args[1],"errors":errors,
        "scope":"bounded serialized function AST; no VM/native call execution or runtime parity", "functions":functions}))?,
    )?;
    if errors != 0 {
        return Err(format!("{errors} function errors").into());
    }
    Ok(())
}
