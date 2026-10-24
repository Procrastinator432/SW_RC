//! AL PlaySource upload tail (100088fb..10008d6d). Produces ordered commands;
//! it does not open a device or upload PCM. Driver Play/Pause use base semantics.
use crate::audio_activation::{Event as PrefixEvent, Pool, Prepared, Request};
use crate::audio_source::Source;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Environment {
    pub category: u8,
    pub category_gain_bits: u32,
    pub editor: bool,
    pub eax_enabled: bool,
    pub eax_version: i32,
    pub listener_bits: [u32; 3],
    /// Actual RSQRTSS seed, required only for nonzero distance in flag 0x1000.
    pub rsqrt_seed_bits: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Buffer {
    pub handle: u32,
    pub buffers: Vec<u32>,
    pub counters: [u32; 2],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EaxCache {
    pub initialized: bool,
    pub value: i32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    Params {
        sound: u32,
    },
    Category {
        index: u8,
    },
    Float {
        hardware: u32,
        parameter: u32,
        value: u32,
    },
    Integer {
        hardware: u32,
        parameter: u32,
        value: u32,
    },
    Vector {
        hardware: u32,
        parameter: u32,
        value: [u32; 3],
    },
    Queue {
        hardware: u32,
        buffers: Vec<u32>,
    },
    Eax {
        version: u8,
        property: u32,
        hardware: u32,
        value: i32,
    },
    Check(String),
    Listener {
        index: i32,
    },
    Play {
        node: usize,
        lifetime_words: [u32; 2],
    },
    Pause {
        node: usize,
        request: i32,
        lifetime_words: [u32; 2],
    },
}
impl Environment {
    fn validate(&self) -> Result<(), String> {
        if !f32::from_bits(self.category_gain_bits).is_finite()
            || f32::from_bits(self.category_gain_bits).abs() > 1e6
            || self
                .listener_bits
                .iter()
                .any(|&b| !f32::from_bits(b).is_finite() || f32::from_bits(b).abs() > 1e6)
        {
            return Err("Invalid AL upload environment".into());
        }
        Ok(())
    }
}
impl Buffer {
    fn validate(&self) -> Result<(), String> {
        if self.handle == 0 || self.buffers.len() > 4096 {
            return Err("Invalid AL buffer description".into());
        }
        Ok(())
    }
}
pub(crate) fn attenuation(ratio: f32) -> Result<i32, String> {
    if !ratio.is_finite() {
        return Err("Nonfinite EAX radius ratio is outside upload contract".into());
    }
    // Original x87 ln(x) multiplied by its binary32 constant, then truncation.
    // Returned integer parity is checked against the original instructions.
    let value = f64::from(ratio.max(f32::from_bits(0x3727c5ac))).ln()
        * f64::from(f32::from_bits(0x445925b2));
    if !value.is_finite() || value < i32::MIN as f64 || value > i32::MAX as f64 {
        return Err("EAX attenuation out of range".into());
    }
    Ok(value as i32)
}
/// A fully staged source graph plus buffer progress and the native static EAX
/// cache. Separate snapshot schema from App/engine saves and audio_handoff.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub pool: Pool,
    pub buffers: Vec<Buffer>,
    pub eax_cache: EaxCache,
}
#[derive(Clone, Debug)]
pub struct Playback {
    pub prepared: Option<Prepared>,
    pub prefix: Vec<PrefixEvent>,
    pub events: Vec<Event>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot<T> {
    schema: u32,
    state: T,
}
impl Session {
    pub fn validate(&self) -> Result<(), String> {
        self.pool.validate()?;
        if self.buffers.len() > 4096 {
            return Err("AL buffer bank too large".into());
        }
        let mut owned = std::collections::BTreeSet::new();
        for slot in &self.pool.slots {
            let node = slot.ok_or("Null AL source slot")?;
            if !owned.insert(node) || self.pool.transient == Some(node) {
                return Err("Aliased AL source ownership".into());
            }
        }
        if self.buffers.iter().map(|b| b.buffers.len()).sum::<usize>() > 65536 {
            return Err("AL buffer bank exceeds total buffer limit".into());
        }
        let mut handles = std::collections::BTreeSet::new();
        for b in &self.buffers {
            b.validate()?;
            if !handles.insert(b.handle) {
                return Err("Duplicate AL buffer handle".into());
            }
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
            return Err("AL upload snapshot too large".into());
        }
        let s: Snapshot<Self> = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if s.schema != 1 {
            return Err("Unsupported AL upload schema".into());
        }
        s.state.validate()?;
        *self = s.state;
        Ok(())
    }
    /// Atomic pure planning. Check events are diagnostic callback boundaries,
    /// not proof that an external OpenAL command succeeded. No driver executed.
    pub fn play(
        &mut self,
        incoming: &Source,
        request: Request,
        environment: &Environment,
    ) -> Result<Playback, String> {
        self.validate()?;
        let mut next = self.clone();
        let (prepared, prefix) = next.pool.prepare(incoming, request)?;
        let events = if let Some(p) = &prepared {
            next.upload(p, environment)?
        } else {
            vec![]
        };
        next.validate()?;
        *self = next;
        Ok(Playback {
            prepared,
            prefix,
            events,
        })
    }
    fn upload(&mut self, prepared: &Prepared, env: &Environment) -> Result<Vec<Event>, String> {
        env.validate()?;
        let node = &mut self.pool.nodes[prepared.node];
        let s = &mut node.source;
        let buffer = self
            .buffers
            .iter_mut()
            .find(|b| b.handle == node.buffer)
            .ok_or("Missing AL buffer description")?;
        let hardware = node.hardware;
        let mut e = vec![
            Event::Params { sound: s.sound },
            Event::Category {
                index: env.category,
            },
        ];
        let gain = (f32::from_bits(s.output_volume_bits) * f32::from_bits(env.category_gain_bits))
            .clamp(f32::from_bits(0x38d1b717), f32::from_bits(0x3f7ff972));
        e.push(Event::Float {
            hardware,
            parameter: 0x100a,
            value: gain.to_bits(),
        });
        e.push(Event::Check("Gain".into()));
        let pitch = f32::from_bits(s.pitch_bits).clamp(0.7, 1.2);
        e.push(Event::Float {
            hardware,
            parameter: 0x1003,
            value: pitch.to_bits(),
        });
        e.push(Event::Check("Pitch".into()));
        e.push(Event::Integer {
            hardware,
            parameter: 0x1007,
            value: (s.flags >> 1) & 1,
        });
        e.push(Event::Check("Looping".into()));
        e.push(Event::Float {
            hardware,
            parameter: 0x1021,
            value: 1f32.to_bits(),
        });
        e.push(Event::Check("Rolloff".into()));
        let version = if env.eax_version < 3 { 2 } else { 3 };
        let eax_property = if version == 2 { 9 } else { 11 };
        if s.flags & 0x100 == 0 && !env.editor {
            if env.eax_enabled && s.flags & 0x800 == 0 {
                let ratio =
                    f32::from_bits(s.radii.radius_bits[1]) / f32::from_bits(s.base_radius_bits);
                e.push(Event::Eax {
                    version,
                    property: eax_property,
                    hardware,
                    value: attenuation(ratio)?,
                });
            }
            let position = s.location_bits.map(f32::from_bits);
            let scale = f32::from_bits(0x3c23d70a);
            let reference = if s.flags & 0x1000 == 0 {
                f32::from_bits(s.radii.radius_bits[1]) * scale
            } else {
                e.push(Event::Listener {
                    index: s.engine_index,
                });
                let [x, y, z] =
                    std::array::from_fn(|i| position[i] - f32::from_bits(env.listener_bits[i]));
                let q = (x * x + y * y) + z * z;
                let root = if q == 0. {
                    0.
                } else {
                    let seed = env
                        .rsqrt_seed_bits
                        .map(f32::from_bits)
                        .ok_or("AL distance needs explicit RSQRTSS seed")?;
                    if !seed.is_finite() || seed <= 0. {
                        return Err("Invalid AL distance seed".into());
                    }
                    ((3. - (seed * q) * seed) * (seed * 0.5)) * q
                };
                root * scale
            };
            e.push(Event::Vector {
                hardware,
                parameter: 0x1004,
                value: [
                    (position[0] * scale).to_bits(),
                    (position[2] * scale).to_bits(),
                    (position[1] * scale).to_bits(),
                ],
            });
            e.push(Event::Float {
                hardware,
                parameter: 0x1020,
                value: reference.to_bits(),
            });
            e.push(Event::Integer {
                hardware,
                parameter: 0x202,
                value: 0,
            });
        } else {
            e.push(Event::Integer {
                hardware,
                parameter: 0x202,
                value: 1,
            });
            e.push(Event::Vector {
                hardware,
                parameter: 0x1004,
                value: [0; 3],
            });
            e.push(Event::Float {
                hardware,
                parameter: 0x1020,
                value: 10f32.to_bits(),
            });
            if env.eax_enabled {
                if !self.eax_cache.initialized {
                    self.eax_cache.value = 0;
                    self.eax_cache.initialized = true;
                }
                e.push(Event::Eax {
                    version,
                    property: eax_property,
                    hardware,
                    value: self.eax_cache.value,
                });
            }
        }
        if env.eax_enabled {
            e.push(Event::Eax {
                version,
                property: 7,
                hardware,
                value: if s.flags & 0x18 != 0 { -10000 } else { 0 },
            });
        }
        e.push(Event::Check("Relative".into()));
        buffer.counters = [0; 2];
        e.push(Event::Integer {
            hardware,
            parameter: 0x1009,
            value: 0,
        });
        if buffer.buffers.len() == 1 {
            e.push(Event::Integer {
                hardware,
                parameter: 0x1009,
                value: buffer.buffers[0],
            });
        } else {
            e.push(Event::Queue {
                hardware,
                buffers: buffer.buffers.clone(),
            });
            e.push(Event::Integer {
                hardware,
                parameter: 0x1007,
                value: 0,
            });
        }
        e.push(Event::Play {
            node: prepared.node,
            lifetime_words: s.lifetime_words,
        });
        s.lifetime_words = [0; 2];
        if prepared.pause != 0 {
            e.push(Event::Pause {
                node: prepared.node,
                request: 1,
                lifetime_words: s.lifetime_words,
            });
            s.pause(1);
        }
        e.push(Event::Check("End".into()));
        Ok(e)
    }
}
