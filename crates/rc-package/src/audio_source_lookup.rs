//! Original active/queued lookup contracts. Stable slices replace native arrays.
use crate::audio_source::Source;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Found {
    Active(usize),
    Queued(usize),
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    pub actor: u32,
    pub sound: u32,
    /// Params+54 bit 0 AND Sound+30 bit 1, with engine actor resolved externally.
    pub auto_actor: bool,
    pub engine_actor: u32,
}
fn validate(active: &[Source], queued: &[Source]) -> Result<(), String> {
    if active.len() > 4096 || queued.len() > 4096 {
        return Err("Too many lookup sources".into());
    }
    for s in active.iter().chain(queued) {
        s.validate()?;
    }
    Ok(())
}
pub fn queued(sources: &[Source], actor: u32, sound: u32) -> Result<Option<usize>, String> {
    validate(&[], sources)?;
    Ok(sources.iter().position(|s| s.matches(actor, sound)))
}
/// GetSourceIndex requires an actor (possibly auto-resolved) AND a sound.
/// Native actor=null/sound=null dereferences null; the portable API rejects it.
pub fn index(sources: &[Source], query: Query) -> Result<Option<usize>, String> {
    validate(sources, &[])?;
    if query.actor == 0 && query.sound == 0 {
        return Err("Null audio index query".into());
    }
    let actor = if query.actor == 0 && query.auto_actor {
        query.engine_actor
    } else {
        query.actor
    };
    Ok(if actor == 0 || query.sound == 0 {
        None
    } else {
        sources.iter().position(|s| s.matches(actor, query.sound))
    })
}
pub fn source(
    active: &[Source],
    waiting: &[Source],
    query: Query,
) -> Result<Option<Found>, String> {
    validate(active, waiting)?;
    if let Some(i) = index(active, query)? {
        return Ok(Some(Found::Active(i)));
    }
    // Original arguments, NOT the actor resolved inside GetSourceIndex.
    Ok(queued(waiting, query.actor, query.sound)?.map(Found::Queued))
}
pub fn by_id(active: &[Source], waiting: &[Source], id: u32) -> Result<Option<Found>, String> {
    validate(active, waiting)?;
    Ok(active
        .iter()
        .position(|s| s.unique_id == id)
        .map(Found::Active)
        .or_else(|| {
            waiting
                .iter()
                .position(|s| s.unique_id == id)
                .map(Found::Queued)
        }))
}
