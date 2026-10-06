//! Read-only Republic Commando package tables. No engine or bytecode execution.
use serde::Serialize;
pub mod ai_contact;
pub mod auto_crouch;
pub mod classes;
pub mod collision;
pub mod controller;
pub mod crouch;
pub mod crouch_motion;
pub mod defaults;
pub mod event_lookup;
pub mod falling;
pub mod falling_collision;
pub mod geometry;
pub mod hit_wall;
pub mod hull;
pub mod level;
pub mod mesh;
pub mod mesh_collision;
pub mod mesh_query;
pub mod movement;
pub mod movement_profile;
pub mod notify_wall;
pub mod pawn_jump;
pub mod pawn_motion;
pub mod pawn_velocity;
pub mod physics;
pub mod properties;
pub mod script;
pub mod script_motion;
pub mod texture;
pub mod volumes;
pub mod walking_input;
pub mod world_collision;
type Result<T> = std::result::Result<T, String>;

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}
impl<'a> Reader<'a> {
    fn at(data: &'a [u8], pos: usize) -> Result<Self> {
        if pos > data.len() {
            return Err("offset outside file".into());
        }
        Ok(Self { data, pos })
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or("offset overflow")?;
        let out = self.data.get(self.pos..end).ok_or("truncated data")?;
        self.pos = end;
        Ok(out)
    }
    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32> {
        Ok(self.u32()? as i32)
    }
    fn f32(&mut self) -> Result<f32> {
        let f = f32::from_bits(self.u32()?);
        if !f.is_finite() {
            return Err(format!(
                "nonfinite coordinate at payload byte {}",
                self.pos - 4
            ));
        }
        Ok(f)
    }
    fn vector(&mut self) -> Result<[f32; 3]> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }
    fn count(&mut self, minimum: usize) -> Result<usize> {
        let n = usize::try_from(self.index()?).map_err(|_| "negative array count")?;
        if n > 1_000_000 || n > (self.data.len() - self.pos) / minimum {
            return Err("array count exceeds payload".into());
        }
        Ok(n)
    }
    fn index(&mut self) -> Result<i32> {
        let first = self.byte()?;
        let mut value = u64::from(first & 0x3f);
        let mut more = first & 0x40 != 0;
        let mut shift = 6;
        for i in 0..4 {
            if !more {
                break;
            }
            let b = self.byte()?;
            if i == 3 && b & 0xe0 != 0 {
                return Err("compact index overflow".into());
            }
            value |= u64::from(b & if i == 3 { 0x1f } else { 0x7f }) << shift;
            more = i != 3 && b & 0x80 != 0;
            shift += 7;
        }
        let signed = if first & 0x80 != 0 {
            -(value as i64)
        } else {
            value as i64
        };
        i32::try_from(signed).map_err(|_| "compact index overflow".into())
    }
    fn string(&mut self) -> Result<String> {
        self.string_limit(65536)
    }
    fn string_limit(&mut self, limit: u32) -> Result<String> {
        let n = self.index()?;
        if n == 0 {
            return Ok(String::new());
        }
        if n.unsigned_abs() > limit {
            return Err("name too long".into());
        }
        if n > 0 {
            let bytes = self.take(n as usize)?;
            if bytes.last() != Some(&0) {
                return Err("unterminated name".into());
            }
            // UE2 ANSI names; preserve each byte rather than silently replace it.
            Ok(bytes[..bytes.len() - 1]
                .iter()
                .map(|&c| char::from(c))
                .collect())
        } else {
            let bytes = self.take(n.unsigned_abs() as usize * 2)?;
            let words: Vec<_> = bytes
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect();
            if words.last() != Some(&0) {
                return Err("unterminated UTF16 name".into());
            }
            String::from_utf16(&words[..words.len() - 1]).map_err(|_| "invalid UTF16".into())
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub version: u16,
    pub licensee_version: u16,
    pub flags: u32,
    pub name_count: u32,
    pub name_offset: u32,
    pub export_count: u32,
    pub export_offset: u32,
    pub import_count: u32,
    pub import_offset: u32,
}
#[derive(Debug, Serialize)]
pub struct Import {
    pub class_package: String,
    pub class_name: String,
    pub outer: i32,
    pub name: String,
}
#[derive(Debug, Serialize)]
pub struct Export {
    pub class: i32,
    pub super_class: i32,
    pub outer: i32,
    pub extra: Option<i32>,
    pub name: String,
    pub flags: u32,
    pub serial_size: u32,
    pub serial_offset: u32,
}
#[derive(Debug, Serialize)]
pub struct Package {
    pub summary: Summary,
    pub names: Vec<String>,
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
}

impl Package {
    pub fn payload<'a>(&self, data: &'a [u8], export: &Export) -> Result<&'a [u8]> {
        let start = export.serial_offset as usize;
        let end = start
            .checked_add(export.serial_size as usize)
            .ok_or("payload overflow")?;
        data.get(start..end)
            .ok_or_else(|| "payload outside file".into())
    }
    pub fn name(&self, index: i32) -> Result<String> {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.names.get(i))
            .cloned()
            .ok_or_else(|| format!("invalid name index {index}"))
    }
    /// Recover original editor source when a Core.TextBuffer is present.
    /// TextBuffers with serialized properties are rejected rather than guessed.
    pub fn text_buffer(&self, data: &[u8], export: &Export) -> Result<String> {
        let start = export.serial_offset as usize;
        let end = start
            .checked_add(export.serial_size as usize)
            .ok_or("payload overflow")?;
        let payload = data.get(start..end).ok_or("truncated TextBuffer")?;
        let mut r = Reader::at(payload, 0)?;
        let name = r.index()?;
        if usize::try_from(name)
            .ok()
            .and_then(|i| self.names.get(i))
            .map(String::as_str)
            != Some("None")
        {
            return Err("TextBuffer with nonempty property list unsupported".into());
        }
        r.u32()?; // editor cursor position
        r.u32()?; // editor scroll position
        let text = r.string_limit(16 * 1024 * 1024)?;
        if r.pos != payload.len() {
            return Err("unexpected trailing TextBuffer data".into());
        }
        Ok(text)
    }
    /// Resolve signed UE object references. Cycles and excessive nesting are rejected.
    pub fn object_path(&self, mut index: i32) -> Result<String> {
        let mut seen = Vec::new();
        let mut parts = Vec::new();
        while index != 0 {
            if seen.contains(&index) || seen.len() >= 128 {
                return Err("cyclic or deeply nested object".into());
            }
            seen.push(index);
            if index > 0 {
                let e = self
                    .exports
                    .get(index as usize - 1)
                    .ok_or("invalid export reference")?;
                parts.push(e.name.clone());
                index = e.outer;
            } else {
                let i = self
                    .imports
                    .get(index.unsigned_abs() as usize - 1)
                    .ok_or("invalid import reference")?;
                parts.push(i.name.clone());
                index = i.outer;
            }
        }
        parts.reverse();
        Ok(parts.join("."))
    }
}

pub fn read_summary(data: &[u8]) -> Result<Summary> {
    let mut r = Reader::at(data, 0)?;
    if r.u32()? != 0x9e2a83c1 {
        return Err("not a little-endian Unreal package".into());
    }
    let v = r.u32()?;
    let s = Summary {
        version: v as u16,
        licensee_version: (v >> 16) as u16,
        flags: r.u32()?,
        name_count: r.u32()?,
        name_offset: r.u32()?,
        export_count: r.u32()?,
        export_offset: r.u32()?,
        import_count: r.u32()?,
        import_offset: r.u32()?,
    };
    // SWRC's export layout changed at versions 151 and 159. Other engines are unsupported.
    if !(99..=159).contains(&s.version) {
        return Err(format!("unsupported SWRC version {}", s.version));
    }
    Ok(s)
}

fn validate_table(data: &[u8], count: u32, offset: u32, minimum: usize) -> Result<()> {
    if count > 1_000_000 {
        return Err("table count exceeds limit".into());
    }
    let available = data
        .len()
        .checked_sub(offset as usize)
        .ok_or("table offset outside file")?;
    if count as usize > available / minimum {
        return Err("table cannot fit in file".into());
    }
    Ok(())
}

pub fn read_package(data: &[u8]) -> Result<Package> {
    let s = read_summary(data)?;
    validate_table(data, s.name_count, s.name_offset, 5)?;
    validate_table(data, s.import_count, s.import_offset, 7)?;
    validate_table(
        data,
        s.export_count,
        s.export_offset,
        if s.version >= 151 { 19 } else { 12 },
    )?;
    let mut r = Reader::at(data, s.name_offset as usize)?;
    let mut names = Vec::new();
    for _ in 0..s.name_count {
        names.push(r.string()?);
        r.u32()?;
    }
    let get_name = |index: i32| -> Result<String> {
        usize::try_from(index)
            .ok()
            .and_then(|i| names.get(i))
            .cloned()
            .ok_or(format!("invalid name index {index}"))
    };
    let mut r = Reader::at(data, s.import_offset as usize)?;
    let mut imports = Vec::new();
    for _ in 0..s.import_count {
        imports.push(Import {
            class_package: get_name(r.index()?)?,
            class_name: get_name(r.index()?)?,
            outer: r.i32()?,
            name: get_name(r.index()?)?,
        });
    }
    let mut r = Reader::at(data, s.export_offset as usize)?;
    let mut exports = Vec::new();
    for _ in 0..s.export_count {
        let class = r.index()?;
        let super_class = r.index()?;
        let outer = r.i32()?;
        let extra = if s.version >= 159 {
            Some(r.index()?)
        } else {
            None
        };
        let name = get_name(r.index()?)?;
        let flags = r.u32()?;
        let (serial_size, serial_offset) = if s.version >= 151 {
            (r.u32()?, r.u32()?)
        } else {
            let size = u32::try_from(r.index()?).map_err(|_| "negative serial size")?;
            let offset = if size == 0 {
                0
            } else {
                u32::try_from(r.index()?).map_err(|_| "negative serial offset")?
            };
            (size, offset)
        };
        if serial_size != 0 {
            let end = (serial_offset as usize)
                .checked_add(serial_size as usize)
                .ok_or("serial overflow")?;
            if end > data.len() {
                return Err(format!("export {name} payload outside file"));
            }
        }
        exports.push(Export {
            class,
            super_class,
            outer,
            extra,
            name,
            flags,
            serial_size,
            serial_offset,
        });
    }
    let valid_ref = |v: i32| {
        v == 0
            || if v > 0 {
                v as usize <= exports.len()
            } else {
                v.unsigned_abs() as usize <= imports.len()
            }
    };
    if imports.iter().any(|i| !valid_ref(i.outer))
        || exports
            .iter()
            .any(|e| !valid_ref(e.class) || !valid_ref(e.super_class) || !valid_ref(e.outer))
    {
        return Err("invalid object reference".into());
    }
    Ok(Package {
        summary: s,
        names,
        imports,
        exports,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_boundaries() {
        for (bytes, expected) in [
            (vec![0x3f], 63),
            (vec![0xbf], -63),
            (vec![0x40, 1], 64),
            (vec![0xc0, 1], -64),
            (vec![0x7f, 0xff, 0xff, 0xff, 0x0f], i32::MAX),
        ] {
            assert_eq!(Reader::at(&bytes, 0).unwrap().index().unwrap(), expected);
        }
        assert!(Reader::at(&[0x40], 0).unwrap().index().is_err());
        assert!(Reader::at(&[0x7f, 0xff, 0xff, 0xff, 0xff], 0)
            .unwrap()
            .index()
            .is_err());
    }
    #[test]
    fn rejects_truncation_and_bad_counts() {
        for n in 0..36 {
            assert!(read_package(&vec![0; n]).is_err());
        }
        let mut data = vec![0; 36];
        data[..4].copy_from_slice(&0x9e2a83c1u32.to_le_bytes());
        data[4..8].copy_from_slice(&159u32.to_le_bytes());
        data[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(read_package(&data).is_err());
    }
    #[test]
    fn names_ansi_and_utf16() {
        assert_eq!(
            Reader::at(&[4, b'A', b'B', b'C', 0], 0)
                .unwrap()
                .string()
                .unwrap(),
            "ABC"
        );
        assert_eq!(
            Reader::at(&[0x82, 0xa9, 3, 0, 0], 0)
                .unwrap()
                .string()
                .unwrap(),
            "Ω"
        );
        assert!(Reader::at(&[2, b'A', b'B'], 0).unwrap().string().is_err());
    }
    #[test]
    fn text_buffer_source_and_truncation() {
        let mut bytes = vec![0; 9];
        bytes.extend_from_slice(&[4, b'a', b'b', b'c', 0]);
        let p = Package {
            summary: Summary {
                version: 159,
                licensee_version: 1,
                flags: 0,
                name_count: 1,
                name_offset: 0,
                export_count: 0,
                export_offset: 0,
                import_count: 0,
                import_offset: 0,
            },
            names: vec!["None".into()],
            imports: vec![],
            exports: vec![],
        };
        let mut e = Export {
            class: 0,
            super_class: 0,
            outer: 0,
            extra: None,
            name: "ScriptText".into(),
            flags: 0,
            serial_size: bytes.len() as u32,
            serial_offset: 0,
        };
        assert_eq!(p.text_buffer(&bytes, &e).unwrap(), "abc");
        for n in 0..bytes.len() {
            e.serial_size = n as u32;
            assert!(p.text_buffer(&bytes[..n], &e).is_err());
        }
        e.serial_size = bytes.len() as u32;
        bytes[0] = 1;
        assert!(p.text_buffer(&bytes, &e).is_err());
    }
    #[test]
    fn object_path_detects_cycle() {
        let e = Export {
            class: 0,
            super_class: 0,
            outer: 1,
            extra: None,
            name: "Cycle".into(),
            flags: 0,
            serial_size: 0,
            serial_offset: 0,
        };
        let p = Package {
            summary: Summary {
                version: 159,
                licensee_version: 1,
                flags: 0,
                name_count: 0,
                name_offset: 0,
                export_count: 1,
                export_offset: 0,
                import_count: 0,
                import_offset: 0,
            },
            names: vec![],
            imports: vec![],
            exports: vec![e],
        };
        assert!(p.object_path(1).is_err());
        assert!(p.object_path(i32::MIN).is_err());
    }
}
