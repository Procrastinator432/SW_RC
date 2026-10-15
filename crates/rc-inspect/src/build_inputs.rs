//! Explicit, manifest-relative inputs for offline scene builders.
//! Generated assets belong outside Git; no builder silently reads an old delivery.
use serde::Deserialize;
use std::{collections::BTreeMap, env, fs, path::PathBuf};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: u32,
    inputs: BTreeMap<String, Entry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    path: PathBuf,
    sha256: String,
}
fn resolve(manifest: &std::path::Path, bytes: &[u8], key: &str) -> Result<PathBuf, String> {
    let doc: Manifest = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if doc.schema != 1 {
        return Err("Unsupported scene input manifest schema".into());
    }
    let entry = doc
        .inputs
        .get(key)
        .ok_or_else(|| format!("Missing scene input: {key}"))?;
    if entry.path.as_os_str().is_empty()
        || entry.sha256.len() != 64
        || !entry.sha256.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(format!("Invalid path/SHA-256 for scene input {key}"));
    }
    Ok(manifest
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join(&entry.path))
}
pub fn input(key: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(
        env::var_os("RC_BUILD_INPUTS")
            .ok_or("Set RC_BUILD_INPUTS to an explicit scene-input manifest; see docs/BUILD.md")?,
    );
    let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    resolve(&path, &bytes, key)
}
pub fn output() -> Result<PathBuf, String> {
    env::var_os("RC_BUILD_OUTPUT").map(PathBuf::from).ok_or_else(||
        "Set RC_BUILD_OUTPUT to a new build directory; archived deliveries are never implicit outputs".into())
}
pub fn game_data() -> Result<PathBuf, String> {
    env::var_os("RC_GAME_DATA")
        .map(PathBuf::from)
        .ok_or_else(|| {
            "Set RC_GAME_DATA to your installed Republic Commando GameData directory".into()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inputs_are_relative_to_manifest_and_missing_entries_do_not_fall_back() {
        let doc = serde_json::json!({"schema":1,"inputs":{"scene":{"path":"assets/base.rcplay","sha256":"ab".repeat(32)}}});
        let bytes = serde_json::to_vec(&doc).unwrap();
        assert_eq!(
            resolve(std::path::Path::new("config/inputs.json"), &bytes, "scene").unwrap(),
            PathBuf::from("config/assets/base.rcplay")
        );
        assert!(resolve(std::path::Path::new("inputs.json"), &bytes, "unknown").is_err());
    }
    #[test]
    fn invalid_schema_and_unpinned_input_are_rejected() {
        for doc in [
            serde_json::json!({"schema":2,"inputs":{}}),
            serde_json::json!({"schema":1,"inputs":{"scene":{"path":"asset","sha256":""}}}),
            serde_json::json!({"schema":1,"inputs":{"scene":{"path":"","sha256":"ab".repeat(32)}}}),
        ] {
            assert!(resolve(
                std::path::Path::new("inputs.json"),
                &serde_json::to_vec(&doc).unwrap(),
                "scene"
            )
            .is_err());
        }
    }
}
