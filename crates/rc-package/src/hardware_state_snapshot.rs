//! Eager, safe original-layout capture for material and post-resource sampler state.
use crate::{
    hardware_material::{Host, Shader},
    hardware_sampler::{Resource, Texture},
    shader_snapshot::Memory,
};
fn at(base: u32, offset: u32) -> Result<u32, String> {
    if base == 0 {
        return Err("Null hardware state pointer".into());
    }
    base.checked_add(offset)
        .ok_or_else(|| "Hardware state address overflow".into())
}
fn byte(m: &Memory, base: u32, offset: u32) -> Result<u8, String> {
    Ok(m.read(at(base, offset)?, 1)?[0])
}
fn word(m: &Memory, base: u32, offset: u32) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        m.read(at(base, offset)?, 4)?.try_into().unwrap(),
    ))
}
/// Resolved stable addresses only; no constructor/default inference or live process reads.
pub fn material(
    m: &Memory,
    renderer: u32,
    shader: u32,
) -> Result<(Host, Shader, [u32; 2]), String> {
    let device = word(m, renderer, 4)?;
    let state = word(m, renderer, 0x9c0c)?;
    let pass = word(m, state, 0x304)?;
    Ok((
        Host {
            renderer_flags: word(m, renderer, 0x9fd0)?,
            pass: m.read(at(pass, 0)?, 28)?.try_into().unwrap(),
            active_passes: byte(m, state, 0x324)?,
        },
        Shader {
            flags: word(m, shader, 0x92c)?,
            alpha_reference: byte(m, shader, 0x930)?,
            source_blend: byte(m, shader, 0x931)?,
            destination_blend: byte(m, shader, 0x932)?,
            fallback: word(m, shader, 0x28)?,
        },
        [word(m, device, 0x4218)?, word(m, device, 0x4220)?],
    ))
}
/// Resource is the result of the external 1001a420 cache lookup, supplied explicitly.
/// Null texture does not read either renderer/device or resource memory.
pub fn sampler(
    m: &Memory,
    renderer: u32,
    texture: u32,
    resource: u32,
) -> Result<(Option<Texture>, Resource, u32), String> {
    if texture == 0 {
        return Ok((
            None,
            Resource {
                address: 0,
                word_3c: 0,
            },
            0,
        ));
    }
    let t = Texture {
        address_u: byte(m, texture, 0x65)?,
        address_v: byte(m, texture, 0x66)?,
        flags_7c: byte(m, texture, 0x7c)?,
    };
    let r = Resource {
        address: resource,
        word_3c: if resource == 0 {
            0
        } else {
            word(m, resource, 0x3c)?
        },
    };
    let device = word(m, renderer, 4)?;
    Ok((Some(t), r, word(m, device, 0x466c)?))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::shader_snapshot::Region;
    #[test]
    fn null_sampler_is_lazy_and_material_overflow_is_rejected() {
        let m = Memory::new(vec![]).unwrap();
        assert!(sampler(&m, u32::MAX, 0, u32::MAX).unwrap().0.is_none());
        assert!(material(&m, u32::MAX, 1).is_err());
        assert!(material(&m, 0, 1).is_err());
    }
    #[test]
    fn truncated_nonnull_resource_is_not_silently_treated_as_null() {
        let m = Memory::new(vec![
            Region {
                address: 0x1065,
                bytes: vec![0, 1],
            },
            Region {
                address: 0x107c,
                bytes: vec![1],
            },
        ])
        .unwrap();
        assert!(sampler(&m, 1, 0x1000, 0x2000).is_err());
    }
}
