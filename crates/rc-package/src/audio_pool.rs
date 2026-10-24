//! Full FindSource decision (103d0d10); virtual pool/driver operations stay owned
//! by the caller. Selection is read-only; callbacks execute stop then attach.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slot {
    pub occupied: bool,
    pub identity: u32,
    pub priority: f32,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Incoming {
    pub identity: u32,
    pub priority: f32,
}
pub trait PoolHost {
    fn stop(&mut self, index: usize) -> Result<(), String>;
    fn attach(&mut self, index: usize, incoming: Incoming) -> Result<(), String>;
}
pub fn select(incoming: Incoming, pool: &[Slot]) -> Result<Option<usize>, String> {
    if pool.len() > 4096
        || !incoming.priority.is_finite()
        || pool.iter().any(|s| !s.priority.is_finite())
    {
        return Err("Invalid audio source pool".into());
    }
    let mut candidate = None;
    let mut priority = incoming.priority;
    for (i, source) in pool.iter().enumerate() {
        // The original stops scanning at the FIRST free slot, even if a later
        // occupied slot has a matching protected ID. Do not reorder these gates.
        if !source.occupied {
            return Ok(Some(i));
        }
        if (source.identity ^ incoming.identity) & 0xffff_fffe == 0 {
            return Ok(if source.identity & 1 != 0 {
                None
            } else {
                Some(i)
            });
        }
        if source.priority < priority {
            priority = source.priority;
            candidate = Some(i);
        }
    }
    Ok(candidate)
}
pub fn apply(
    incoming: Incoming,
    pool: &[Slot],
    host: &mut impl PoolHost,
) -> Result<Option<usize>, String> {
    let choice = select(incoming, pool)?;
    if let Some(index) = choice {
        host.stop(index)?;
        host.attach(index, incoming)?;
    }
    Ok(choice)
}
