//! D3DDrv 1000fff0: capability gates, setup callback and packed pass-state handoff.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Host {
    pub renderer_flags: u32,
    pub pass: [u8; 28],
    pub active_passes: u8,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Shader {
    pub flags: u32,
    pub alpha_reference: u8,
    pub source_blend: u8,
    pub destination_blend: u8,
    pub fallback: u32,
}
/// Callback abstracts 1000dbe0; counts other than -1 are narrowed to AL as in x86.
/// Safe callback errors retain host mutations. No native pointer dereference/GPU call.
pub fn set(
    host: &mut Host,
    shader: Shader,
    capabilities: [u32; 2],
    mut diagnostic: Option<&mut String>,
    fallback: Option<&mut u32>,
    setup: &mut dyn FnMut(&mut Host) -> Result<i32, String>,
) -> Result<bool, String> {
    let unsupported = if capabilities[0] == 0 {
        Some("No vertex shader support detected")
    } else if capabilities[1] == 0 {
        Some("No pixel shader support detected")
    } else {
        None
    };
    if let Some(message) = unsupported {
        if let Some(text) = diagnostic {
            *text = message.into();
        }
        if let Some(value) = fallback {
            *value = shader.fallback;
        }
        return Ok(false);
    }
    host.renderer_flags |= 0x20;
    let count = setup(host)?;
    if count == -1 {
        if let Some(text) = diagnostic.as_mut() {
            text.push_str("  Failed to set hardware shader.");
        }
        if let Some(value) = fallback {
            *value = shader.fallback;
        }
        return Ok(false);
    }
    let previous = u32::from_le_bytes(host.pass[4..8].try_into().unwrap());
    let flags = (previous & !0x1f) | ((shader.flags >> 2) & 7) | ((shader.flags << 3) & 0x18);
    host.pass[4..8].copy_from_slice(&flags.to_le_bytes());
    host.pass[8] = shader.alpha_reference;
    host.pass[9] = count as u8;
    host.pass[20..24].copy_from_slice(&u32::from(shader.source_blend).to_le_bytes());
    host.pass[24..28].copy_from_slice(&u32::from(shader.destination_blend).to_le_bytes());
    host.active_passes = 1;
    Ok(true)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn host() -> Host {
        Host {
            renderer_flags: 0x8000,
            pass: [0xa5; 28],
            active_passes: 9,
        }
    }
    fn shader() -> Shader {
        Shader {
            flags: 0x15,
            alpha_reference: 73,
            source_blend: 5,
            destination_blend: 2,
            fallback: 77,
        }
    }
    #[test]
    fn capability_gates_replace_diagnostic_and_skip_callback() {
        for (caps, message) in [
            ([0, 1], "No vertex shader support detected"),
            ([1, 0], "No pixel shader support detected"),
        ] {
            let mut h = host();
            let before = h.clone();
            let mut text = "previous".into();
            let mut fallback = 99;
            assert!(!set(
                &mut h,
                shader(),
                caps,
                Some(&mut text),
                Some(&mut fallback),
                &mut |_| panic!()
            )
            .unwrap());
            assert_eq!(h, before);
            assert_eq!(text, message);
            assert_eq!(fallback, 77);
        }
    }
    #[test]
    fn failure_keeps_callback_writes_and_success_preserves_unrelated_bytes() {
        let mut h = host();
        let mut text = String::from("previous");
        let mut fallback = 99;
        assert!(!set(
            &mut h,
            shader(),
            [1, 1],
            Some(&mut text),
            Some(&mut fallback),
            &mut |h| {
                h.pass[0] = 7;
                Ok(-1)
            }
        )
        .unwrap());
        assert_eq!(h.renderer_flags, 0x8020);
        assert_eq!(h.pass[0], 7);
        assert_eq!(h.active_passes, 9);
        assert_eq!(text, "previous  Failed to set hardware shader.");
        assert_eq!(fallback, 77);
        assert!(set(
            &mut h,
            shader(),
            [u32::MAX, 1],
            Some(&mut text),
            Some(&mut fallback),
            &mut |_| Ok(-2)
        )
        .unwrap());
        assert_eq!(h.pass[9], 254);
        assert_eq!(h.pass[8], 73);
        assert_eq!(h.pass[10], 0xa5);
        assert_eq!(h.pass[0], 7);
        assert_eq!(h.active_passes, 1);
        assert_eq!(fallback, 77);
        assert_eq!(
            u32::from_le_bytes(h.pass[4..8].try_into().unwrap()) & 31,
            13
        );
        let before = h.clone();
        assert!(set(&mut h, shader(), [1, 1], None, None, &mut |host| {
            host.pass[1] = 19;
            Err("host setup failure".into())
        })
        .is_err());
        assert_eq!(h.pass[1], 19);
        assert_eq!(h.pass[8..], before.pass[8..]);
    }
}
