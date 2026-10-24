//! Update's spatial soft-reservation slice, 103d96c2..103d97ac.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub queued: bool,
    pub flags: u32,
    pub priority_bits: u32,
    pub name: String,
}
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub order: Vec<usize>,
    pub stopped: Vec<usize>,
    pub considered: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedSound {
    pub sound: u32,
    pub name: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub limit: i32,
    pub names: Vec<NamedSound>,
}
impl Config {
    pub fn validate(&self) -> Result<(), String> {
        let mut seen = std::collections::BTreeSet::new();
        if !(0..=4096).contains(&self.limit)
            || self.names.len() > 4096
            || self
                .names
                .iter()
                .any(|n| n.sound == 0 || !seen.insert(n.sound) || !name_valid(&n.name))
        {
            return Err("Invalid reservation sound names".into());
        }
        Ok(())
    }
    pub fn name(&self, sound: u32) -> Result<&str, String> {
        self.names
            .iter()
            .find(|n| n.sound == sound)
            .map(|n| n.name.as_str())
            .ok_or_else(|| "Missing original reservation sound name".into())
    }
}
pub fn name_valid(name: &str) -> bool {
    !name.is_empty() && name.len() <= 256 && name.bytes().all(|c| (32..=126).contains(&c))
}
pub fn plan(
    entries: &[Entry],
    channels: usize,
    remaining: i32,
    limit: i32,
) -> Result<Plan, String> {
    if entries.len() > 4096
        || channels > 4096
        || !(0..=channels as i32).contains(&remaining)
        || !(0..=4096).contains(&limit)
        || entries.iter().any(|e| {
            !name_valid(&e.name)
                || !f32::from_bits(e.priority_bits).is_finite()
                || f32::from_bits(e.priority_bits).abs() > 1e6
        })
    {
        return Err("Invalid spatial reservation input".into());
    }
    let mut out = Plan {
        order: (0..entries.len()).collect(),
        stopped: vec![],
        considered: 0,
    };
    if remaining > limit {
        return Ok(out);
    }
    let mut tail = channels;
    let mut cleared = vec![false; entries.len()];
    for index in 0..entries.len() {
        let id = out.order[index];
        let e = &entries[id];
        if cleared[id]
            || !e.queued
            || e.flags & 0x140 != 0x40
            || !(e.name.starts_with("imp_") || e.name == "exp_GEN_proxMine_01")
        {
            continue;
        }
        // The native comparison loads the exact f64 constant, not an f32 1.88.
        if remaining <= 0 || f64::from(f32::from_bits(e.priority_bits)) < 1.88f64 {
            if index < channels && tail < entries.len() {
                out.order.swap(index, tail);
                tail += 1;
            }
            out.stopped.push(id);
            cleared[id] = true;
        }
        out.considered += 1;
    }
    Ok(out)
}
