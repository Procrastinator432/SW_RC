//! Original UpdateBase gates/motion/order, with explicit driver/actor boundaries.
use crate::audio_source::{vector, Source};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseInput {
    /// Two virtual queries; neither is inferred from the stored Pause request.
    pub paused: [bool; 2],
    pub actor_valid: bool,
    pub engine_present: bool,
    pub listener_velocity: [f32; 3],
    pub actor_location: [f32; 3],
    pub actor_velocity: [f32; 3],
}
impl BaseInput {
    pub fn validate(&self) -> Result<(), String> {
        vector(self.listener_velocity)?;
        vector(self.actor_location)?;
        vector(self.actor_velocity)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BaseOutcome {
    pub updated: bool,
    pub counter_incremented: bool,
}
/// Side effects are intentionally ordered. Failed callbacks can leave earlier
/// source/host changes in place; use the staged Runtime for atomic pure updates.
pub trait UpdateHost {
    fn volume(&mut self, source: &mut Source, delta: f32) -> Result<(), String>;
    fn radii(&mut self, source: &mut Source, delta: f32) -> Result<(), String>;
    fn priority(&mut self, source: &mut Source, delta: f32) -> Result<(), String>;
}
pub fn update(
    source: &mut Source,
    input: &BaseInput,
    delta: f32,
    host: &mut impl UpdateHost,
) -> Result<BaseOutcome, String> {
    source.validate()?;
    input.validate()?;
    crate::audio_curve::time(delta)?;
    let mut out = BaseOutcome {
        updated: false,
        counter_incremented: false,
    };
    if !source.allocated || input.paused[0] {
        return Ok(out);
    }
    if source.actor != 0 && !input.actor_valid {
        source.actor = 0;
    }
    if !input.engine_present {
        return Ok(out);
    }
    out.updated = true;
    if source.identity == 0 || input.paused[1] {
        return Ok(out);
    }
    out.counter_incremented = source.flags & 4 == 0;
    if source.flags & 0x100 != 0 {
        source.location_bits = [0; 3];
        source.velocity_bits = [0; 3];
    } else if source.actor != 0 {
        source.location_bits = input.actor_location.map(f32::to_bits);
        source.velocity_bits = std::array::from_fn(|i| {
            (input.actor_velocity[i] - input.listener_velocity[i]).to_bits()
        });
    }
    source.validate()?;
    host.volume(source, delta)?;
    host.radii(source, delta)?;
    // Read AFTER callbacks: Volume may have stopped/cleared the source.
    if source.identity & 0xe != 6 {
        host.priority(source, delta)?;
    }
    source.validate()?;
    Ok(out)
}
