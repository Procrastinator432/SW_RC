//! Explicit map anchors. Class defaults and game spawn selection are not inferred.
use crate::{
    properties::{self, Value},
    Package,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct WorldBinding {
    pub level_export: usize,
    pub model_export: usize,
    pub model_path: String,
    pub actor_slots: usize,
    pub decoded_bytes: usize,
    pub remaining_bytes: usize,
}
/// ULevelBase/ULevel serialized prefix through the Model reference, not a full level reader.
pub fn world_binding(pkg: &Package, data: &[u8]) -> Result<WorldBinding, String> {
    if !(151..=159).contains(&pkg.summary.version) || pkg.summary.licensee_version > 1 {
        return Err("unsupported Level prefix version".into());
    }
    let mut levels = pkg
        .exports
        .iter()
        .enumerate()
        .filter(|(_, e)| pkg.object_path(e.class).as_deref() == Ok("Engine.Level"));
    let (level_export, export) = levels.next().ok_or("map has no Engine.Level export")?;
    if levels.next().is_some() {
        return Err("map has ambiguous Engine.Level exports".into());
    }
    let payload = pkg.payload(data, export)?;
    let properties = properties::read(pkg, data, export)?;
    let mut reader = crate::Reader::at(payload, properties.native_offset)?;
    if pkg.summary.version >= 153 {
        pkg.object_path(reader.index()?)?;
    }
    let actor_slots = reader.count(1)?;
    for _ in 0..actor_slots {
        pkg.object_path(reader.index()?)?;
    }
    // FURL: protocol, host, map, portal; option strings; port and Valid.
    for _ in 0..4 {
        reader.string()?;
    }
    let options = reader.count(1)?;
    for _ in 0..options {
        reader.string()?;
    }
    reader.u32()?;
    if reader.u32()? > 1 {
        return Err("invalid Level URL Valid flag".into());
    }
    let model = reader.index()?;
    let model_export = model
        .checked_sub(1)
        .and_then(|index| usize::try_from(index).ok())
        .ok_or("Level world Model is not a local export")?;
    let model_object = pkg
        .exports
        .get(model_export)
        .ok_or("Level world Model reference out of range")?;
    if pkg.object_path(model_object.class)? != "Engine.Model" {
        return Err("Level world reference is not Engine.Model".into());
    }
    Ok(WorldBinding {
        level_export,
        model_export,
        model_path: pkg.object_path(model)?,
        actor_slots,
        decoded_bytes: reader.pos,
        remaining_bytes: payload.len() - reader.pos,
    })
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PlayerStart {
    pub actor: String,
    pub location: [f32; 3],
    /// Unreal pitch, yaw, roll in 65536 units per turn.
    pub rotation: [i32; 3],
}

/// Exact Engine.PlayerStart exports with explicitly serialized Location and Rotation.
/// Missing transforms and subclasses are deliberately omitted.
pub fn player_starts(pkg: &Package, data: &[u8]) -> Result<Vec<PlayerStart>, String> {
    let mut result = Vec::new();
    for (i, export) in pkg.exports.iter().enumerate() {
        if pkg.object_path(export.class)? != "Engine.PlayerStart" {
            continue;
        }
        let props = properties::read(pkg, data, export)?;
        let value = |name| {
            props
                .values
                .iter()
                .find(|p| p.name == name && p.array_index == 0)
                .map(|p| &p.value)
        };
        if let (Some(Value::Vector(location)), Some(Value::Rotator(rotation))) =
            (value("Location"), value("Rotation"))
        {
            if location.iter().any(|v| !v.is_finite() || v.abs() > 1e9) {
                return Err("Invalid PlayerStart position".into());
            }
            result.push(PlayerStart {
                actor: pkg.object_path(i as i32 + 1)?,
                location: *location,
                rotation: *rotation,
            });
        }
    }
    Ok(result)
}

/// PC/offline import with definitions from System and Properties packages.
/// Resolves transforms only; game spawn selection and enabled-state filtering
/// remain separate. Direct Android CTM import still uses explicit tags.
pub fn player_starts_with_defaults(
    pkg: &Package,
    data: &[u8],
    catalog: &crate::defaults::Catalog,
    source: &str,
) -> Result<Vec<PlayerStart>, String> {
    let mut starts = Vec::new();
    for (i, export) in pkg.exports.iter().enumerate() {
        let class = pkg.object_path(export.class)?;
        if !class.eq_ignore_ascii_case("Engine.PlayerStart") {
            continue;
        }
        let props = properties::read(pkg, data, export)?;
        let Value::Vector(location) = catalog
            .instance(&class, &props, "Location", 0, source)?
            .value
        else {
            return Err("PlayerStart Location is not a vector".into());
        };
        let Value::Rotator(rotation) = catalog
            .instance(&class, &props, "Rotation", 0, source)?
            .value
        else {
            return Err("PlayerStart Rotation is not a rotator".into());
        };
        if location.iter().any(|v| !v.is_finite() || v.abs() > 1e9) {
            return Err("Invalid PlayerStart position".into());
        }
        starts.push(PlayerStart {
            actor: pkg.object_path(i as i32 + 1)?,
            location,
            rotation,
        });
    }
    Ok(starts)
}

#[cfg(test)]
mod binding_tests {
    use super::*;
    use crate::{Export, Import, Summary};
    fn fixture(version: u16) -> (Package, Vec<u8>) {
        let mut data = vec![0];
        if version >= 153 {
            data.push(0);
        }
        data.extend([2, 0, 1]); // two actor slots: null, local reference
        data.extend([0, 0, 0, 0, 0]); // four empty FURL strings and empty options
        data.extend(7777u32.to_le_bytes());
        data.extend(1u32.to_le_bytes());
        data.push(2);
        let export = |name: &str, class: i32, size: u32| Export {
            class,
            super_class: 0,
            outer: 0,
            extra: None,
            name: name.into(),
            flags: 0,
            serial_size: size,
            serial_offset: 0,
        };
        let import = |name: &str, outer: i32| Import {
            class_package: "Core".into(),
            class_name: "Class".into(),
            outer,
            name: name.into(),
        };
        (
            Package {
                summary: Summary {
                    version,
                    licensee_version: 1,
                    flags: 0,
                    name_count: 1,
                    name_offset: 0,
                    export_count: 2,
                    export_offset: 0,
                    import_count: 3,
                    import_offset: 0,
                },
                names: vec!["None".into()],
                imports: vec![
                    import("Engine", 0),
                    import("Model", -1),
                    import("Level", -1),
                ],
                exports: vec![
                    export("LevelRenamed", -3, data.len() as u32),
                    export("WorldRenamed", -2, 0),
                ],
            },
            data,
        )
    }
    #[test]
    fn level_binding_versions_renamed_objects_and_every_truncation() {
        for version in [151, 152, 153, 159] {
            let (mut pkg, data) = fixture(version);
            let binding = world_binding(&pkg, &data).unwrap();
            assert_eq!(binding.model_path, "WorldRenamed");
            assert_eq!(binding.actor_slots, 2);
            assert_eq!(binding.model_export, 1);
            for end in 0..data.len() {
                pkg.exports[0].serial_size = end as u32;
                assert!(world_binding(&pkg, &data[..end]).is_err());
            }
        }
    }
    #[test]
    fn level_binding_rejects_bad_model_url_and_ambiguous_levels() {
        let (mut pkg, mut data) = fixture(159);
        let last = data.len() - 1;
        for value in [0, 1, 3, 0x81] {
            data[last] = value;
            assert!(world_binding(&pkg, &data).is_err());
        }
        data[last] = 2;
        data[last - 4] = 2;
        assert!(world_binding(&pkg, &data).is_err());
        data[last - 4] = 1;
        let mut minimum = data[..last].to_vec();
        minimum.extend([0xc0, 0x80, 0x80, 0x80, 0x10]);
        pkg.exports[0].serial_size = minimum.len() as u32;
        assert!(world_binding(&pkg, &minimum).is_err());
        pkg.exports[0].serial_size = data.len() as u32;
        pkg.exports[1].class = -3;
        assert!(world_binding(&pkg, &data).is_err());
    }
}
