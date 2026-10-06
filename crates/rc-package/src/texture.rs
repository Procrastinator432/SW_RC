//! Original UE2/SWRC texture mips and BC1/BC2/BC3 decoding. No shader evaluation.
use super::{
    properties::{self, Value},
    Export, Package, Reader, Result,
};
pub struct Mip<'a> {
    pub width: usize,
    pub height: usize,
    pub data: &'a [u8],
}
pub struct Texture<'a> {
    pub format: u8,
    pub palette: Option<i32>,
    pub mips: Vec<Mip<'a>>,
}
pub fn read_texture<'a>(pkg: &Package, data: &'a [u8], e: &Export) -> Result<Texture<'a>> {
    if !(99..=159).contains(&pkg.summary.version) || pkg.summary.licensee_version > 1 {
        return Err("Unsupported SWRC texture version".into());
    }
    let props = properties::read(pkg, data, e)?;
    let format = props
        .values
        .iter()
        .find_map(|p| {
            if p.name == "Format" {
                if let Value::Byte(v) = p.value {
                    Some(v)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .unwrap_or(0);
    let palette = props.values.iter().find_map(|p| {
        if p.name == "Palette" {
            if let Value::Object { index, .. } = p.value {
                Some(index)
            } else {
                None
            }
        } else {
            None
        }
    });
    let payload = pkg.payload(data, e)?;
    let mut r = Reader::at(payload, props.native_offset)?;
    let count = r.count(15)?;
    if count > 32 {
        return Err("Too many texture mips".into());
    }
    let mut mips = Vec::new();
    for _ in 0..count {
        let end = r.u32()? as usize; // lazy byte-array absolute end, before dimensions
        let size = usize::try_from(r.index()?).map_err(|_| "Negative mip byte count")?;
        if size > 64 * 1024 * 1024 {
            return Err("Mip data exceeds limit".into());
        }
        let bytes = r.take(size)?;
        if end != e.serial_offset as usize + r.pos {
            return Err("Lazy mip end does not match data".into());
        }
        let width = r.u32()? as usize;
        let height = r.u32()? as usize;
        r.byte()?;
        r.byte()?; // UBits, VBits
        if width == 0 || height == 0 || width > 8192 || height > 8192 {
            return Err("Invalid mip dimensions".into());
        }
        mips.push(Mip {
            width,
            height,
            data: bytes,
        });
    }
    if r.pos != payload.len() {
        return Err("Uninterpreted texture tail".into());
    }
    Ok(Texture {
        format,
        palette,
        mips,
    })
}
pub fn read_palette(pkg: &Package, data: &[u8], e: &Export) -> Result<Vec<u32>> {
    let props = properties::read(pkg, data, e)?;
    let mut r = Reader::at(pkg.payload(data, e)?, props.native_offset)?;
    let count = r.count(4)?;
    if count != 256 {
        return Err("Palette requires 256 entries".into());
    }
    let result = (0..count)
        .map(|_| {
            let c = r.take(4)?;
            Ok(argb(c[0], c[1], c[2], c[3]))
        })
        .collect::<Result<_>>()?;
    if r.pos != r.data.len() {
        return Err("Palette has trailing data".into());
    }
    Ok(result)
}
fn argb(r: u8, g: u8, b: u8, a: u8) -> u32 {
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}
fn color565(c: u16) -> [u8; 3] {
    let r = ((c >> 11) & 31) as u8;
    let g = ((c >> 5) & 63) as u8;
    let b = (c & 31) as u8;
    [
        (r << 3) | (r >> 2),
        (g << 2) | (g >> 4),
        (b << 3) | (b >> 2),
    ]
}
/// Returns straight-alpha ARGB8888 pixels; caller decides alpha test/blending.
pub fn decode(format: u8, mip: &Mip<'_>, palette: Option<&[u32]>) -> Result<Vec<u32>> {
    let (w, h) = (mip.width, mip.height);
    if w == 0 || h == 0 || w > 8192 || h > 8192 || w * h > 16_777_216 {
        return Err("Texture dimensions exceed decode limit".into());
    }
    let raw = mip.data;
    let pixel_bytes = match format {
        0 | 9 => Some(1),
        4 => Some(3),
        5 => Some(4),
        _ => None,
    };
    if let Some(size) = pixel_bytes {
        if raw.len() != w * h * size {
            return Err("Texture byte count does not match dimensions".into());
        }
        return raw
            .chunks_exact(size)
            .map(|c| {
                Ok(match format {
                    0 => *palette
                        .and_then(|p| p.get(c[0] as usize))
                        .ok_or("Missing P8 palette")?,
                    4 => argb(c[2], c[1], c[0], 255),
                    5 => argb(c[2], c[1], c[0], c[3]),
                    9 => argb(c[0], c[0], c[0], 255),
                    _ => unreachable!(),
                })
            })
            .collect();
    }
    let stride = match format {
        3 => 8,
        7 | 8 => 16,
        _ => return Err(format!("Unsupported texture format {format}")),
    };
    let bw = w.div_ceil(4);
    let bh = h.div_ceil(4);
    if raw.len() != bw * bh * stride {
        return Err("DXT byte count does not match dimensions".into());
    }
    let mut pixels = vec![0; w * h];
    for (block, bytes) in raw.chunks_exact(stride).enumerate() {
        let color = &bytes[stride - 8..];
        let a = u16::from_le_bytes(color[0..2].try_into().unwrap());
        let b = u16::from_le_bytes(color[2..4].try_into().unwrap());
        let mut colors = [[0u8; 3]; 4];
        colors[0] = color565(a);
        colors[1] = color565(b);
        let opaque = format != 3 || a > b;
        colors[2] = std::array::from_fn(|i| {
            if opaque {
                ((2 * colors[0][i] as u16 + colors[1][i] as u16) / 3) as u8
            } else {
                ((colors[0][i] as u16 + colors[1][i] as u16) / 2) as u8
            }
        });
        if opaque {
            colors[3] = std::array::from_fn(|i| {
                ((colors[0][i] as u16 + 2 * colors[1][i] as u16) / 3) as u8
            });
        }
        let selectors = u32::from_le_bytes(color[4..8].try_into().unwrap());
        let mut alphas = [255u8; 16];
        if format == 7 {
            let bits = u64::from_le_bytes(bytes[..8].try_into().unwrap());
            for (i, alpha) in alphas.iter_mut().enumerate() {
                *alpha = ((bits >> (i * 4)) & 15) as u8 * 17;
            }
        }
        if format == 8 {
            let (a, b) = (bytes[0] as u16, bytes[1] as u16);
            let mut table = [0u8; 8];
            table[0] = a as u8;
            table[1] = b as u8;
            if a > b {
                for i in 1..=6 {
                    table[i + 1] = (((7 - i) as u16 * a + i as u16 * b) / 7) as u8;
                }
            } else {
                for i in 1..=4 {
                    table[i + 1] = (((5 - i) as u16 * a + i as u16 * b) / 5) as u8;
                }
                table[7] = 255;
            }
            let bits = bytes[2..8]
                .iter()
                .enumerate()
                .fold(0u64, |v, (i, &b)| v | ((b as u64) << (i * 8)));
            for (i, alpha) in alphas.iter_mut().enumerate() {
                *alpha = table[((bits >> (i * 3)) & 7) as usize];
            }
        }
        for (i, &alpha) in alphas.iter().enumerate() {
            let x = (block % bw) * 4 + i % 4;
            let y = (block / bw) * 4 + i / 4;
            if x >= w || y >= h {
                continue;
            }
            let index = ((selectors >> (i * 2)) & 3) as usize;
            let c = colors[index];
            let alpha = if !opaque && index == 3 { 0 } else { alpha };
            pixels[y * w + x] = argb(c[0], c[1], c[2], alpha);
        }
    }
    Ok(pixels)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mip_lazy_offsets_dimensions_and_every_truncation() {
        let pkg = Package {
            summary: super::super::Summary {
                version: 159,
                licensee_version: 1,
                flags: 0,
                name_count: 2,
                name_offset: 0,
                export_count: 0,
                export_offset: 0,
                import_count: 0,
                import_offset: 0,
            },
            names: vec!["None".into(), "Format".into()],
            imports: vec![],
            exports: vec![],
        };
        let mut payload = vec![1, 1, 3, 0, 1]; // Format=DXT1, None, one mip
        payload.extend_from_slice(&82u32.to_le_bytes());
        payload.push(8);
        payload.extend_from_slice(&[0, 0xf8, 0xe0, 7, 0, 0, 0, 0]);
        payload.extend_from_slice(&4u32.to_le_bytes());
        payload.extend_from_slice(&4u32.to_le_bytes());
        payload.extend_from_slice(&[2, 2]);
        let mut data = vec![0; 64];
        data.extend_from_slice(&payload);
        let mut e = Export {
            class: 0,
            super_class: 0,
            outer: 0,
            extra: None,
            name: "Test".into(),
            flags: 0,
            serial_size: payload.len() as u32,
            serial_offset: 64,
        };
        let tex = read_texture(&pkg, &data, &e).unwrap();
        assert_eq!(tex.format, 3);
        assert_eq!(tex.mips[0].width, 4);
        assert_eq!(decode(3, &tex.mips[0], None).unwrap()[0], 0xffff0000);
        for len in 0..payload.len() {
            e.serial_size = len as u32;
            assert!(read_texture(&pkg, &data[..64 + len], &e).is_err());
        }
        e.serial_size = payload.len() as u32;
        data[69] = 0;
        assert!(read_texture(&pkg, &data, &e).is_err());
    }
    #[test]
    fn dxt_modes_alpha_and_short_data() {
        let mut block = vec![0x00, 0xf8, 0xe0, 0x07, 0, 0, 0, 0]; // red, green; all index 0
        assert_eq!(
            decode(
                3,
                &Mip {
                    width: 4,
                    height: 4,
                    data: &block
                },
                None
            )
            .unwrap(),
            vec![0xffff0000; 16]
        );
        block[..4].copy_from_slice(&[0, 0, 255, 255]);
        block[4..].fill(255);
        assert_eq!(
            decode(
                3,
                &Mip {
                    width: 1,
                    height: 1,
                    data: &block
                },
                None
            )
            .unwrap(),
            vec![0]
        );
        let mut bc2 = vec![0x88; 8];
        bc2.extend_from_slice(&[0, 0xf8, 0xe0, 7, 0, 0, 0, 0]);
        assert_eq!(
            decode(
                7,
                &Mip {
                    width: 4,
                    height: 4,
                    data: &bc2
                },
                None
            )
            .unwrap()[0],
            0x88ff0000
        );
        let mut bc3 = vec![200, 20, 0, 0, 0, 0, 0, 0];
        bc3.extend_from_slice(&[0, 0xf8, 0xe0, 7, 0, 0, 0, 0]);
        assert_eq!(
            decode(
                8,
                &Mip {
                    width: 4,
                    height: 4,
                    data: &bc3
                },
                None
            )
            .unwrap()[0],
            0xc8ff0000
        );
        // BC3 interpolation selector 2, then the explicit opaque selector 7.
        bc3[2] = 2;
        assert_eq!(
            decode(
                8,
                &Mip {
                    width: 4,
                    height: 4,
                    data: &bc3
                },
                None
            )
            .unwrap()[0]
                >> 24,
            174
        );
        bc3[0] = 20;
        bc3[1] = 200;
        bc3[2] = 7;
        assert_eq!(
            decode(
                8,
                &Mip {
                    width: 4,
                    height: 4,
                    data: &bc3
                },
                None
            )
            .unwrap()[0]
                >> 24,
            255
        );
        let colors = [0, 0xf8, 0xe0, 7, 0xe4, 0, 0, 0];
        assert_eq!(
            &decode(
                3,
                &Mip {
                    width: 4,
                    height: 4,
                    data: &colors
                },
                None
            )
            .unwrap()[..4],
            &[0xffff0000, 0xff00ff00, 0xffaa5500, 0xff55aa00]
        );
        assert!(decode(
            3,
            &Mip {
                width: 4,
                height: 4,
                data: &block[..7]
            },
            None
        )
        .is_err());
    }
}
