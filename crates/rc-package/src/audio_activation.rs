//! OpenAL preparation before parameter upload/playback. Opaque node indices
//! retain physical channels while native base-source assignment copies identity.
use crate::{
    audio_pool::{self, Incoming, Slot},
    audio_source::Source,
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub source: Source,
    pub hardware: u32,
    pub buffer: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pool {
    pub nodes: Vec<Node>,
    pub slots: Vec<Option<usize>>,
    pub transient: Option<usize>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub engine_present: bool,
    pub sound_valid: bool,
    pub buffer: u32,
    pub queued: i32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    SoundValid,
    Precache,
    ClearError,
    IsQueued,
    Copy(usize),
    Stop(usize),
    Clear(usize),
    Ready { node: usize, pause: i32 },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prepared {
    pub node: usize,
    pub pause: i32,
}
pub fn copy_source(destination: &mut Source, source: &Source) -> Result<(), String> {
    destination.validate()?;
    source.validate()?;
    *destination = source.clone();
    Ok(())
}
impl Pool {
    pub fn validate(&self) -> Result<(), String> {
        if self.nodes.len() > 4097
            || self.slots.len() > 4096
            || self
                .slots
                .iter()
                .flatten()
                .chain(self.transient.iter())
                .any(|&i| i >= self.nodes.len())
        {
            return Err("Invalid audio physical source graph".into());
        }
        for node in &self.nodes {
            node.source.validate()?;
        }
        Ok(())
    }
    /// Whole native SetTransientSource: replace the FIRST matching slot with the
    /// previous transient, then stop the supplied physical node, even if absent.
    pub fn set_transient(&mut self, node: usize) -> Result<Vec<Event>, String> {
        self.validate()?;
        if node >= self.nodes.len() {
            return Err("Unknown transient audio node".into());
        }
        if let Some(i) = self.slots.iter().position(|s| *s == Some(node)) {
            self.slots[i] = self.transient;
        }
        self.transient = Some(node);
        self.nodes[node].source.stop();
        Ok(vec![Event::Stop(node)])
    }
    /// Driver IsFinished (+18), not the allocated marker or IsPlaying.
    pub fn available(&self, finished: &[i32]) -> Result<Option<usize>, String> {
        self.validate()?;
        if finished.len() != self.slots.len() {
            return Err("Finished reply/source mismatch".into());
        }
        for (slot, reply) in self.slots.iter().zip(finished) {
            let node = slot.ok_or("Null physical source slot")?;
            if *reply != 0 {
                return Ok(Some(node));
            }
        }
        Ok(None)
    }
    /// Selected PlaySource prefix: success stops before gain/pitch/EAX/buffer
    /// upload and Play. Buffer precaching and validity are caller replies.
    pub fn prepare(
        &mut self,
        incoming: &Source,
        request: Request,
    ) -> Result<(Option<Prepared>, Vec<Event>), String> {
        self.validate()?;
        incoming.validate()?;
        let mut events = vec![];
        if !request.engine_present || incoming.sound == 0 {
            return Ok((None, events));
        }
        events.push(Event::SoundValid);
        if !request.sound_valid {
            return Ok((None, events));
        }
        events.push(Event::Precache);
        if request.buffer == 0 {
            return Ok((None, events));
        }
        events.extend([Event::ClearError, Event::IsQueued]);
        let Some(node) = self.transient else {
            return Ok((None, events));
        };
        let mut next = self.clone();
        let pause = if request.queued != 0 {
            incoming.requested_pause
        } else {
            0
        };
        copy_source(&mut next.nodes[node].source, incoming)?;
        events.push(Event::Copy(node));
        if next.nodes[node].source.flags & 0x100 != 0 {
            next.nodes[node].source.location_bits = [0; 3];
        }
        next.nodes[node].buffer = request.buffer;
        let slots: Vec<_> = next
            .slots
            .iter()
            .map(|index| {
                let s = &next.nodes[index.ok_or("Null physical source slot")?].source;
                Ok(Slot {
                    occupied: s.allocated,
                    identity: s.identity,
                    priority: f32::from_bits(s.priority_bits),
                })
            })
            .collect::<Result<_, String>>()?;
        let choice = audio_pool::select(
            Incoming {
                identity: incoming.identity,
                priority: f32::from_bits(incoming.priority_bits),
            },
            &slots,
        )?;
        let prepared = if let Some(i) = choice {
            let selected = next.slots[i].ok_or("Null physical source slot")?;
            events.push(Event::Stop(selected));
            next.nodes[selected].source.stop();
            events.extend(next.set_transient(selected)?);
            events.push(Event::Ready { node, pause });
            Some(Prepared { node, pause })
        } else {
            next.nodes[node].source.clear();
            events.push(Event::Clear(node));
            None
        };
        next.validate()?;
        *self = next;
        Ok((prepared, events))
    }
}
