//! Bounded serialized script traversal to locate UState masks/function flags.
//! Layout only: no control-flow evaluation or bytecode execution.
use crate::{properties, state_children, Export, Package, Reader};
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Serialize)]
pub struct SerializedStateMasks {
    pub probe_mask: u64,
    pub ignore_mask: u64,
    pub label_table_offset: u16,
    pub state_flags: u32,
    pub logical_script_bytes: u32,
    pub metadata_offset: usize,
}
fn raw(r: &mut Reader<'_>, pos: &mut u32, n: u32) -> Result<(), String> {
    r.take(n as usize)?;
    *pos += n;
    Ok(())
}
fn reference(
    pkg: &Package,
    r: &mut Reader<'_>,
    pos: &mut u32,
    name: bool,
) -> Result<String, String> {
    let index = r.index()?;
    *pos += 4;
    if name {
        pkg.name(index)
    } else {
        pkg.object_path(index)
    }
}
fn expr(
    pkg: &Package,
    r: &mut Reader<'_>,
    pos: &mut u32,
    size: u32,
    depth: u32,
) -> Result<u8, String> {
    if depth > 128 || *pos >= size {
        return Err("script layout bounds/depth exceeded".into());
    }
    let token = r.byte()?;
    *pos += 1;
    match token {
        0 | 1 | 2 | 0x20 | 0x29 => {
            reference(pkg, r, pos, false)?;
        }
        0x21 | 0x44 => {
            reference(pkg, r, pos, true)?;
        }
        6 => raw(r, pos, 2)?,
        7 | 9 | 0x18 => {
            raw(r, pos, 2)?;
            expr(pkg, r, pos, size, depth + 1)?;
        }
        8 | 0xb | 0x16 | 0x17 | 0x25 | 0x26 | 0x27 | 0x28 | 0x2a | 0x2d | 0x30 | 0x31 => {}
        0xa => {
            let offset = u16::from_le_bytes(r.take(2)?.try_into().unwrap());
            *pos += 2;
            if offset != u16::MAX {
                expr(pkg, r, pos, size, depth + 1)?;
            }
        }
        0xc => loop {
            let name = reference(pkg, r, pos, true)?;
            raw(r, pos, 4)?;
            if *pos > size {
                return Err("label table exceeds script size".into());
            }
            if name == "None" {
                break;
            }
        },
        4 | 0xd | 0xe | 0x37 => {
            expr(pkg, r, pos, size, depth + 1)?;
        }
        0xf | 0x10 | 0x14 | 0x1a | 0x45 | 0x11 | 0x40 | 0x41 => {
            let count = if token == 0x11 {
                4
            } else if matches!(token, 0x40 | 0x41) {
                3
            } else {
                2
            };
            for _ in 0..count {
                expr(pkg, r, pos, size, depth + 1)?;
            }
        }
        0x12 | 0x19 => {
            expr(pkg, r, pos, size, depth + 1)?;
            raw(r, pos, 3)?;
            expr(pkg, r, pos, size, depth + 1)?;
        }
        5 | 0x39 => {
            raw(r, pos, 1)?;
            expr(pkg, r, pos, size, depth + 1)?;
        }
        0x13 | 0x2e | 0x36 => {
            reference(pkg, r, pos, false)?;
            expr(pkg, r, pos, size, depth + 1)?;
        }
        0x1b | 0x38 | 0x1c | 0x60..=0xff => {
            match token {
                0x1b | 0x38 => {
                    reference(pkg, r, pos, true)?;
                }
                0x1c => {
                    reference(pkg, r, pos, false)?;
                }
                0x60..=0x6f => raw(r, pos, 1)?,
                _ => {}
            }
            while expr(pkg, r, pos, size, depth + 1)? != 0x16 {}
        }
        0x1d | 0x1e => raw(r, pos, 4)?,
        0x22 | 0x23 => raw(r, pos, 12)?,
        0x24 | 0x2c => raw(r, pos, 1)?,
        0x1f => loop {
            let byte = r.byte()?;
            *pos += 1;
            if byte == 0 {
                break;
            }
            if *pos >= size {
                return Err("unterminated script string".into());
            }
        },
        0x2f => {
            expr(pkg, r, pos, size, depth + 1)?;
            raw(r, pos, 2)?;
        }
        0x32 | 0x33 => {
            reference(pkg, r, pos, false)?;
            expr(pkg, r, pos, size, depth + 1)?;
            expr(pkg, r, pos, size, depth + 1)?;
        }
        0x43 => {
            reference(pkg, r, pos, false)?;
            reference(pkg, r, pos, true)?;
        }
        _ => {
            return Err(format!(
                "unreviewed serialized script layout token 0x{token:02x}"
            ))
        }
    }
    if *pos > size {
        return Err("script layout logical overrun".into());
    }
    Ok(token)
}
fn script_tail(pkg: &Package, r: &mut Reader<'_>) -> Result<u32, String> {
    pkg.object_path(r.index()?)?; // CppText
    r.take(8)?;
    if pkg.summary.version > 129 {
        r.take(4)?;
    }
    let size = r.u32()?;
    if size > 1_000_000 {
        return Err("script layout size limit".into());
    }
    let mut pos = 0;
    while pos < size {
        expr(pkg, r, &mut pos, size, 0)?;
    }
    Ok(size)
}
pub fn read(pkg: &Package, data: &[u8], owner_index: i32) -> Result<SerializedStateMasks, String> {
    let header = state_children::read(pkg, data, owner_index)?;
    let owner = &pkg.exports[owner_index as usize - 1];
    let payload = pkg.payload(data, owner)?;
    let mut r = Reader::at(payload, header.prefix_bytes)?;
    let logical_script_bytes = script_tail(pkg, &mut r)?;
    let metadata_offset = r.pos;
    let probe_mask = u64::from_le_bytes(r.take(8)?.try_into().unwrap());
    let ignore_mask = u64::from_le_bytes(r.take(8)?.try_into().unwrap());
    let label_table_offset = u16::from_le_bytes(r.take(2)?.try_into().unwrap());
    let state_flags = r.u32()?;
    if owner.class != 0 && r.pos != payload.len() {
        return Err("unconsumed UState payload".into());
    }
    Ok(SerializedStateMasks {
        probe_mask,
        ignore_mask,
        label_table_offset,
        state_flags,
        logical_script_bytes,
        metadata_offset,
    })
}
/// Validate layout through the trailer; does not assume flags are the last word
/// because FUNC_Net adds a replication offset after them.
pub fn function_flags(pkg: &Package, data: &[u8], export: &Export) -> Result<u32, String> {
    if !(120..=159).contains(&pkg.summary.version)
        || pkg.summary.licensee_version != 1
        || !pkg
            .object_path(export.class)?
            .eq_ignore_ascii_case("Core.Function")
        || export.flags & 0x02000000 != 0
    {
        return Err("unsupported function layout".into());
    }
    let props = properties::read(pkg, data, export)?;
    if !props.values.is_empty() {
        return Err("nonempty function properties".into());
    }
    let payload = pkg.payload(data, export)?;
    let mut r = Reader::at(payload, props.native_offset)?;
    let parent = r.index()?;
    pkg.object_path(parent)?;
    if parent != export.super_class {
        return Err("function SuperField mismatch".into());
    }
    pkg.object_path(r.index()?)?;
    let mut seen = HashSet::new();
    loop {
        let child = r.index()?;
        if child == 0 {
            break;
        }
        pkg.object_path(child)?;
        if !seen.insert(child) || seen.len() > 65536 {
            return Err("duplicate/excessive function children".into());
        }
    }
    pkg.name(r.index()?)?;
    script_tail(pkg, &mut r)?;
    r.take(3)?; // NativeIndex u16, precedence byte
    let flags = r.u32()?;
    if flags & 0x40 != 0 {
        r.take(2)?;
    }
    if r.pos != payload.len() {
        return Err("unconsumed function layout payload".into());
    }
    Ok(flags)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(script: &[u8], size: u32, function: bool) -> (Package, Vec<u8>) {
        let mut data = vec![];
        if function {
            data.push(0);
        }
        data.extend([0, 0, 0, 1, 0]);
        data.extend([0; 12]);
        data.extend(size.to_le_bytes());
        data.extend(script);
        if function {
            data.extend([0; 3]);
            data.extend(0x20840u32.to_le_bytes());
            data.extend(0x1234u16.to_le_bytes());
        } else {
            data.extend((1u64 << 53).to_le_bytes());
            data.extend((!1u64).to_le_bytes());
            data.extend(1u16.to_le_bytes());
            data.extend(2u32.to_le_bytes());
        }
        let export = Export {
            class: if function { -1 } else { 0 },
            super_class: 0,
            outer: 0,
            extra: None,
            name: "Test".into(),
            flags: 0,
            serial_offset: 0,
            serial_size: data.len() as u32,
        };
        let pkg = Package {
            summary: crate::Summary {
                version: 159,
                licensee_version: 1,
                flags: 0,
                name_count: 3,
                name_offset: 0,
                export_count: 1,
                export_offset: 0,
                import_count: 2,
                import_offset: 0,
            },
            names: vec!["None".into(), "Test".into(), "Begin".into()],
            imports: vec![
                crate::Import {
                    class_package: "Core".into(),
                    class_name: "Class".into(),
                    outer: -2,
                    name: "Function".into(),
                },
                crate::Import {
                    class_package: "Core".into(),
                    class_name: "Package".into(),
                    outer: 0,
                    name: "Core".into(),
                },
            ],
            exports: vec![export],
        };
        (pkg, data)
    }
    #[test]
    fn labels_use_logical_name_width_and_mask_words_survive_truncation_checks() {
        let script = [0xc, 2, 1, 0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff]; // Begin offset1, None sentinel
        let (mut pkg, data) = fixture(&script, 17, false);
        let masks = read(&pkg, &data, 1).unwrap();
        assert_eq!(masks.probe_mask, 1u64 << 53);
        assert_eq!(masks.ignore_mask, !1u64);
        assert_eq!(masks.state_flags, 2);
        assert_eq!(masks.logical_script_bytes, 17);
        for end in 0..data.len() {
            pkg.exports[0].serial_size = end as u32;
            assert!(read(&pkg, &data[..end], 1).is_err(), "prefix {end}");
        }
        pkg.imports.push(crate::Import {
            class_package: "Core".into(),
            class_name: "Class".into(),
            outer: -2,
            name: "State".into(),
        });
        pkg.exports[0].class = -3;
        let mut state_data = data.clone();
        state_data.insert(0, 0);
        pkg.exports[0].serial_size = state_data.len() as u32;
        assert_eq!(read(&pkg, &state_data, 1).unwrap().probe_mask, 1u64 << 53);
        state_data.push(0);
        pkg.exports[0].serial_size = state_data.len() as u32;
        assert!(read(&pkg, &state_data, 1).is_err());
    }
    #[test]
    fn function_replication_trailer_is_parsed_not_guessed_from_last_word() {
        let (mut pkg, data) = fixture(&[4, 0xb], 2, true);
        assert_eq!(
            function_flags(&pkg, &data, &pkg.exports[0]).unwrap(),
            0x20840
        );
        for end in 0..data.len() {
            pkg.exports[0].serial_size = end as u32;
            assert!(
                function_flags(&pkg, &data[..end], &pkg.exports[0]).is_err(),
                "prefix {end}"
            );
        }
        let mut trailing = data.clone();
        trailing.push(0);
        pkg.exports[0].serial_size = trailing.len() as u32;
        assert!(function_flags(&pkg, &trailing, &pkg.exports[0]).is_err());
    }
    #[test]
    fn unknown_layout_overrun_and_unterminated_strings_fail() {
        for (script, size) in [
            (vec![0x46], 1),
            (vec![0x1d, 0, 0, 0, 0], 4),
            (vec![0x1f, b'x'], 2),
            (vec![0xc, 2, 0, 0, 0, 0], 9),
        ] {
            let (pkg, data) = fixture(&script, size, false);
            assert!(read(&pkg, &data, 1).is_err());
        }
    }
}
