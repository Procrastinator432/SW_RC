//! Ordered serialized UStruct Children prefix for original SWRC classes/states.
//! Reads the prefix only; does not validate state bytecode or the remaining payload.
use crate::{properties, Export, Package, Reader};
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Serialize)]
pub struct StateChild {
    pub export_index: i32,
    pub path: String,
    pub class: String,
    pub is_struct: bool,
    pub is_function: bool,
}
#[derive(Debug, Serialize)]
pub struct StateChildren {
    pub super_reference: i32,
    pub children: Vec<StateChild>,
    pub prefix_bytes: usize,
}
pub fn read(pkg: &Package, data: &[u8], owner_index: i32) -> Result<StateChildren, String> {
    let owner: &Export = pkg
        .exports
        .get(
            usize::try_from(owner_index.checked_sub(1).ok_or("invalid owner")?)
                .map_err(|_| "invalid owner")?,
        )
        .ok_or("missing owner export")?;
    if !(120..=159).contains(&pkg.summary.version)
        || pkg.summary.licensee_version != 1
        || owner.flags & 0x02000000 != 0
        || (owner.class != 0
            && !pkg
                .object_path(owner.class)?
                .eq_ignore_ascii_case("Core.State"))
    {
        return Err("expected SWRC class/state without object stack".into());
    }
    let payload = pkg.payload(data, owner)?;
    let start = if owner.class == 0 {
        0
    } else {
        let props = properties::read(pkg, data, owner)?;
        if !props.values.is_empty() {
            return Err("nonempty state properties unsupported".into());
        }
        props.native_offset
    };
    let mut reader = Reader::at(payload, start)?;
    let super_reference = reader.index()?;
    pkg.object_path(super_reference)?;
    if super_reference != owner.super_class {
        return Err("state/class SuperField mismatch".into());
    }
    pkg.object_path(reader.index()?)?; // ScriptText
    let mut children = Vec::new();
    let mut seen = HashSet::new();
    loop {
        let index = reader.index()?;
        if index == 0 {
            break;
        }
        if !seen.insert(index) || seen.len() > 65536 {
            return Err("duplicate/excessive ordered children".into());
        }
        let child = pkg
            .exports
            .get(
                usize::try_from(index.checked_sub(1).ok_or("invalid child")?)
                    .map_err(|_| "child is not local export")?,
            )
            .ok_or("missing child export")?;
        if child.outer != owner_index {
            return Err("serialized child has foreign Outer".into());
        }
        let class = if child.class == 0 {
            "Core.Class".into()
        } else {
            pkg.object_path(child.class)?
        };
        let lower = class.to_lowercase();
        let is_struct = matches!(
            lower.as_str(),
            "core.class" | "core.state" | "core.function" | "core.struct"
        );
        if !(is_struct
            || matches!(lower.as_str(), "core.enum" | "core.const")
            || (lower.starts_with("core.") && lower.ends_with("property")))
        {
            return Err(format!("unreviewed child field class: {class}"));
        }
        children.push(StateChild {
            export_index: index,
            path: pkg.object_path(index)?,
            is_function: lower == "core.function",
            class,
            is_struct,
        });
    }
    if pkg.name(reader.index()?)? != owner.name {
        return Err("state/class FriendlyName mismatch".into());
    }
    Ok(StateChildren {
        super_reference,
        children,
        prefix_bytes: reader.pos,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Package, Vec<u8>) {
        let data = vec![0, 0, 2, 0, 1];
        let owner = Export {
            class: 0,
            super_class: 0,
            outer: 0,
            extra: None,
            name: "Owner".into(),
            flags: 0,
            serial_size: 5,
            serial_offset: 0,
        };
        let child = Export {
            class: 0,
            super_class: 0,
            outer: 1,
            extra: None,
            name: "Child".into(),
            flags: 0,
            serial_size: 0,
            serial_offset: 5,
        };
        (
            Package {
                summary: crate::Summary {
                    version: 159,
                    licensee_version: 1,
                    flags: 0,
                    name_count: 2,
                    name_offset: 0,
                    export_count: 2,
                    export_offset: 0,
                    import_count: 0,
                    import_offset: 0,
                },
                names: vec!["None".into(), "Owner".into()],
                imports: vec![],
                exports: vec![owner, child],
            },
            data,
        )
    }
    #[test]
    fn ordered_prefix_and_truncations() {
        let (mut pkg, data) = fixture();
        let result = read(&pkg, &data, 1).unwrap();
        assert_eq!(result.children[0].export_index, 2);
        assert!(result.children[0].is_struct);
        assert_eq!(result.prefix_bytes, 5);
        for version in [134, 157] {
            pkg.summary.version = version;
            assert_eq!(read(&pkg, &data, 1).unwrap().children[0].export_index, 2);
        }
        for end in 0..data.len() {
            pkg.exports[0].serial_size = end as u32;
            assert!(read(&pkg, &data[..end], 1).is_err());
        }
    }
    #[test]
    fn duplicate_foreign_missing_and_super_mismatch_fail() {
        let (mut pkg, mut data) = fixture();
        assert!(read(&pkg, &data, i32::MIN).is_err());
        pkg.exports[1].outer = 0;
        assert!(read(&pkg, &data, 1).is_err());
        pkg.exports[1].outer = 1;
        data = vec![0, 0, 2, 2, 0, 1];
        pkg.exports[0].serial_size = 6;
        assert!(read(&pkg, &data, 1).is_err());
        data[2] = 3;
        assert!(read(&pkg, &data, 1).is_err());
        pkg.exports[0].super_class = 2;
        assert!(read(&pkg, &data, 1).is_err());
    }
    #[test]
    fn state_prefix_skips_only_empty_tagged_properties() {
        let (mut pkg, mut data) = fixture();
        pkg.imports = vec![
            crate::Import {
                class_package: "Core".into(),
                class_name: "Class".into(),
                outer: -2,
                name: "State".into(),
            },
            crate::Import {
                class_package: "Core".into(),
                class_name: "Package".into(),
                outer: 0,
                name: "Core".into(),
            },
        ];
        pkg.exports[0].class = -1;
        data.insert(0, 0); // Empty UObject tagged properties before UStruct prefix.
        pkg.exports[0].serial_size = data.len() as u32;
        assert_eq!(read(&pkg, &data, 1).unwrap().prefix_bytes, 6);
        data[0] = 1;
        assert!(read(&pkg, &data, 1).is_err());
    }
}
