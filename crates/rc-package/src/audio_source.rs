//! Driver-independent FAbstractSource fields. Pointer-shaped values are opaque
//! caller handles, never dereferenced. This is not a native memory layout.
use crate::{
    audio_curve::{Curve, CurveMath},
    audio_fade::Fade,
    audio_geometry::SavedRadii,
    world_audio::Radii,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub allocated: bool,
    pub sound: u32,
    pub original_sound: u32,
    pub actor: u32,
    pub lifetime_words: [u32; 2],
    pub flags: u32,
    pub identity: u32,
    pub unique_id: u32,
    pub engine_index: i32,
    pub priority_bits: u32,
    pub cursor: u32,
    pub requested_pause: i32,
    pub voice_exempt: bool,
    pub fade: Fade,
    pub location_bits: [u32; 3],
    pub velocity_bits: [u32; 3],
    pub base_radius_bits: u32,
    pub radii: SavedRadii,
    pub reserved98: u32,
    pub volume_bits: u32,
    pub pitch_bits: u32,
    pub output_volume_bits: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Init {
    /// Resolved engine validity; original outer-object traversal stays external.
    pub engine_present: bool,
    pub sound: u32,
    pub original_sound: u32,
    pub actor: u32,
    pub flags: u32,
    pub identity: u32,
    pub engine_index: i32,
    pub location: [f32; 3],
    pub volume: f32,
    pub pitch: f32,
    pub initial_output: f32,
    pub fade_seconds: f32,
    pub fade_curve: u8,
    pub radius: f32,
    pub ambient_priority: f32,
    pub radius_caps: [f32; 3],
    pub voice_exempt: bool,
}
impl Source {
    /// Constructor on zero-initialized portable storage, not a claim that the
    /// native constructor initializes its untouched velocity/radius/time fields.
    pub fn new(counter: &mut u32) -> Self {
        *counter = counter.wrapping_add(1);
        Self {
            allocated: false,
            sound: 0,
            original_sound: 0,
            actor: 0,
            lifetime_words: [0; 2],
            flags: 0,
            identity: 0,
            unique_id: *counter,
            engine_index: 0,
            priority_bits: 0,
            cursor: 0,
            requested_pause: 0,
            voice_exempt: false,
            fade: Fade {
                curve: Curve {
                    words: [0; 6],
                    kind: 0,
                },
                duration_bits: 0,
                elapsed_bits: 0,
                mode: 0,
                held_bits: 1f32.to_bits(),
            },
            location_bits: [0; 3],
            velocity_bits: [0; 3],
            base_radius_bits: 0,
            radii: SavedRadii {
                radius_bits: [0; 3],
                changed_at_bits: 0,
            },
            reserved98: 0,
            volume_bits: 0,
            pitch_bits: 0,
            output_volume_bits: 0,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        self.fade.validate()?;
        vector(self.location_bits.map(f32::from_bits))?;
        vector(self.velocity_bits.map(f32::from_bits))?;
        let r = self.radii.restore()?;
        if [
            self.base_radius_bits,
            self.pitch_bits,
            self.volume_bits,
            self.priority_bits,
        ]
        .iter()
        .any(|&w| !finite(f32::from_bits(w), 1e6))
            || [r.priority_radius, r.current, r.target]
                .iter()
                .any(|&v| !finite(v, 1e6))
            || !finite(f32::from_bits(self.output_volume_bits), 1e12)
            || r.changed_at.abs() > 1e12
            || !f64::from_bits(
                u64::from(self.lifetime_words[0]) | (u64::from(self.lifetime_words[1]) << 32),
            )
            .is_finite()
        {
            return Err("Invalid saved abstract audio source".into());
        }
        Ok(())
    }
    pub fn slot(&self) -> u8 {
        ((self.identity >> 1) & 7) as u8
    }
    /// Init 103d2a40: reuses unique ID, pause and velocity; no random draws.
    pub fn initialize(&mut self, input: &Init, math: &mut impl CurveMath) -> Result<bool, String> {
        self.validate()?;
        input.validate()?;
        if !input.engine_present {
            return Ok(false);
        }
        let mut next = self.clone();
        next.sound = input.sound;
        next.original_sound = input.original_sound;
        next.actor = input.actor;
        next.flags = input.flags;
        next.identity = input.identity;
        next.engine_index = input.engine_index;
        next.priority_bits = if next.slot() == 3 {
            input.ambient_priority
        } else {
            -1.
        }
        .to_bits();
        next.cursor = 0;
        next.voice_exempt = input.voice_exempt;
        next.location_bits = if input.flags & 0x100 != 0 {
            [0; 3]
        } else {
            input.location.map(f32::to_bits)
        };
        next.fade.curve = Curve::new(0.)?;
        next.fade
            .curve
            .set(1., input.fade_seconds, input.fade_curve, math)?;
        next.fade.duration_bits = input.fade_seconds.to_bits();
        next.fade.elapsed_bits = 0;
        next.fade.mode = u8::from(input.fade_seconds > 0.);
        next.fade.held_bits = 1f32.to_bits();
        next.output_volume_bits = input.initial_output.to_bits();
        next.volume_bits = input.volume.to_bits();
        next.pitch_bits = input.pitch.to_bits();
        let mut radius = input.radius;
        match next.slot() {
            0 if input.radius_caps[0] > 1. && radius > input.radius_caps[0] => {
                radius = input.radius_caps[0]
            }
            // Native compares the ambient cap, then assigns the SLOT ZERO cap.
            3 if input.radius_caps[1] > 1. && radius > input.radius_caps[1] => {
                radius = input.radius_caps[0]
            }
            5 if input.radius_caps[2] > 1. && radius > input.radius_caps[2] => {
                radius = input.radius_caps[2]
            }
            _ => (),
        }
        if radius == 0. {
            radius = 10.;
        }
        next.base_radius_bits = radius.to_bits();
        next.radii = SavedRadii::capture(&Radii::initial(radius)?)?;
        next.lifetime_words = [0; 2];
        next.allocated = true;
        next.validate()?;
        *self = next;
        Ok(true)
    }
    /// Clear 10358eb0 deliberately retains curve/radii/position/unique identity.
    pub fn clear(&mut self) {
        self.priority_bits = 0;
        self.allocated = false;
        self.sound = 0;
        self.original_sound = 0;
        self.actor = 0;
        self.requested_pause = 0;
        self.cursor = 0;
        self.identity = 0;
        self.flags = 0;
        self.fade.held_bits = 1f32.to_bits();
    }
    /// Native base Pause stores the request. IsPaused is a separate virtual query.
    pub fn pause(&mut self, request: i32) -> bool {
        self.requested_pause = request;
        true
    }
    /// Base Stop with the default Clear implementation. Driver overrides remain external.
    pub fn stop(&mut self) -> bool {
        self.clear();
        true
    }
    pub fn matches(&self, actor: u32, sound: u32) -> bool {
        (actor == 0 || self.actor == actor)
            && (sound == 0 || self.sound == sound || self.original_sound == sound)
    }
}
impl Init {
    pub fn validate(&self) -> Result<(), String> {
        vector(self.location)?;
        crate::audio_curve::time(self.fade_seconds)?;
        if self.sound == 0
            || self.fade_curve > 3
            || !finite(self.volume, 16.)
            || !finite(self.pitch, 16.)
            || !finite(self.initial_output, 16.)
            || !finite(self.ambient_priority, 1e6)
            || !self.radius.is_finite()
            || !(0. ..=1e6).contains(&self.radius)
            || self
                .radius_caps
                .iter()
                .any(|v| !v.is_finite() || !(0. ..=1e6).contains(v))
        {
            return Err("Invalid abstract source initialization".into());
        }
        Ok(())
    }
}
pub(crate) fn finite(value: f32, limit: f32) -> bool {
    value.is_finite() && value.abs() <= limit
}
pub(crate) fn vector(v: [f32; 3]) -> Result<(), String> {
    if v.iter().any(|&v| !finite(v, 1e6)) {
        Err("Invalid abstract source vector".into())
    } else {
        Ok(())
    }
}
