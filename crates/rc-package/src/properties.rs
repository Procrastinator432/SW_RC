//! UE2 tagged properties. Unknown payloads remain explicit; no class-default inference.
use super::{Export, Package, Reader, Result};
use serde::Serialize;
use std::collections::HashMap;
#[derive(Clone, Debug, Serialize)]
pub struct Property {
    pub name: String,
    pub kind: u8,
    pub array_index: u32,
    pub struct_name: Option<String>,
    pub bytes: usize,
    pub declared_bytes: usize,
    pub payload_offset: usize,
    pub value: Value,
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum Value {
    Byte(u8),
    Int(i32),
    Bool(bool),
    Float(f32),
    Object { index: i32, path: String },
    Name(String),
    Vector([f32; 3]),
    Rotator([i32; 3]),
    String(String),
    StructArray(Vec<Properties>),
    Raw { prefix: Vec<u8> },
}
#[derive(Clone, Debug, Serialize)]
pub struct Properties {
    pub values: Vec<Property>,
    pub native_offset: usize,
}
/// Decode a complete nested tagged struct, rejecting trailing native bytes.
pub fn tagged_struct(pkg: &Package, payload: &[u8]) -> Result<Properties> {
    let mut reader = Reader::at(payload, 0)?;
    let result = read_reader(pkg, &mut reader)?;
    if reader.pos != payload.len() {
        return Err("Trailing bytes in tagged struct".into());
    }
    Ok(result)
}
pub fn read(pkg: &Package, data: &[u8], e: &Export) -> Result<Properties> {
    let mut r = Reader::at(pkg.payload(data, e)?, 0)?;
    if e.flags & 0x02000000 != 0 {
        let node = r.index()?;
        r.index()?;
        r.take(8)?;
        r.take(2)?; // FStateFrame LatentAction is a 16-bit word in SWRC
        if node != 0 {
            r.index()?;
        }
    }
    read_reader(pkg, &mut r)
}
pub(super) fn read_reader(pkg: &Package, r: &mut Reader<'_>) -> Result<Properties> {
    read_reader_with_arrays(pkg, r, &HashMap::new())
}
pub(super) fn read_reader_with_arrays(
    pkg: &Package,
    r: &mut Reader<'_>,
    arrays: &HashMap<String, String>,
) -> Result<Properties> {
    let mut values = Vec::new();
    loop {
        if values.len() >= 65536 {
            return Err("property count exceeds limit".into());
        }
        let name = pkg.name(r.index()?)?;
        if name == "None" {
            return Ok(Properties {
                values,
                native_offset: r.pos,
            });
        }
        let info = r.byte()?;
        let kind = info & 15;
        if kind == 0 {
            return Err(format!("invalid property type for {name}"));
        }
        let struct_name = if kind == 10 {
            Some(pkg.name(r.index()?)?)
        } else {
            None
        };
        let size = match (info >> 4) & 7 {
            0 => 1,
            1 => 2,
            2 => 4,
            3 => 12,
            4 => 16,
            5 => r.byte()? as usize,
            6 => u16::from_le_bytes(r.take(2)?.try_into().unwrap()) as usize,
            _ => r.u32()? as usize,
        };
        let mut array_index = 0;
        if info & 128 != 0 && kind != 3 {
            let b = r.byte()?;
            array_index = if b < 128 {
                b as u32
            } else if b & 64 == 0 {
                ((b as u32 & 63) << 8) | r.byte()? as u32
            } else {
                ((b as u32 & 63) << 24)
                    | ((r.byte()? as u32) << 16)
                    | ((r.byte()? as u32) << 8)
                    | r.byte()? as u32
            };
        }
        let payload_offset = r.pos;
        if kind == 9 {
            if let Some(element_type) = arrays.get(&name.to_lowercase()) {
                let count = r.count(1)?;
                if count > 4096 {
                    return Err("Struct array exceeds diagnostic limit".into());
                }
                let elements = (0..count)
                    .map(|_| read_reader(pkg, r))
                    .collect::<Result<Vec<_>>>()?;
                values.push(Property {
                    name,
                    kind,
                    array_index,
                    struct_name: Some(element_type.clone()),
                    bytes: r.pos - payload_offset,
                    declared_bytes: size,
                    payload_offset,
                    value: Value::StructArray(elements),
                });
                continue;
            }
        }
        let bytes = if kind == 3 { &[][..] } else { r.take(size)? };
        let mut v = Reader::at(bytes, 0)?;
        let value = match kind {
            1 if size == 1 => Value::Byte(v.byte()?),
            2 if size == 4 => Value::Int(v.i32()?),
            3 => Value::Bool(info & 128 != 0),
            4 if size == 4 => Value::Float(v.f32()?),
            5 | 8 => {
                let index = v.index()?;
                Value::Object {
                    index,
                    path: pkg.object_path(index)?,
                }
            }
            6 => Value::Name(pkg.name(v.index()?)?),
            11 => Value::Vector(v.vector()?),
            12 => Value::Rotator([v.i32()?, v.i32()?, v.i32()?]),
            10 if struct_name.as_deref() == Some("Vector") => Value::Vector(v.vector()?),
            10 if struct_name.as_deref() == Some("Rotator") => {
                Value::Rotator([v.i32()?, v.i32()?, v.i32()?])
            }
            13 => Value::String(v.string_limit(16 * 1024 * 1024)?),
            _ => {
                v.pos = bytes.len();
                Value::Raw {
                    prefix: bytes.iter().take(32).copied().collect(),
                }
            }
        };
        if v.pos != bytes.len() {
            return Err(format!(
                "property {name} consumed {} of {} bytes",
                v.pos,
                bytes.len()
            ));
        }
        values.push(Property {
            name,
            kind,
            array_index,
            struct_name,
            bytes: bytes.len(),
            declared_bytes: size,
            payload_offset,
            value,
        });
    }
}

/// Tagged structs in an array property, without discarding their payloads.
pub fn struct_array(pkg: &Package, payload: &[u8]) -> Result<Vec<Properties>> {
    let mut r = Reader::at(payload, 0)?;
    let count = r.count(1)?;
    if count > 4096 {
        return Err("Struct array exceeds diagnostic limit".into());
    }
    let result = (0..count)
        .map(|_| read_reader(pkg, &mut r))
        .collect::<Result<Vec<_>>>()?;
    if r.pos != payload.len() {
        return Err("Trailing struct array data".into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_typed_arrays_ignore_advisory_size_and_preserve_next_tag() {
        let pkg = package();
        let schema = HashMap::from([("values".into(), "Element".into())]);
        // The declared size excludes the element terminator, as in original Pawn.
        let data = [4, 0x59, 3, 1, 1, 0x83, 0, 1, 0x03, 0];
        let props =
            read_reader_with_arrays(&pkg, &mut Reader::at(&data, 0).unwrap(), &schema).unwrap();
        assert_eq!(props.native_offset, data.len());
        assert_eq!(props.values[0].declared_bytes, 3);
        assert_eq!(props.values[0].bytes, 4);
        let Value::StructArray(elements) = &props.values[0].value else {
            panic!("expected typed array")
        };
        assert_eq!(elements.len(), 1);
        assert!(matches!(elements[0].values[0].value, Value::Bool(true)));
        assert!(matches!(props.values[1].value, Value::Bool(false)));
        for end in 0..data.len() {
            assert!(read_reader_with_arrays(
                &pkg,
                &mut Reader::at(&data[..end], 0).unwrap(),
                &schema
            )
            .is_err());
        }
        // A size tag alone cannot activate the typed parser without metadata.
        assert!(read_reader(&pkg, &mut Reader::at(&data, 0).unwrap()).is_ok());
        let too_many = [4, 0x59, 0, 0x41, 0x40]; // compact count 4097
        assert!(
            read_reader_with_arrays(&pkg, &mut Reader::at(&too_many, 0).unwrap(), &schema).is_err()
        );
    }
    #[test]
    fn struct_arrays_keep_element_boundaries_and_reject_truncation() {
        let pkg = package();
        let data = [2, 1, 0x83, 0, 1, 0x03, 0];
        let elements = struct_array(&pkg, &data).unwrap();
        assert_eq!(elements.len(), 2);
        assert!(matches!(elements[0].values[0].value, Value::Bool(true)));
        assert!(matches!(elements[1].values[0].value, Value::Bool(false)));
        for len in 0..data.len() {
            assert!(struct_array(&pkg, &data[..len]).is_err());
        }
    }
    fn package() -> Package {
        Package {
            summary: super::super::Summary {
                version: 159,
                licensee_version: 1,
                flags: 0,
                name_count: 5,
                name_offset: 0,
                export_count: 0,
                export_offset: 0,
                import_count: 0,
                import_offset: 0,
            },
            names: vec![
                "None".into(),
                "Enabled".into(),
                "Location".into(),
                "Vector".into(),
                "Values".into(),
            ],
            imports: vec![],
            exports: vec![],
        }
    }
    #[test]
    fn bool_struct_and_array_tags_do_not_drift() {
        let p = package();
        let mut data = vec![1, 0x83, 2, 0x3a, 3];
        for f in [1.0f32, 2.0, 3.0] {
            data.extend_from_slice(&f.to_le_bytes());
        }
        data.extend_from_slice(&[4, 0xa2, 0x80, 0x80]);
        data.extend_from_slice(&123i32.to_le_bytes());
        data.push(0);
        let props = read_reader(&p, &mut Reader::at(&data, 0).unwrap()).unwrap();
        assert_eq!(props.native_offset, data.len());
        assert_eq!(props.values[2].array_index, 128);
        assert!(matches!(props.values[0].value, Value::Bool(true)));
        assert!(matches!(
            props.values[1].value,
            Value::Vector([1.0, 2.0, 3.0])
        ));
        for n in 0..data.len() {
            assert!(read_reader(&p, &mut Reader::at(&data[..n], 0).unwrap()).is_err());
        }
    }
    #[test]
    fn stack_latent_action_is_word_and_null_node_has_no_code_offset() {
        let p = package();
        let mut data = vec![0; 12];
        data.extend_from_slice(&[1, 0x83, 0]);
        let e = Export {
            class: 0,
            super_class: 0,
            outer: 0,
            extra: None,
            name: "Actor".into(),
            flags: 0x02000000,
            serial_size: data.len() as u32,
            serial_offset: 0,
        };
        assert_eq!(read(&p, &data, &e).unwrap().native_offset, 15);
    }
}
