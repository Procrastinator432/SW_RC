//! Finite, owner-thread native WAV loopback session for the Android ingress diagnostic.
//! The game-wide mixed source host is intentionally not replaced here.
use crate::{
    audio_device::{Backend, Executor, Kind, Op},
    audio_pcm::{Packet, MAX_FRAMES},
    audio_wave::Pcm,
};
pub const MAX_WAVE: usize = 2 * 1024 * 1024;
pub struct Live<B: Backend> {
    device: Executor<B>,
    epoch: u64,
    sequence: u64,
    frame_limit: usize,
    rendered: usize,
    fault: bool,
    rate: u32,
}
impl<B: Backend> Live<B> {
    pub fn open(backend: B, wave: &[u8], rate: u32, epoch: u64) -> Result<Self, String> {
        // Validate all caller data before creating physical sources.
        if wave.len() > MAX_WAVE
            || !(8000..=192000).contains(&rate)
            || epoch == 0
            || epoch > i64::MAX as u64
        {
            return Err("Invalid live WAV input/rate/epoch".into());
        }
        let pcm = Pcm::from_wave(wave)?;
        if pcm.bytes.is_empty() {
            return Err("Empty live WAV".into());
        }
        let mut device = Executor::new(backend, epoch)?;
        device.create(Kind::Source, 10)?;
        device.create(Kind::Buffer, 100)?;
        let program = device.prepare(&[
            Op::Upload {
                buffer: 100,
                format: crate::audio_wave::registration_format(pcm.info.channels, pcm.info.bits),
                frequency: pcm.info.frequency,
                bytes: pcm.bytes,
            },
            Op::Integer {
                source: 10,
                parameter: 0x1009,
                value: 100,
            },
            Op::Integer {
                source: 10,
                parameter: 0x1007,
                value: 1,
            },
            Op::Play(10),
        ])?;
        device.apply(&program).map_err(|error| error.reason)?;
        Ok(Self {
            device,
            epoch,
            sequence: 0,
            frame_limit: rate as usize * 5,
            rendered: 0,
            fault: false,
            rate,
        })
    }
    /// Call only after the output queue has reserved these exact frames.
    pub fn render(&mut self, frames: usize, epoch: u64, sequence: u64) -> Result<Vec<u8>, String> {
        if self.fault
            || frames == 0
            || frames > MAX_FRAMES
            || epoch != self.epoch
            || sequence != self.sequence
            || self
                .rendered
                .checked_add(frames)
                .is_none_or(|n| n > self.frame_limit)
            || self.sequence >= i64::MAX as u64
        {
            return Err("Invalid/stale live render request or diagnostic end".into());
        }
        let result = (|| {
            let rendered = self.device.render(frames)?;
            if rendered.frequency != self.rate {
                return Err("Live renderer returned wrong rate".into());
            }
            let bytes = Packet::from_rendered(rendered, epoch, sequence)?.encode()?;
            Ok::<_, String>(bytes)
        })();
        if result.is_err() {
            self.fault = true;
            return result;
        }
        self.rendered += frames;
        self.sequence += 1;
        result
    }
    pub fn rendered_frames(&self) -> usize {
        self.rendered
    }
    pub fn next_sequence(&self) -> u64 {
        self.sequence
    }
}
