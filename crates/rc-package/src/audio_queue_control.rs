//! Commands over queue slots and real static drivers. Child of queue_cycle.
use super::*;
use crate::{
    audio_commands as commands,
    audio_source_lookup::{Found, Query},
};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Command {
    PauseSound {
        actor: u32,
        sound: u32,
        request: i32,
    },
    PauseAll {
        request: i32,
        exclude: u32,
    },
    StopSound {
        actor: u32,
        sound: u32,
        fade_bits: u32,
    },
    StopAll {
        exclude: u32,
    },
    StopVoices {
        actor: u32,
    },
}
#[derive(Debug)]
pub struct CommandResult {
    pub matched: Option<bool>,
    pub events: Vec<Event>,
}
impl Host {
    fn managed(&self) -> commands::Sources {
        commands::Sources {
            active: self
                .playback
                .pool()
                .slots
                .iter()
                .map(|n| self.playback.pool().nodes[n.unwrap()].source.clone())
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
        self.playback.set_pending(mask);
    }
    pub fn refresh_cues(&mut self) -> Result<Vec<Event>, String> {
        Ok(self
            .playback
            .refresh_cues()?
            .into_iter()
            .map(Event::Playback)
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
    ) -> Result<CommandResult, String> {
        let before = self.saved()?;
        let result = self.dispatch_inner(command, bank, math).and_then(|out| {
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
    ) -> Result<CommandResult, String> {
        // Static ownership cannot execute a streamed command profile.
        if bank.iter().any(|s| s.streamed) {
            return Err("Static command host rejects streamed profiles".into());
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
            .playback
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
                        self.playback
                            .pause_node(slots[i], request)?
                            .into_iter()
                            .map(Event::Playback),
                    );
                }
                commands::Event::Stop {
                    target: Found::Active(i),
                    voice_exempt,
                } => {
                    let node = slots[i];
                    let mut source = self.playback.pool().nodes[node].source.clone();
                    source.voice_exempt = voice_exempt;
                    self.playback.replace_source(node, source)?;
                    events.extend(
                        self.playback
                            .stop_node(node)?
                            .into_iter()
                            .map(Event::Playback),
                    );
                    let mut source = self.playback.pool().nodes[node].source.clone();
                    source.voice_exempt = sources.active[i].voice_exempt;
                    self.playback.replace_source(node, source)?;
                }
                commands::Event::Fade {
                    target: Found::Active(i),
                    ..
                } => {
                    self.playback
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
        self.playback.register_pcm(wave, registration)
    }
    /// Rust ownership policy: delete matching pending objects before release.
    /// Not a claim about native UnregisterSound's handling of a pending queue.
    pub fn unregister(&mut self, sound: u32) -> Result<Vec<Event>, String> {
        let before = self.saved()?;
        let result = (|| {
            let mut events = vec![];
            for (i, q) in self.waiting.iter().enumerate() {
                if q.sound.handle == sound {
                    events.push(Event::Command(commands::Event::Delete {
                        target: Found::Queued(i),
                        unique_id: q.source.unique_id,
                    }));
                }
            }
            self.waiting.retain(|q| q.sound.handle != sound);
            events.extend(
                self.playback
                    .unregister(sound)?
                    .into_iter()
                    .map(Event::Resource),
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
