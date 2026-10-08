//! Native HsHologram wrapper and ConstantColor value semantics.
use crate::properties::{Properties, Value};
use serde::{Deserialize, Serialize};
/// FColor is stored B,G,R,A; this word is ARGB without changing alpha.
pub fn stored_color(
    defaults: &Properties,
    instance: &Properties,
    name: &str,
) -> Result<u32, String> {
    let property = instance
        .values
        .iter()
        .find(|p| p.name == name && p.array_index == 0)
        .or_else(|| {
            defaults
                .values
                .iter()
                .find(|p| p.name == name && p.array_index == 0)
        });
    match property {
        None => Ok(0),
        Some(p) if p.struct_name.as_deref() == Some("Color") && p.bytes == 4 => match &p.value {
            Value::Raw { prefix } if prefix.len() == 4 => {
                Ok(u32::from_le_bytes(prefix.as_slice().try_into().unwrap()))
            }
            _ => Err(format!("Invalid Color payload for {name}")),
        },
        _ => Err(format!("Invalid Color property {name}")),
    }
}
/// UConstantColor::GetColor copies the stored word and ignores time completely.
pub fn constant_color(color: u32, _time: f32) -> u32 {
    color
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct HologramShader {
    pub color_words: [u32; 4],
    pub texture0: u32,
    pub texture1: u32,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct HologramWrapper {
    pub diffuse: u32,
    pub color: u32,
    pub use_marker_color: bool,
    pub fallback: u32,
}
/// Raw resource handles are supplied by the host, not package reference indices.
/// Reproduces normal-return writes, including Texture0 saved into Texture1.
pub fn setup_hologram(
    wrapper: &mut HologramWrapper,
    shader: Option<&mut HologramShader>,
    render: &mut dyn FnMut(&mut HologramShader, u32, u32) -> i32,
) -> i32 {
    let Some(shader) = shader else {
        return 0;
    };
    let saved_color = shader.color_words;
    let saved_texture0 = shader.texture0;
    if wrapper.diffuse != 0 {
        shader.texture1 = wrapper.diffuse;
    }
    if !wrapper.use_marker_color {
        let channel = |shift: u32| {
            (((wrapper.color >> shift) & 255u32) as f32 * f32::from_bits(0x3b808081)).to_bits()
        };
        shader.color_words = [channel(16), channel(8), channel(0), 1f32.to_bits()];
    }
    wrapper.fallback = shader.texture1;
    let result = render(shader, 0, 0);
    shader.texture1 = saved_texture0;
    if !wrapper.use_marker_color {
        shader.color_words = saved_color;
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (HologramWrapper, HologramShader) {
        (
            HologramWrapper {
                diffuse: 77,
                color: 0x32804020,
                use_marker_color: false,
                fallback: 99,
            },
            HologramShader {
                color_words: [0x7fc01234, 0x80000000, 9, 17],
                texture0: 11,
                texture1: 22,
            },
        )
    }
    #[test]
    fn missing_shader_does_not_call_host_or_write_wrapper() {
        let (mut w, _) = fixture();
        let before = w;
        assert_eq!(
            setup_hologram(&mut w, None, &mut |_, _, _| panic!("host")),
            0
        );
        assert_eq!(w, before);
    }
    #[test]
    fn override_tint_callback_arguments_return_and_restore_match_native() {
        let (mut w, mut s) = fixture();
        let original = s;
        let result = setup_hologram(&mut w, Some(&mut s), &mut |s, a, b| {
            assert_eq!([a, b], [0, 0]);
            assert_eq!(s.texture1, 77);
            assert_eq!(s.color_words[3], 1f32.to_bits());
            s.color_words = [0; 4];
            s.texture1 = 888;
            s.texture0 = 999;
            -7
        });
        assert_eq!(result, -7);
        assert_eq!(w.fallback, 77);
        assert_eq!(s.color_words, original.color_words);
        assert_eq!(s.texture1, 11);
        assert_eq!(s.texture0, 999);
    }
    #[test]
    fn null_diffuse_uses_existing_texture1_but_restores_texture0() {
        let (mut w, mut s) = fixture();
        w.diffuse = 0;
        setup_hologram(&mut w, Some(&mut s), &mut |s, _, _| {
            assert_eq!(s.texture1, 22);
            0
        });
        assert_eq!(w.fallback, 22);
        assert_eq!(s.texture1, 11);
    }
    #[test]
    fn marker_mode_preserves_host_color_writes() {
        let (mut w, mut s) = fixture();
        w.use_marker_color = true;
        let old = s.color_words;
        setup_hologram(&mut w, Some(&mut s), &mut |s, _, _| {
            assert_eq!(s.color_words, old);
            s.color_words = [123; 4];
            1
        });
        assert_eq!(s.color_words, [123; 4]);
    }
    #[test]
    fn hologram_alpha_is_one_even_when_stored_color_alpha_is_zero() {
        let (mut w, mut s) = fixture();
        w.color = 0x00ffffff;
        setup_hologram(&mut w, Some(&mut s), &mut |s, _, _| {
            assert_eq!(s.color_words, [1f32.to_bits(); 4]);
            1
        });
    }
    #[test]
    fn constant_color_retains_zero_alpha_and_ignores_nonfinite_time() {
        assert_eq!(constant_color(0x00282b24, f32::NAN), 0x00282b24);
        assert_eq!(constant_color(0x81234567, f32::INFINITY), 0x81234567);
    }
    fn props(color: Vec<u8>) -> Properties {
        Properties {
            native_offset: 0,
            values: vec![crate::properties::Property {
                name: "Color".into(),
                kind: 10,
                array_index: 0,
                struct_name: Some("Color".into()),
                bytes: color.len(),
                declared_bytes: color.len(),
                payload_offset: 0,
                value: Value::Raw { prefix: color },
            }],
        }
    }
    #[test]
    fn bgra_instance_overrides_defaults_without_opaque_alpha() {
        assert_eq!(
            stored_color(
                &props(vec![255; 4]),
                &props(vec![0x24, 0x2b, 0x28, 0]),
                "Color"
            )
            .unwrap(),
            0x00282b24
        );
    }
    #[test]
    fn malformed_override_is_not_replaced_by_class_color() {
        assert!(stored_color(&props(vec![255; 4]), &props(vec![1, 2, 3]), "Color").is_err());
    }
    #[test]
    fn absent_color_uses_zero_initialized_word() {
        let p = Properties {
            native_offset: 0,
            values: vec![],
        };
        assert_eq!(stored_color(&p, &p, "Color").unwrap(), 0);
    }
}
