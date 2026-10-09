//! Read-only adapter for captured original x86 D3DDrv state; never dereferences
//! process pointers or runs the original engine. Supplied addresses are relocated.
use crate::shader_constants::{
    scene::{Scene, SqrtSeed},
    Host, Matrix,
};
use crate::shader_lights::{Light, Lighting};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Region {
    pub address: u32,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Debug)]
pub struct Memory {
    regions: Vec<Region>,
}
impl Memory {
    pub fn new(mut regions: Vec<Region>) -> Result<Self, String> {
        regions.sort_by_key(|r| r.address);
        let mut end = 0u64;
        for region in &regions {
            let next = region.address as u64 + region.bytes.len() as u64;
            if region.address == 0
                || region.bytes.is_empty()
                || next > 1u64 << 32
                || (region.address as u64) < end
            {
                return Err("Invalid, empty, overlapping or overflowing snapshot region".into());
            }
            end = next;
        }
        Ok(Self { regions })
    }
    /// A field may cross contiguous captured regions, but never an uncaptured gap.
    pub fn read(&self, address: u32, length: usize) -> Result<Vec<u8>, String> {
        let end = (address as u64)
            .checked_add(length as u64)
            .filter(|e| *e <= 1u64 << 32)
            .ok_or("Snapshot address range overflow")?;
        if address == 0 {
            return Err("Null snapshot read".into());
        }
        let mut cursor = address as u64;
        let mut out = Vec::new();
        while cursor < end {
            let r = self
                .regions
                .iter()
                .find(|r| {
                    r.address as u64 <= cursor && cursor < r.address as u64 + r.bytes.len() as u64
                })
                .ok_or_else(|| format!("Uncaptured snapshot address {cursor:08x}"))?;
            let offset = (cursor - r.address as u64) as usize;
            let n = (r.bytes.len() - offset).min((end - cursor) as usize);
            out.extend_from_slice(&r.bytes[offset..offset + n]);
            cursor += n as u64;
        }
        Ok(out)
    }
    fn at(base: u32, offset: u32) -> Result<u32, String> {
        if base == 0 {
            return Err("Null snapshot object".into());
        }
        base.checked_add(offset)
            .ok_or_else(|| "Snapshot field address overflow".into())
    }
    fn words<const N: usize>(&self, base: u32, offset: u32) -> Result<[u32; N], String> {
        let bytes = self.read(Self::at(base, offset)?, N * 4)?;
        Ok(std::array::from_fn(|i| {
            u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap())
        }))
    }
    fn word(&self, base: u32, offset: u32) -> Result<u32, String> {
        Ok(self.words::<1>(base, offset)?[0])
    }
    fn byte(&self, base: u32, offset: u32) -> Result<u8, String> {
        Ok(self.read(Self::at(base, offset)?, 1)?[0])
    }
    fn matrix(&self, base: u32, offset: u32) -> Result<Matrix, String> {
        let words = self.words::<16>(base, offset)?;
        Ok(std::array::from_fn(|r| {
            std::array::from_fn(|c| words[r * 4 + c])
        }))
    }
}
/// Addresses of resolved global VARIABLES, not import-table slots or DLL RVAs.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Globals {
    pub engine_time: u32,
    pub editor: u32,
    pub cubemap_manager: u32,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Addresses {
    pub renderer: u32,
    pub state: u32,
    pub viewport: u32,
    pub camera: u32,
    pub editor_actor: u32,
    pub cubemap_actor: u32,
}
/// Full eager snapshot for diagnostics. Requires every selected path's fields,
/// unlike the original per-constant lazy reads. Failure returns no partial Host.
pub fn capture(
    memory: &Memory,
    renderer: u32,
    globals: Globals,
    seed: Option<SqrtSeed>,
) -> Result<(Host, Addresses), String> {
    let state = memory.word(renderer, 0x9c0c)?;
    let viewport = memory.word(renderer, 8)?;
    let editor = memory.word(globals.editor, 0)? != 0;
    let engine_time = f32::from_bits(memory.word(globals.engine_time, 0)?);
    let manager = memory.word(globals.cubemap_manager, 0)?;
    let cubemap_actor = if manager == 0 {
        0
    } else {
        memory.word(manager, 0x2c)?
    };
    let (actor_draw_scale, alpha_gate) = if cubemap_actor == 0 {
        (None, false)
    } else {
        (
            Some(memory.words::<3>(cubemap_actor, 0x1b4)?),
            memory.word(cubemap_actor, 0x64)? & 0x2000 == 0,
        )
    };
    let camera = if editor {
        0
    } else {
        memory.word(viewport, 0x184)?
    };
    let editor_actor = if editor {
        memory.word(viewport, 0x30)?
    } else {
        0
    };
    let scene = Scene {
        actor_draw_scale,
        fog: memory.words::<2>(state, 0x2f0)?,
        editor_eye: if editor {
            memory.words::<3>(editor_actor, 0x138)?
        } else {
            [0; 3]
        },
        runtime_eye: if camera == 0 {
            None
        } else {
            Some(memory.words::<3>(camera, 0x194)?)
        },
        reciprocal_sqrt_seed: seed,
    };
    let mut slots = [None; 4];
    for (index, slot) in slots.iter_mut().enumerate() {
        let source = memory.word(state, 0x328 + index as u32 * 4)?;
        if source == 0 {
            continue;
        }
        let actor = memory.word(source, 0)?;
        *slot = Some(if actor == 0 {
            Light::default()
        } else {
            Light {
                actor_present: true,
                kind: memory.byte(actor, 0x2a)?,
                cone: memory.byte(actor, 0x39)?,
                color: memory.words::<4>(source, 8)?,
                position: memory.words::<3>(source, 0x18)?,
                direction: memory.words::<3>(source, 0x24)?,
                radius: memory.words::<2>(source, 0x30)?,
                flags: memory.words::<2>(source, 0x38)?,
                brightness: memory.word(source, 0x48)?,
            }
        });
    }
    let host = Host {
        scene,
        lighting: Lighting {
            slots,
            alpha_gate,
            ambient_bgra: memory.word(state, 0x148)?,
        },
        object_to_world: memory.matrix(state, 0x2c)?,
        world_to_camera: memory.matrix(state, 0x6c)?,
        projection: memory.matrix(state, 0xac)?,
        camera_to_world: if camera == 0 {
            None
        } else {
            Some(memory.matrix(camera, 0x54)?)
        },
        editor,
        engine_time,
    };
    Ok((
        host,
        Addresses {
            renderer,
            state,
            viewport,
            camera,
            editor_actor,
            cubemap_actor,
        },
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn regions_are_checked_sorted_and_contiguous_fields_are_readable() {
        let m = Memory::new(vec![
            Region {
                address: 0x102,
                bytes: vec![3, 4],
            },
            Region {
                address: 0x100,
                bytes: vec![1, 2],
            },
        ])
        .unwrap();
        assert_eq!(m.word(0x100, 0).unwrap(), 0x04030201);
        assert!(m.read(0, 1).is_err());
        assert!(m.read(0x100, 5).is_err());
        assert!(Memory::new(vec![
            Region {
                address: 0x100,
                bytes: vec![0; 4]
            },
            Region {
                address: 0x102,
                bytes: vec![0; 4]
            }
        ])
        .is_err());
        assert!(Memory::new(vec![Region {
            address: 0xffffffff,
            bytes: vec![0; 2]
        }])
        .is_err());
        assert!(Memory::new(vec![Region {
            address: 1,
            bytes: vec![]
        }])
        .is_err());
    }
    #[test]
    fn overflow_and_uncaptured_gaps_fail_without_pointer_access() {
        let m = Memory::new(vec![Region {
            address: 0xfffffffc,
            bytes: vec![0; 4],
        }])
        .unwrap();
        assert!(m.read(0xfffffffc, 5).is_err());
        assert!(m.word(0xfffffffc, 4).is_err());
        let gaps = Memory::new(vec![
            Region {
                address: 100,
                bytes: vec![0; 2],
            },
            Region {
                address: 103,
                bytes: vec![0; 2],
            },
        ])
        .unwrap();
        assert!(gaps.read(100, 4).is_err());
    }
}
