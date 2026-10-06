//! Serialized UClass default deltas; no constructor/config/default-object execution.
use super::{properties, Export, Package, Reader, Result};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Serialize)]
pub struct ClassDefaults {
    pub class_flags: u32,
    pub config_name: String,
    pub defaults_offset: usize,
    pub properties: properties::Properties,
}

#[derive(Clone, Debug, Serialize)]
pub struct Field {
    pub name: String,
    pub kind: String,
    pub dimension: u32,
    pub flags: u32,
    pub struct_name: Option<String>,
}

/// Definitions owned by this class, including flags needed to distinguish
/// config/localized properties from immutable serialized/zero defaults.
pub fn fields(pkg: &Package, data: &[u8], class: &Export) -> Result<Vec<Field>> {
    let Some(owner) = pkg.exports.iter().position(|e| {
        e.name == class.name && e.outer == class.outer && e.serial_offset == class.serial_offset
    }) else {
        return Err("class definition not present in package".into());
    };
    let mut fields = Vec::new();
    for export in &pkg.exports {
        if export.outer != owner as i32 + 1 {
            continue;
        }
        let kind = pkg.object_path(export.class)?;
        if !kind.starts_with("Core.") || !kind.ends_with("Property") {
            continue;
        }
        let payload = pkg.payload(data, export)?;
        let props = properties::read(pkg, data, export)?;
        let mut r = Reader::at(payload, props.native_offset)?;
        if pkg.summary.version < 137 {
            pkg.object_path(r.index()?)?;
        }
        let dimension = if pkg.summary.version < 136 {
            r.u32()?
        } else {
            u16::from_le_bytes(r.take(2)?.try_into().unwrap()) as u32
        };
        if dimension == 0 || dimension > 65536 {
            return Err("invalid property ArrayDim".into());
        }
        let flags = r.u32()?;
        pkg.name(r.index()?)?;
        if flags & 0x20 != 0 {
            r.take(2)?;
        }
        let struct_name = if kind.eq_ignore_ascii_case("Core.StructProperty") {
            Some(pkg.object_path(r.index()?)?)
        } else {
            None
        };
        fields.push(Field {
            name: export.name.clone(),
            kind,
            dimension,
            flags,
            struct_name,
        });
    }
    Ok(fields)
}

/// Matches core.dll UClass/UStruct/UState serializers. Bytecode is traversed
/// structurally, accounting for compact references versus logical 32-bit slots.
pub fn read(pkg: &Package, data: &[u8], export: &Export) -> Result<ClassDefaults> {
    if export.class != 0 || export.flags & 0x02000000 != 0 {
        return Err("expected UClass without a state frame".into());
    }
    let payload = pkg.payload(data, export)?;
    let mut r = Reader::at(payload, 0)?;
    let super_field = r.index()?;
    pkg.object_path(super_field)?;
    if super_field != export.super_class {
        return Err("serialized SuperField differs from class export".into());
    }
    pkg.object_path(r.index()?)?; // ScriptText
    let mut children = HashSet::new();
    loop {
        let child = r.index()?;
        if child == 0 {
            break;
        }
        pkg.object_path(child)?;
        if !children.insert(child) || children.len() > 65536 {
            return Err("duplicate or excessive class children".into());
        }
    }
    if pkg.name(r.index()?)? != export.name {
        return Err("class FriendlyName differs from export name".into());
    }
    if pkg.summary.version > 119 {
        pkg.object_path(r.index()?)?; // CppText
    }
    r.take(8)?; // Line, TextPos
    if pkg.summary.version > 129 {
        r.take(4)?;
    }
    let script_size = r.u32()?;
    if script_size > 1_000_000 {
        return Err("class script exceeds limit".into());
    }
    let mut logical = 0;
    while logical < script_size {
        expression(pkg, &mut r, &mut logical, script_size, 0)?;
    }
    r.take(22)?; // ProbeMask, IgnoreMask, LabelTableOffset, StateFlags
    let class_flags = r.u32()?;
    r.take(16)?; // ClassGuid
    let dependencies = r.count(9)?;
    for _ in 0..dependencies {
        pkg.object_path(r.index()?)?;
        r.take(8)?; // Deep, ScriptTextCRC
    }
    let imports = r.count(1)?;
    for _ in 0..imports {
        pkg.name(r.index()?)?;
    }
    pkg.object_path(r.index()?)?; // ClassWithin (supported versions >=99)
    let config_name = pkg.name(r.index()?)?;
    let additional_names = r.count(1)?;
    for _ in 0..additional_names {
        pkg.name(r.index()?)?;
    }
    let defaults_offset = r.pos;
    let arrays = struct_arrays(pkg, data, export)?;
    let properties = properties::read_reader_with_arrays(pkg, &mut r, &arrays)?;
    if r.pos != payload.len() {
        return Err(format!(
            "{} unconsumed class bytes (defaults {defaults_offset}, end {})",
            payload.len() - r.pos,
            r.pos
        ));
    }
    Ok(ClassDefaults {
        class_flags,
        config_name,
        defaults_offset,
        properties,
    })
}

// UArrayProperty -> Inner UStructProperty -> UStruct determines tagged array
// boundaries. Serialized size is advisory (the engine logs size mismatches).
fn struct_arrays(pkg: &Package, data: &[u8], class: &Export) -> Result<HashMap<String, String>> {
    if pkg.summary.version <= 117 {
        return Ok(HashMap::new()); // Older structs use native binary serialization.
    }
    let mut owners = HashSet::new();
    let mut current = pkg
        .exports
        .iter()
        .position(|e| {
            e.name == class.name
                && e.outer == class.outer
                && e.serial_offset == class.serial_offset
                && e.class == class.class
        })
        .map(|i| i as i32 + 1);
    while let Some(index) = current {
        if !owners.insert(index) {
            return Err("local class ancestry cycle".into());
        }
        let parent = pkg.exports[(index - 1) as usize].super_class;
        current = (parent > 0).then_some(parent);
        if parent > 0 {
            pkg.object_path(parent)?;
        }
    }
    let mut arrays = HashMap::new();
    for export in &pkg.exports {
        if !owners.contains(&export.outer)
            || !pkg
                .object_path(export.class)?
                .eq_ignore_ascii_case("Core.ArrayProperty")
        {
            continue;
        }
        let inner = property_target(pkg, data, export)?;
        if inner <= 0 {
            continue;
        }
        let inner_export = pkg
            .exports
            .get(inner as usize - 1)
            .ok_or("invalid array Inner export")?;
        if !pkg
            .object_path(inner_export.class)?
            .eq_ignore_ascii_case("Core.StructProperty")
        {
            continue;
        }
        let target = property_target(pkg, data, inner_export)?;
        if target == 0 {
            return Err("null array element struct".into());
        }
        let path = pkg.object_path(target)?;
        if ["Vector", "Rotator", "Color"].iter().any(|name| {
            path.rsplit('.')
                .next()
                .is_some_and(|p| p.eq_ignore_ascii_case(name))
        }) {
            continue; // Native binary structs do not use tagged members.
        }
        arrays.insert(export.name.to_lowercase(), path);
    }
    Ok(arrays)
}

fn property_target(pkg: &Package, data: &[u8], export: &Export) -> Result<i32> {
    let payload = pkg.payload(data, export)?;
    let properties = properties::read(pkg, data, export)?;
    let mut r = Reader::at(payload, properties.native_offset)?;
    if pkg.summary.version < 137 {
        pkg.object_path(r.index()?)?;
    }
    r.take(if pkg.summary.version < 136 { 4 } else { 2 })?; // ArrayDim
    let flags = r.u32()?;
    pkg.name(r.index()?)?; // Category
    if flags & 0x20 != 0 {
        r.take(2)?;
    } // RepOffset
    let target = r.index()?;
    pkg.object_path(target)?;
    if r.pos != payload.len() {
        return Err("unconsumed array/struct property metadata".into());
    }
    Ok(target)
}

// Bounded subset observed in UClass replication expressions. Unknown tokens
// fail explicitly; this is neither a VM nor a general UnrealScript decompiler.
fn expression(
    pkg: &Package,
    r: &mut Reader<'_>,
    pos: &mut u32,
    size: u32,
    depth: u32,
) -> Result<u8> {
    if depth > 128 || *pos >= size {
        return Err("class expression exceeds script/depth bounds".into());
    }
    let token = r.byte()?;
    *pos += 1;
    match token {
        0 | 1 | 2 | 0x20 | 0x29 => {
            pkg.object_path(r.index()?)?;
            *pos += 4;
        }
        0x21 | 0x44 => {
            pkg.name(r.index()?)?;
            *pos += 4;
        }
        0x13 | 0x2e | 0x36 => {
            pkg.object_path(r.index()?)?;
            *pos += 4;
            expression(pkg, r, pos, size, depth + 1)?;
        }
        0x12 | 0x19 => {
            expression(pkg, r, pos, size, depth + 1)?;
            r.take(3)?;
            *pos += 3;
            expression(pkg, r, pos, size, depth + 1)?;
        }
        0x1d | 0x1e => {
            r.take(4)?;
            *pos += 4;
        }
        0x24 | 0x2c => {
            r.take(1)?;
            *pos += 1;
        }
        6 => {
            r.take(2)?;
            *pos += 2;
        }
        4 | 0xd | 0xe | 0x37 | 0x39 => {
            if token == 0x39 {
                r.take(1)?;
                *pos += 1;
            }
            expression(pkg, r, pos, size, depth + 1)?;
        }
        7 | 9 | 0x18 => {
            r.take(2)?;
            *pos += 2;
            expression(pkg, r, pos, size, depth + 1)?;
        }
        0xf | 0x10 | 0x14 | 0x1a | 0x45 => {
            expression(pkg, r, pos, size, depth + 1)?;
            expression(pkg, r, pos, size, depth + 1)?;
        }
        8 | 0xb | 0x16 | 0x17 | 0x25 | 0x26 | 0x27 | 0x28 | 0x2a | 0x2d | 0x30 | 0x31 => {}
        0x60..=0xff => {
            if token < 0x70 {
                r.take(1)?;
                *pos += 1;
            }
            while expression(pkg, r, pos, size, depth + 1)? != 0x16 {}
        }
        _ => return Err(format!("unsupported class expression token 0x{token:02x}")),
    }
    if *pos > size {
        return Err("class expression logical size overrun".into());
    }
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(script: &[u8], logical_size: u32) -> (Package, Export, Vec<u8>) {
        let pkg = Package {
            summary: super::super::Summary {
                version: 159,
                licensee_version: 1,
                flags: 0,
                name_count: 3,
                name_offset: 0,
                export_count: 0,
                export_offset: 0,
                import_count: 0,
                import_offset: 0,
            },
            names: vec!["None".into(), "TestClass".into(), "Enabled".into()],
            imports: vec![],
            exports: vec![],
        };
        let mut bytes = vec![0, 0, 0, 1, 0];
        bytes.extend_from_slice(&[0; 12]);
        bytes.extend_from_slice(&logical_size.to_le_bytes());
        bytes.extend_from_slice(script);
        bytes.extend_from_slice(&[0; 22 + 4 + 16]);
        bytes.extend_from_slice(&[0; 5]); // dependency/import counts, within, config, names
        bytes.extend_from_slice(&[2, 0x83, 0]);
        let export = Export {
            class: 0,
            super_class: 0,
            outer: 0,
            extra: None,
            name: "TestClass".into(),
            flags: 0,
            serial_size: bytes.len() as u32,
            serial_offset: 0,
        };
        (pkg, export, bytes)
    }
    #[test]
    fn class_header_defaults_and_every_truncation() {
        let (pkg, mut export, bytes) = fixture(&[], 0);
        let defaults = read(&pkg, &bytes, &export).unwrap();
        assert!(matches!(
            defaults.properties.values[0].value,
            properties::Value::Bool(true)
        ));
        for end in 0..bytes.len() {
            export.serial_size = end as u32;
            assert!(read(&pkg, &bytes[..end], &export).is_err(), "prefix {end}");
        }
        export.serial_size = bytes.len() as u32;
        export.super_class = 1;
        assert!(read(&pkg, &bytes, &export).is_err());
    }
    #[test]
    fn compact_bytecode_references_use_logical_width() {
        // Native operator, null object constant (serialized 1 byte, logical 4), EndParms.
        let (pkg, export, bytes) = fixture(&[0x80, 0x20, 0, 0x16], 7);
        assert!(read(&pkg, &bytes, &export).is_ok());
        let (pkg, export, bytes) = fixture(&[0x80, 0x20, 0, 0x16], 6);
        assert!(read(&pkg, &bytes, &export).is_err());
        let (pkg, export, bytes) = fixture(&[0x20, 1], 5);
        assert!(read(&pkg, &bytes, &export).is_err()); // invalid object reference
    }
    #[test]
    fn unknown_bytecode_and_trailing_payload_are_rejected() {
        let (pkg, export, bytes) = fixture(&[0x46], 1);
        assert!(read(&pkg, &bytes, &export)
            .unwrap_err()
            .contains("unsupported"));
        let (pkg, mut export, mut bytes) = fixture(&[], 0);
        bytes.push(0);
        export.serial_size += 1;
        assert!(read(&pkg, &bytes, &export)
            .unwrap_err()
            .contains("unconsumed"));
    }
    #[test]
    fn property_metadata_target_and_all_truncations() {
        let (pkg, mut export, _) = fixture(&[], 0);
        let bytes = [0, 1, 0, 0x20, 0, 0, 0, 0, 7, 0, 0];
        // None, ArrayDim=1, CPF_Net, Category=None, RepOffset=7, null target.
        export.serial_size = bytes.len() as u32;
        assert_eq!(property_target(&pkg, &bytes, &export).unwrap(), 0);
        for end in 0..bytes.len() {
            export.serial_size = end as u32;
            assert!(property_target(&pkg, &bytes[..end], &export).is_err());
        }
        let mut trailing = bytes.to_vec();
        trailing.push(0);
        export.serial_size = trailing.len() as u32;
        assert!(property_target(&pkg, &trailing, &export).is_err());
    }
}
