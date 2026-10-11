//! Versioned input replay; never assigns position, aim, health or encounter answers.
//! SHA256 binding is checked by the Python runner. FNV is a portable mismatch
//! diagnostic inside Rust, not an authentication or cryptographic hash.
use crate::{Game, Input};
use serde::{Deserialize, Serialize};

pub fn fingerprint(bytes: &[u8]) -> String {
    let mut value = 0xcbf29ce484222325_u64;
    for &byte in bytes {
        value = (value ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    format!("{value:016x}")
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", deny_unknown_fields)]
pub enum Action {
    Frame { elapsed: f64, input: Input },
    Suspend,
    Restart,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub stage: String,
    pub action: Action,
    pub expected_ticks: u32,
    pub expected_state: String,
    pub pixels_640x400: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Replay {
    pub version: u32,
    pub bundle_bytes: usize,
    pub bundle_fingerprint: String,
    pub bundle_sha256: Option<String>,
    pub rng: String,
    pub entries: Vec<Entry>,
}
pub fn state_fingerprint(game: &Game) -> Result<String, String> {
    serde_json::to_vec(&(
        &game.state,
        game.accumulator,
        game.pending_jump,
        game.pending_fire,
        game.pending_reload,
    ))
    .map(|bytes| fingerprint(&bytes))
    .map_err(|e| e.to_string())
}
pub fn apply(game: &mut Game, action: &Action) -> Result<u32, String> {
    match *action {
        Action::Frame { elapsed, input } => game.advance(elapsed, input),
        Action::Suspend => {
            game.suspend();
            Ok(0)
        }
        Action::Restart => {
            game.restart();
            Ok(0)
        }
    }
}
pub fn pixels(game: &Game) -> Result<String, String> {
    Ok(fingerprint(
        &game
            .render(640, 400)?
            .iter()
            .flat_map(|p| p.to_le_bytes())
            .collect::<Vec<_>>(),
    ))
}
impl Replay {
    pub fn new(bundle: &[u8]) -> Self {
        Self {
            version: 1,
            bundle_bytes: bundle.len(),
            bundle_fingerprint: fingerprint(bundle),
            bundle_sha256: None,
            rng: "none: current simulation has no random generator".into(),
            entries: vec![],
        }
    }
    pub fn validate_bundle(&self, bundle: &[u8]) -> Result<(), String> {
        if self.version != 1
            || self.bundle_bytes != bundle.len()
            || self.bundle_fingerprint != fingerprint(bundle)
            || self.entries.is_empty()
            || self.entries.len() > 100_000
            || self.rng != "none: current simulation has no random generator"
        {
            return Err("Unsupported replay, bundle mismatch or entry limit".into());
        }
        Ok(())
    }
    pub fn record(
        &mut self,
        game: &mut Game,
        action: Action,
        render: bool,
        stage: &str,
    ) -> Result<(), String> {
        let expected_ticks = apply(game, &action)?;
        self.entries.push(Entry {
            stage: stage.into(),
            action,
            expected_ticks,
            expected_state: state_fingerprint(game)?,
            pixels_640x400: if render { Some(pixels(game)?) } else { None },
        });
        Ok(())
    }
    pub fn verify_entry(game: &mut Game, entry: &Entry, render: bool) -> Result<u32, String> {
        let ticks = apply(game, &entry.action)?;
        if ticks != entry.expected_ticks || state_fingerprint(game)? != entry.expected_state {
            return Err("Replay tick/state mismatch".into());
        }
        if render {
            if let Some(expected) = &entry.pixels_640x400 {
                if pixels(game)? != *expected {
                    return Err("Replay pixels mismatch".into());
                }
            }
        }
        Ok(ticks)
    }
}
