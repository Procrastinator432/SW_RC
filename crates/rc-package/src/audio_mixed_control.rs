//! Commands over mixed queue slots and their sole resource owner.
use super::*;
use crate::{
    audio_commands as commands,
    audio_source_lookup::{Found, Query},
};
#[derive(Debug)]
pub struct CommandResult {
    pub matched: Option<bool>,
    pub events: Vec<Event>,
}
impl Host {
    fn managed(&self) -> commands::Sources {
        commands::Sources {
            active: self
                .mixed
                .pool()
                .slots
                .iter()
                .map(|n| self.mixed.pool().nodes[n.unwrap()].source.clone())
                .collect(),
            waiting: self.waiting.iter().map(|q| q.source.clone()).collect(),
            counter: self.counter,
            cue_id: self.cue_id,
        }
    }
    pub fn configure_reservation(
        &mut self,
        config: crate::audio_reservation::Config,
    ) -> Result<(), String> {
        config.validate()?;
        let old = self.reservation.replace(config);
        if let Err(e) = self.validate() {
            self.reservation = old;
            return Err(e);
        }
        Ok(())
    }
    pub fn cue_id(&self) -> u32 {
        self.cue_id
    }
    pub fn set_cue_id(&mut self, id: u32) {
        self.cue_id = id;
    }
    pub fn set_pending_cues(&mut self, mask: u32) {
        self.mixed.set_pending(mask);
    }
    pub fn refresh_cues(&mut self) -> Result<Vec<Event>, String> {
        Ok(self
            .mixed
            .refresh_cues()?
            .into_iter()
            .map(Event::Mixed)
            .collect())
    }
    pub fn is_sound_paused(&self, query: Query) -> Result<i32, String> {
        self.validate()?;
        let sources = self.managed();
        let active = sources
            .active
            .iter()
            .map(|s| s.requested_pause)
            .collect::<Vec<_>>();
        sources.is_sound_paused(query, &active, &vec![0; sources.waiting.len()])
    }
    pub fn dispatch(
        &mut self,
        command: &Command,
        bank: &[commands::Sound],
        math: &mut impl CurveMath,
        replies: &[driver::Reply],
    ) -> Result<CommandResult, String> {
        let before = self.saved()?;
        let result = self
            .dispatch_inner(command, bank, math, replies)
            .and_then(|out| {
                self.validate()?;
                Ok(out)
            });
        if result.is_err() {
            *self = Self::from_saved(before)?;
        }
        result
    }
    fn dispatch_inner(
        &mut self,
        command: &Command,
        bank: &[commands::Sound],
        math: &mut impl CurveMath,
        replies: &[driver::Reply],
    ) -> Result<CommandResult, String> {
        if replies.len() != self.mixed.pool().nodes.len() {
            return Err("Mixed command reply shape mismatch".into());
        }
        for p in bank {
            let kinds = self
                .waiting
                .iter()
                .filter(|q| q.content.sound().handle == p.handle)
                .map(|q| matches!(q.content, Content::Stream { .. }))
                .chain(
                    self.mixed
                        .pool()
                        .nodes
                        .iter()
                        .filter(|n| n.source.allocated && n.source.sound == p.handle)
                        .map(|n| n.source.flags & 4 != 0),
                );
            if kinds.into_iter().any(|streamed| streamed != p.streamed) {
                return Err("Command profile disagrees with owned source kind".into());
            }
        }
        let mut sources = self.managed();
        let (matched, plan) = match *command {
            Command::PauseSound {
                actor,
                sound,
                request,
            } => (None, sources.pause_sound(actor, sound, request)?),
            Command::PauseAll { request, exclude } => (None, sources.pause_all(request, exclude)?),
            Command::StopSound {
                actor,
                sound,
                fade_bits,
            } => {
                let (matched, events) =
                    sources.stop_sound(actor, sound, f32::from_bits(fade_bits), bank, math)?;
                (Some(matched), events)
            }
            Command::StopAll { exclude } => (None, sources.stop_all(exclude)?),
            Command::StopVoices { actor } => (None, sources.stop_voices(actor, bank)?),
        };
        let slots = self
            .mixed
            .pool()
            .slots
            .iter()
            .map(|n| n.unwrap())
            .collect::<Vec<_>>();
        let mut events = vec![];
        for event in plan {
            events.push(Event::Command(event.clone()));
            match event {
                commands::Event::Pause {
                    target: Found::Active(i),
                    request,
                } => {
                    events.extend(
                        self.mixed
                            .control(
                                slots[i],
                                &driver::Command::Pause(request),
                                &replies[slots[i]],
                            )?
                            .into_iter()
                            .map(Event::Mixed),
                    );
                }
                commands::Event::Stop {
                    target: Found::Active(i),
                    voice_exempt,
                } => {
                    let node = slots[i];
                    let mut source = self.mixed.pool().nodes[node].source.clone();
                    source.voice_exempt = voice_exempt;
                    self.mixed.replace_source(node, source)?;
                    events.extend(
                        self.mixed
                            .control(node, &driver::Command::Stop, &replies[node])?
                            .into_iter()
                            .map(Event::Mixed),
                    );
                    let mut source = self.mixed.pool().nodes[node].source.clone();
                    source.voice_exempt = sources.active[i].voice_exempt;
                    self.mixed.replace_source(node, source)?;
                }
                commands::Event::Fade {
                    target: Found::Active(i),
                    ..
                } => {
                    self.mixed
                        .replace_source(slots[i], sources.active[i].clone())?;
                }
                _ => {}
            }
        }
        for (q, source) in self.waiting.iter_mut().zip(sources.waiting) {
            q.source = source;
        }
        Ok(CommandResult { matched, events })
    }
    pub fn empty_waiting(&mut self) -> Result<Vec<Event>, String> {
        let mut sources = self.managed();
        let events = sources.empty_waiting()?;
        self.waiting.clear();
        Ok(events.into_iter().map(Event::Command).collect())
    }
    pub fn register_pcm(
        &mut self,
        wave: &[u8],
        registration: &crate::audio_buffer_bank::Registration,
    ) -> Result<crate::audio_resource_host::Registered, String> {
        self.transaction(|h| h.mixed.register_pcm(wave, registration))
    }
    /// Rust ownership policy: delete matching pending objects before release.
    /// Not a claim about native UnregisterSound's handling of a pending queue.
    pub fn unregister(
        &mut self,
        sound: u32,
        replies: &[driver::Reply],
    ) -> Result<Vec<Event>, String> {
        let before = self.saved()?;
        let result = (|| {
            let mut events = vec![];
            for (i, q) in self.waiting.iter().enumerate() {
                if q.content.sound().handle == sound {
                    events.push(Event::Command(commands::Event::Delete {
                        target: Found::Queued(i),
                        unique_id: q.source.unique_id,
                    }));
                }
            }
            self.waiting.retain(|q| q.content.sound().handle != sound);
            events.extend(
                self.mixed
                    .unregister(sound, replies)?
                    .into_iter()
                    .map(Event::Mixed),
            );
            if let Some(config) = &mut self.reservation {
                config.names.retain(|n| n.sound != sound);
            }
            self.validate()?;
            Ok(events)
        })();
        if result.is_err() {
            *self = Self::from_saved(before)?;
        }
        result
    }
}
