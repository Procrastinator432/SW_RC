//! Selected original Geonosian bot melee budget; navigation and goal scheduling are adapters.
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub provenance: serde_json::Value,
    pub burst_count: i32,
}
impl Assets {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=32).contains(&self.burst_count) {
            return Err("Invalid selected bot melee budget".into());
        }
        Ok(())
    }
}
