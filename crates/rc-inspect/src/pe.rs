use serde_json::{json, Value};
pub fn inspect(data: &[u8]) -> Result<Value, String> {
    let u16at = |p: usize| -> Result<u16, String> {
        Ok(u16::from_le_bytes(
            data.get(p..p + 2)
                .ok_or("truncated PE")?
                .try_into()
                .unwrap(),
        ))
    };
    let u32at = |p: usize| -> Result<u32, String> {
        Ok(u32::from_le_bytes(
            data.get(p..p + 4)
                .ok_or("truncated PE")?
                .try_into()
                .unwrap(),
        ))
    };
    if data.get(..2) != Some(b"MZ") {
        return Err("not PE".into());
    }
    let pe = u32at(0x3c)? as usize;
    if data.get(pe..pe + 4) != Some(b"PE\0\0") {
        return Err("bad PE signature".into());
    }
    let optional = pe + 24;
    let magic = u16at(optional)?;
    if magic != 0x10b {
        return Err("only PE32 supported".into());
    }
    let image_base = u32at(optional + 28)?;
    let table = optional + u16at(pe + 20)? as usize;
    let mut sections = Vec::new();
    for i in 0..u16at(pe + 6)? as usize {
        let p = table + i * 40;
        let name = data.get(p..p + 8).ok_or("bad section")?;
        sections.push((
            String::from_utf8_lossy(name)
                .trim_end_matches('\0')
                .to_string(),
            u32at(p + 12)?,
            u32at(p + 8)?,
            u32at(p + 20)?,
            u32at(p + 16)?,
        ));
    }
    let offset = |rva: u32| -> Result<usize, String> {
        for (_, va, _, raw, size) in &sections {
            if let Some(delta) = rva.checked_sub(*va) {
                if delta < *size {
                    return Ok(*raw as usize + delta as usize);
                }
            }
        }
        if rva < u32at(optional + 60)? {
            return Ok(rva as usize);
        }
        Err(format!("unmapped RVA {rva:x}"))
    };
    let cstr = |p: usize| -> Result<String, String> {
        let tail = data.get(p..).ok_or("bad string offset")?;
        let len = tail
            .iter()
            .position(|&b| b == 0)
            .ok_or("unterminated PE string")?;
        Ok(String::from_utf8_lossy(&tail[..len]).to_string())
    };
    let metadata = json!({"machine":format!("0x{:04x}",u16at(pe+4)?),"image_base":format!("{image_base:08x}"),"entry_point":format!("{:08x}",u64::from(image_base)+u64::from(u32at(optional+16)?)),"sections":sections});
    let directories = (|| -> Result<Value, String> {
        let mut exports = Vec::new();
        let export_rva = u32at(optional + 96)?;
        if export_rva != 0 {
            let e = offset(export_rva)?;
            let base = u32at(e + 16)?;
            let functions = offset(u32at(e + 28)?)?;
            let names = offset(u32at(e + 32)?)?;
            let ordinals = offset(u32at(e + 36)?)?;
            let count = u32at(e + 24)?;
            if count > data.len() as u32 / 4 {
                return Err("bad export count".into());
            }
            for i in 0..count as usize {
                let ordinal = u16at(ordinals + i * 2)? as usize;
                if ordinal >= u32at(e + 20)? as usize {
                    return Err("invalid export ordinal".into());
                }
                let rva = u32at(functions + ordinal * 4)?;
                let forwarder = if rva >= export_rva && rva - export_rva < u32at(optional + 100)? {
                    Some(cstr(offset(rva)?)?)
                } else {
                    None
                };
                exports.push(json!({"name":cstr(offset(u32at(names+i*4)?)?)?,"ordinal":base+ordinal as u32,"rva":rva,"address":format!("{:08x}",u64::from(image_base)+u64::from(rva)),"forwarder":forwarder}));
            }
        }
        let mut imports = Vec::new();
        let import_rva = u32at(optional + 104)?;
        if import_rva != 0 {
            let mut p = offset(import_rva)?;
            for _ in 0..4096 {
                let lookup = u32at(p)?;
                let name = u32at(p + 12)?;
                let iat = u32at(p + 16)?;
                if lookup == 0 && name == 0 && iat == 0 {
                    break;
                }
                let dll = cstr(offset(name)?)?;
                let mut t = offset(if lookup == 0 { iat } else { lookup })?;
                let mut symbols = Vec::new();
                for _ in 0..data.len() / 4 {
                    let v = u32at(t)?;
                    if v == 0 {
                        break;
                    }
                    symbols.push(if v & 0x80000000 != 0 {
                        json!({"ordinal":v&0xffff})
                    } else {
                        json!({"name":cstr(offset(v)?+2)?})
                    });
                    t += 4;
                }
                imports.push(json!({"dll":dll,"symbols":symbols}));
                p += 20;
            }
        }
        Ok(json!({"exports":exports,"imports":imports}))
    })();
    let mut info = metadata;
    match directories {
        Ok(d) => {
            info["exports"] = d["exports"].clone();
            info["imports"] = d["imports"].clone();
        }
        Err(e) => {
            info["directory_error"] = json!(e);
            info["exports"] = Value::Null;
            info["imports"] = Value::Null;
        }
    }
    Ok(info)
}
