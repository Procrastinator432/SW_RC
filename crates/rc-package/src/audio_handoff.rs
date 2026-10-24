//! Pure persisted composition of preparation, driver-census replies and queue
//! destruction. Caller provides order; native sorting and playback stay external.
use crate::{
    audio_activation::{Event as ActivationEvent, Pool, Prepared, Request},
    audio_census::{Event as CensusEvent, Frame, Params, Status},
    audio_source::Source,
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Handoff {
    pub pool: Pool,
    pub waiting: Vec<Source>,
    pub counter: u32,
    pub status: Status,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    Prepare {
        queue: usize,
        event: ActivationEvent,
    },
    Census(CensusEvent),
    Delete {
        queue: usize,
        unique_id: u32,
    },
}
#[derive(Clone, Debug)]
pub struct Cycle {
    pub prepared: Vec<Option<Prepared>>,
    pub events: Vec<Event>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot<T> {
    schema: u32,
    state: T,
}
impl Handoff {
    pub fn validate(&self) -> Result<(), String> {
        self.pool.validate()?;
        self.status.validate()?;
        let mut owned = std::collections::BTreeSet::new();
        for slot in &self.pool.slots {
            let node = slot.ok_or("Null handoff physical slot")?;
            if !owned.insert(node) || self.pool.transient == Some(node) {
                return Err("Aliased handoff physical ownership".into());
            }
        }

        if self.waiting.len() > 4096 {
            return Err("Handoff queue too large".into());
        }
        for s in &self.waiting {
            s.validate()?;
        }
        Ok(())
    }
    pub fn checkpoint(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        serde_json::to_vec(&Snapshot {
            schema: 1,
            state: self,
        })
        .map_err(|e| e.to_string())
    }
    pub fn load(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("Handoff checkpoint too large".into());
        }
        let saved: Snapshot<Self> = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if saved.schema != 1 {
            return Err("Unsupported handoff schema".into());
        }
        saved.state.validate()?;
        *self = saved.state;
        Ok(())
    }
    /// Preparation events are requests; Ready is not a claim of successful AL
    /// upload/play. Census replies represent that separate driver boundary.
    pub fn cycle(
        &mut self,
        order: &[usize],
        requests: &[Request],
        frames: &[Frame],
        params: &[Params],
    ) -> Result<Cycle, String> {
        self.validate()?;
        if order.len() != self.waiting.len() || requests.len() != self.waiting.len() {
            return Err("Handoff request/order mismatch".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        if order
            .iter()
            .any(|&i| i >= self.waiting.len() || !seen.insert(i))
        {
            return Err("Invalid handoff permutation".into());
        }
        let mut next = self.clone();
        let mut prepared = vec![];
        let mut events = vec![];
        for &i in order {
            let (result, trace) = next.pool.prepare(&next.waiting[i], requests[i])?;
            prepared.push(result);
            events.extend(
                trace
                    .into_iter()
                    .map(|event| Event::Prepare { queue: i, event }),
            );
        }
        let mut active = next
            .pool
            .slots
            .iter()
            .map(|i| {
                Ok(next.pool.nodes[i.ok_or("Null census source slot")?]
                    .source
                    .clone())
            })
            .collect::<Result<Vec<_>, String>>()?;
        let trace = next.status.recount(&mut active, frames, params)?;
        // Aliased physical slots are valid for individual native functions but
        // not for simultaneous staged census outputs: require an owned layout.
        let mut physical = std::collections::BTreeSet::new();
        for (slot, s) in next.pool.slots.iter().zip(active) {
            let i = slot.ok_or("Null census source slot")?;
            if !physical.insert(i) {
                return Err("Aliased handoff census slots".into());
            }
            next.pool.nodes[i].source = s;
        }
        events.extend(trace.into_iter().map(Event::Census));
        for (i, s) in next.waiting.iter().enumerate() {
            events.push(Event::Delete {
                queue: i,
                unique_id: s.unique_id,
            });
        }
        next.waiting.clear();
        next.validate()?;
        *self = next;
        Ok(Cycle { prepared, events })
    }
}
