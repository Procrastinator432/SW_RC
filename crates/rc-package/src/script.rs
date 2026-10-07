//! Bounded function bytecode inspection for SWRC v159; no VM execution.
use crate::{properties, Export, Package, Reader, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Deserialize, Serialize)]
pub struct Function {
    pub friendly_name: String,
    pub script_offset: usize,
    pub serialized_script_bytes: usize,
    pub logical_script_bytes: u32,
    pub native_index: u16,
    pub precedence: u8,
    pub flags: u32,
    pub replication_offset: Option<u16>,
    pub expressions: Vec<Expression>,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct Expression {
    pub logical_offset: u32,
    pub serialized_offset: usize,
    pub logical_end: u32,
    pub opcode: u8,
    pub operand: Operand,
    pub children: Vec<Expression>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", content = "value")]
pub enum Operand {
    None,
    Object {
        index: i32,
        path: String,
    },
    Name(String),
    /// Zero-terminated ANSI bytes preserved one byte per Unicode code point.
    String(String),
    Byte(u8),
    Int(i32),
    Float(f32),
    Vector([f32; 3]),
    Rotator([i32; 3]),
    Target(u16),
    Context {
        skip: u16,
        result_size: u8,
    },
    Native(u16),
}
fn word(r: &mut Reader<'_>) -> Result<u16> {
    Ok(u16::from_le_bytes(r.take(2)?.try_into().unwrap()))
}
pub fn read_function(pkg: &Package, data: &[u8], export: &Export) -> Result<Function> {
    if pkg.summary.version != 159
        || pkg.summary.licensee_version != 1
        || !pkg
            .object_path(export.class)?
            .eq_ignore_ascii_case("Core.Function")
        || export.flags & 0x02000000 != 0
    {
        return Err("expected SWRC v159/1 function without state frame".into());
    }
    let payload = pkg.payload(data, export)?;
    let properties = properties::read(pkg, data, export)?;
    if !properties.values.is_empty() {
        return Err("function tagged properties not supported".into());
    }
    let mut r = Reader::at(payload, properties.native_offset)?;
    let parent = r.index()?;
    pkg.object_path(parent)?;
    if parent != export.super_class {
        return Err("function SuperField mismatch".into());
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
            return Err("duplicate or excessive function children".into());
        }
    }
    // Operator functions have symbolic FriendlyName, e.g. !, versus Not_PreBool.
    let friendly_name = pkg.name(r.index()?)?;
    pkg.object_path(r.index()?)?; // CppText
    r.take(12)?; // Line, TextPos, v>129 extra
    let size = r.u32()?;
    if size > 1_000_000 {
        return Err("function script exceeds limit".into());
    }
    let start = r.pos;
    let mut pos = 0;
    let mut expressions = Vec::new();
    while pos < size {
        expressions.push(expression(pkg, &mut r, &mut pos, size, start, 0)?);
    }
    let serialized_script_bytes = r.pos - start;
    let native_index = word(&mut r)?;
    let precedence = r.byte()?;
    let flags = r.u32()?;
    let replication_offset = if flags & 0x40 != 0 {
        Some(word(&mut r)?)
    } else {
        None
    };
    if r.pos != payload.len() {
        return Err("unconsumed function payload".into());
    }
    // Absolute jumps must point to a top-level instruction, not a compressed file offset.
    let boundaries: HashSet<_> = expressions.iter().map(|e| e.logical_offset).collect();
    for e in &expressions {
        if matches!(e.opcode, 6 | 7) {
            if let Operand::Target(target) = e.operand {
                if !boundaries.contains(&u32::from(target)) {
                    return Err(format!(
                        "jump target {target} is not an instruction boundary"
                    ));
                }
            }
        }
    }
    Ok(Function {
        friendly_name,
        script_offset: start,
        serialized_script_bytes,
        logical_script_bytes: size,
        native_index,
        precedence,
        flags,
        replication_offset,
        expressions,
    })
}

fn expression(
    pkg: &Package,
    r: &mut Reader<'_>,
    pos: &mut u32,
    size: u32,
    start: usize,
    depth: u32,
) -> Result<Expression> {
    if depth > 128 || *pos >= size {
        return Err("expression exceeds script/depth bounds".into());
    }
    let logical_offset = *pos;
    let serialized_offset = r.pos - start;
    let opcode = r.byte()?;
    *pos += 1;
    let mut operand = Operand::None;
    let mut children = Vec::new();
    let count = match opcode {
        0 | 1 | 2 | 0x20 | 0x29 | 0x13 | 0x2e | 0x36 | 0x1c => {
            let index = r.index()?;
            operand = Operand::Object {
                index,
                path: pkg.object_path(index)?,
            };
            *pos += 4;
            usize::from(matches!(opcode, 0x13 | 0x2e | 0x36))
        }
        0x21 | 0x1b => {
            operand = Operand::Name(pkg.name(r.index()?)?.to_string());
            *pos += 4;
            0
        }
        6 | 7 | 0x18 => {
            operand = Operand::Target(word(r)?);
            *pos += 2;
            usize::from(opcode != 6)
        }
        0x12 | 0x19 => {
            children.push(expression(pkg, r, pos, size, start, depth + 1)?);
            operand = Operand::Context {
                skip: word(r)?,
                result_size: r.byte()?,
            };
            *pos += 3;
            1
        }
        0x1d => {
            operand = Operand::Int(r.i32()?);
            *pos += 4;
            0
        }
        0x1e => {
            operand = Operand::Float(r.f32()?);
            *pos += 4;
            0
        }
        0x1f => {
            let mut value = String::new();
            loop {
                if *pos >= size {
                    return Err("unterminated script string within logical bounds".into());
                }
                let byte = r.byte()?;
                *pos += 1;
                if byte == 0 {
                    break;
                }
                value.push(char::from(byte));
            }
            operand = Operand::String(value);
            0
        }
        0x22 => {
            operand = Operand::Rotator([r.i32()?, r.i32()?, r.i32()?]);
            *pos += 12;
            0
        }
        0x23 => {
            operand = Operand::Vector(r.vector()?);
            *pos += 12;
            0
        }
        0x24 | 0x2c | 0x39 => {
            operand = Operand::Byte(r.byte()?);
            *pos += 1;
            usize::from(opcode == 0x39)
        }
        4 | 0xd | 0xe | 0x2d | 0x37 => 1,
        0xf | 0x10 | 0x14 | 0x1a | 0x45 => 2,
        8 | 0xb | 0x16 | 0x17 | 0x25 | 0x26 | 0x27 | 0x28 | 0x2a | 0x30 | 0x31 => 0,
        0x60..=0xff => {
            let index = if opcode < 0x70 {
                *pos += 1;
                (u16::from(opcode - 0x60) << 8) | u16::from(r.byte()?)
            } else {
                u16::from(opcode)
            };
            operand = Operand::Native(index);
            0
        }
        _ => {
            return Err(format!(
                "unsupported function opcode 0x{opcode:02x} at logical {logical_offset}"
            ))
        }
    };
    for _ in 0..count {
        children.push(expression(pkg, r, pos, size, start, depth + 1)?);
    }
    if matches!(opcode, 0x1b | 0x1c | 0x60..=0xff) {
        loop {
            let child = expression(pkg, r, pos, size, start, depth + 1)?;
            let end = child.opcode == 0x16;
            children.push(child);
            if end {
                break;
            }
        }
    }
    if *pos > size {
        return Err("function logical size overrun".into());
    }
    if opcode == 0x18 {
        if let Operand::Target(skip) = operand {
            // Native bool skip includes the enclosing EndFunctionParms byte.
            if u32::from(skip) != children[0].logical_end - children[0].logical_offset + 1
                || r.data.get(r.pos) != Some(&0x16)
            {
                return Err("short-circuit skip does not match expression length".into());
            }
        }
    }
    if let Operand::Context { skip, .. } = operand {
        let result = &children[1];
        if u32::from(skip) != result.logical_end - result.logical_offset {
            return Err("context skip does not match expression length".into());
        }
    }
    Ok(Expression {
        logical_offset,
        serialized_offset,
        logical_end: *pos,
        opcode,
        operand,
        children,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Import, Summary};
    #[test]
    fn ansi_strings_preserve_bytes_and_instruction_boundaries() {
        let (pkg, e, bytes) = fixture(&[0x1f, 0, 0x1f, b'A', 0xe4, 0, 4, 0xb], 8);
        let f = read_function(&pkg, &bytes, &e).unwrap();
        assert!(matches!(&f.expressions[0].operand, Operand::String(s) if s.is_empty()));
        assert!(matches!(&f.expressions[1].operand, Operand::String(s) if s == "A\u{e4}"));
        assert_eq!(f.expressions[1].logical_offset, 2);
        assert_eq!(f.expressions[1].logical_end, 6);
        assert_eq!(f.expressions[2].logical_offset, 6);
        let (pkg, e, bytes) = fixture(&[6, 7, 0, 0x1f, b'a', b'b', 0, 4, 0xb], 9);
        assert!(read_function(&pkg, &bytes, &e).is_ok());
        let (pkg, e, bytes) = fixture(&[6, 5, 0, 0x1f, b'a', b'b', 0, 4, 0xb], 9);
        assert!(read_function(&pkg, &bytes, &e)
            .unwrap_err()
            .contains("boundary"));
    }
    #[test]
    fn strings_stop_at_logical_limit_without_reading_function_metadata() {
        for size in [1, 2, 3] {
            let (pkg, e, bytes) = fixture(&[0x1f, b'a', b'b'], size);
            assert!(read_function(&pkg, &bytes, &e)
                .unwrap_err()
                .contains("unterminated"));
        }
        let (pkg, e, bytes) = fixture(&[0x1f, b'x', 0], 3);
        for len in 0..bytes.len() {
            assert!(read_function(&pkg, &bytes[..len], &e).is_err());
        }
        assert!(read_function(&pkg, &bytes, &e).is_ok());
        let (pkg, e, bytes) = fixture(&[0x34, 0, 0], 3);
        assert!(read_function(&pkg, &bytes, &e)
            .unwrap_err()
            .contains("unsupported"));
    }
    #[test]
    fn vector_and_rotator_constants_preserve_values_and_width() {
        let mut script = vec![0x23];
        for v in [1.25_f32, -2.5, 0.0] {
            script.extend(v.to_le_bytes());
        }
        script.push(0x22);
        for v in [-32768_i32, 65536, i32::MAX] {
            script.extend(v.to_le_bytes());
        }
        let (pkg, e, bytes) = fixture(&script, 26);
        let f = read_function(&pkg, &bytes, &e).unwrap();
        assert!(matches!(
            f.expressions[0].operand,
            Operand::Vector([1.25, -2.5, 0.0])
        ));
        assert!(matches!(
            f.expressions[1].operand,
            Operand::Rotator([-32768, 65536, i32::MAX])
        ));
        assert_eq!(f.expressions[1].logical_offset, 13);
    }
    #[test]
    fn vector_nonfinite_and_constant_size_overrun_fail() {
        let mut script = vec![0x23];
        for v in [0.0_f32, f32::NAN, 0.0] {
            script.extend(v.to_le_bytes());
        }
        let (pkg, e, bytes) = fixture(&script, 13);
        assert!(read_function(&pkg, &bytes, &e)
            .unwrap_err()
            .contains("nonfinite"));
        script[5..9].copy_from_slice(&0.0_f32.to_le_bytes());
        let (pkg, e, bytes) = fixture(&script, 12);
        assert!(read_function(&pkg, &bytes, &e)
            .unwrap_err()
            .contains("overrun"));
    }
    fn fixture(script: &[u8], size: u32) -> (Package, Export, Vec<u8>) {
        let pkg = Package {
            summary: Summary {
                version: 159,
                licensee_version: 1,
                flags: 0,
                name_count: 3,
                name_offset: 0,
                export_count: 0,
                export_offset: 0,
                import_count: 2,
                import_offset: 0,
            },
            names: vec!["None".into(), "Test".into(), "!".into()],
            imports: vec![
                Import {
                    class_package: "Core".into(),
                    class_name: "Package".into(),
                    outer: 0,
                    name: "Core".into(),
                },
                Import {
                    class_package: "Core".into(),
                    class_name: "Class".into(),
                    outer: -1,
                    name: "Function".into(),
                },
            ],
            exports: vec![],
        };
        let mut bytes = vec![0, 0, 0, 0, 1, 0]; // None, super, text, children, friendly, cpp
        bytes.extend([0; 12]);
        bytes.extend(size.to_le_bytes());
        bytes.extend(script);
        bytes.extend([0; 7]);
        let export = Export {
            class: -2,
            super_class: 0,
            outer: 0,
            extra: None,
            name: "Test".into(),
            flags: 0,
            serial_size: bytes.len() as u32,
            serial_offset: 0,
        };
        (pkg, export, bytes)
    }
    #[test]
    fn logical_targets_differ_from_compact_file_offsets() {
        // Jump to Return after a compressed instance-object reference.
        let (pkg, e, bytes) = fixture(&[6, 8, 0, 1, 0, 4, 0x28], 10);
        let f = read_function(&pkg, &bytes, &e).unwrap();
        assert_eq!(f.serialized_script_bytes, 7);
        assert_eq!(f.expressions[2].logical_offset, 8);
        assert_eq!(f.expressions[2].serialized_offset, 5);
        let mut invalid = bytes;
        invalid[f.script_offset + 1] = 5;
        assert!(read_function(&pkg, &invalid, &e)
            .unwrap_err()
            .contains("boundary"));
    }
    #[test]
    fn truncation_unknown_tokens_and_trailing_bytes_fail() {
        let (pkg, e, bytes) = fixture(&[4, 0x28, 4, 0xb], 4);
        for n in 0..bytes.len() {
            let short = Export {
                serial_size: n as u32,
                name: e.name.clone(),
                ..e
            };
            assert!(
                read_function(&pkg, &bytes[..n], &short).is_err(),
                "truncation {n}"
            );
        }
        let mut bad = bytes.clone();
        bad[22] = 0x5f;
        assert!(read_function(&pkg, &bad, &e)
            .unwrap_err()
            .contains("unsupported"));
        let mut extra = bytes;
        extra.push(0);
        let bigger = Export {
            serial_size: extra.len() as u32,
            ..e
        };
        assert!(read_function(&pkg, &extra, &bigger)
            .unwrap_err()
            .contains("unconsumed"));
    }
    #[test]
    fn native_index_symbolic_name_and_bool_wrapper_preserve_tree() {
        let (pkg, e, mut bytes) = fixture(&[0x6f, 0x82, 0x2d, 1, 0, 0x16], 9);
        bytes[4] = 2; // Operator FriendlyName need not equal export name.
        let f = read_function(&pkg, &bytes, &e).unwrap();
        assert_eq!(f.friendly_name, "!");
        assert!(matches!(f.expressions[0].operand, Operand::Native(3970)));
        assert_eq!(f.expressions[0].children[0].children[0].opcode, 1);
    }
    #[test]
    fn skip_length_and_recursion_limits_are_checked() {
        let (pkg, e, bytes) = fixture(&[0x18, 3, 0, 0x28], 4);
        assert!(read_function(&pkg, &bytes, &e)
            .unwrap_err()
            .contains("skip"));
        let (pkg, e, bytes) = fixture(&[0x82, 0x27, 0x18, 2, 0, 0x28, 0x16], 7);
        assert!(read_function(&pkg, &bytes, &e).is_ok());
        let mut deep = vec![0x2d; 130];
        deep.push(0x28);
        let (pkg, e, bytes) = fixture(&deep, deep.len() as u32);
        assert!(read_function(&pkg, &bytes, &e)
            .unwrap_err()
            .contains("depth"));
    }
}
