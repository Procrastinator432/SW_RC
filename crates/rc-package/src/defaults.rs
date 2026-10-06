//! Scalar default resolution using observed native initialization/overlay order.
use crate::{
    classes::Field,
    properties::{Properties, Value},
};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

pub struct Class {
    pub name: String,
    pub parent: Option<String>,
    pub package: String,
    pub source: String,
    pub properties: Properties,
    pub fields: Vec<Field>,
}
#[derive(Default)]
pub struct Catalog {
    classes: HashMap<String, Class>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Resolved {
    pub value: Value,
    pub declaring_class: String,
    pub source_package: String,
    pub stage: String,
    pub payload_offset: Option<usize>,
    pub external_config_or_localization: bool,
}

impl Catalog {
    pub fn insert(&mut self, class: Class) -> Result<(), String> {
        let key = class.name.to_lowercase();
        if self.classes.contains_key(&key) {
            return Err(format!("duplicate class {}", class.name));
        }
        self.classes.insert(key, class);
        Ok(())
    }
    pub fn len(&self) -> usize {
        self.classes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.classes.is_empty()
    }
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.classes.values().map(|c| c.name.as_str())
    }

    /// Follow serialized ancestry; never guess inheritance from class names.
    pub fn derives_from(&self, class: &str, base: &str) -> Result<bool, String> {
        let mut next = Some(class.to_lowercase());
        let mut seen = HashSet::new();
        while let Some(key) = next {
            if !seen.insert(key.clone()) || seen.len() > 4096 {
                return Err("class ancestry cycle/limit".into());
            }
            let entry = self
                .classes
                .get(&key)
                .ok_or_else(|| format!("missing class {key}"))?;
            if entry.name.eq_ignore_ascii_case(base) {
                return Ok(true);
            }
            next = entry.parent.as_ref().map(|p| p.to_lowercase());
        }
        Ok(false)
    }

    pub fn resolve(&self, class: &str, property: &str, index: u32) -> Result<Resolved, String> {
        let mut chain = Vec::new();
        let mut seen = HashSet::new();
        let mut next = Some(class.to_lowercase());
        while let Some(key) = next {
            if !seen.insert(key.clone()) || seen.len() > 4096 {
                return Err("class ancestry cycle/limit".into());
            }
            let class = self
                .classes
                .get(&key)
                .ok_or_else(|| format!("missing class {key}"))?;
            next = class.parent.as_ref().map(|p| p.to_lowercase());
            chain.push(class);
        }
        let mut result = None;
        let mut external = false;
        for class in chain.into_iter().rev() {
            if let Some(field) = class
                .fields
                .iter()
                .find(|f| f.name.eq_ignore_ascii_case(property))
            {
                if index >= field.dimension {
                    return Err("property index exceeds ArrayDim".into());
                }
                external |= field.flags & (0x4000 | 0x8000) != 0; // CPF_Config / CPF_Localized
                result = zero(field).map(|value| Resolved {
                    value,
                    declaring_class: class.name.clone(),
                    source_package: class.source.clone(),
                    stage: "zero_initialization".into(),
                    payload_offset: None,
                    external_config_or_localization: external,
                });
            }
            apply(class, property, index, "class_delta", external, &mut result)?;
            let short = class.name.rsplit('.').next().ok_or("invalid class path")?;
            let special = class
                .parent
                .as_ref()
                .and_then(|p| p.rsplit('.').next())
                .is_some_and(|p| short.eq_ignore_ascii_case(&format!("{p}Defaults")));
            if !special {
                if let Some(overlay) = self
                    .classes
                    .get(&format!("properties.{short}defaults").to_lowercase())
                {
                    if !overlay
                        .parent
                        .as_ref()
                        .is_some_and(|p| p.eq_ignore_ascii_case(&class.name))
                    {
                        return Err(format!("override {} has unexpected parent", overlay.name));
                    }
                    apply(
                        overlay,
                        property,
                        index,
                        "properties_override",
                        external,
                        &mut result,
                    )?;
                }
            }
        }
        let mut resolved = result
            .ok_or_else(|| format!("no supported definition/value for {class}.{property}"))?;
        resolved.external_config_or_localization = external;
        Ok(resolved)
    }

    pub fn instance(
        &self,
        class: &str,
        props: &Properties,
        property: &str,
        index: u32,
        source: &str,
    ) -> Result<Resolved, String> {
        let mut result = self.resolve(class, property, index)?;
        if let Some(tag) = props
            .values
            .iter()
            .rev()
            .find(|p| p.name.eq_ignore_ascii_case(property) && p.array_index == index)
        {
            scalar(&tag.value)?;
            if std::mem::discriminant(&result.value) != std::mem::discriminant(&tag.value) {
                return Err("instance property type differs from definition".into());
            }
            result.value = tag.value.clone();
            result.stage = "instance_delta".into();
            result.source_package = source.into();
            result.payload_offset = Some(tag.payload_offset);
        }
        if result.external_config_or_localization {
            return Err(format!(
                "{property} requires config/localization resolution"
            ));
        }
        Ok(result)
    }
}

fn apply(
    class: &Class,
    property: &str,
    index: u32,
    stage: &str,
    external: bool,
    result: &mut Option<Resolved>,
) -> Result<(), String> {
    if let Some(tag) = class
        .properties
        .values
        .iter()
        .rev()
        .find(|p| p.name.eq_ignore_ascii_case(property) && p.array_index == index)
    {
        scalar(&tag.value)?;
        let previous = result
            .as_ref()
            .ok_or("serialized scalar has no supported field definition")?;
        if std::mem::discriminant(&previous.value) != std::mem::discriminant(&tag.value) {
            return Err("default property type differs from definition".into());
        }
        let mut value = tag.value.clone();
        if let Value::Object { index, path } = &mut value {
            if *index > 0 {
                *path = format!("{}.{path}", class.package);
            }
        }
        *result = Some(Resolved {
            value,
            declaring_class: class.name.clone(),
            source_package: class.source.clone(),
            stage: stage.into(),
            payload_offset: Some(tag.payload_offset),
            external_config_or_localization: external,
        });
    }
    Ok(())
}
fn scalar(value: &Value) -> Result<(), String> {
    if matches!(value, Value::Raw { .. } | Value::StructArray(_)) {
        return Err("compound default merging is unsupported".into());
    }
    Ok(())
}
fn zero(field: &Field) -> Option<Value> {
    match field.kind.to_lowercase().as_str() {
        "core.boolproperty" => Some(Value::Bool(false)),
        "core.byteproperty" => Some(Value::Byte(0)),
        "core.intproperty" => Some(Value::Int(0)),
        "core.floatproperty" => Some(Value::Float(0.0)),
        "core.nameproperty" => Some(Value::Name("None".into())),
        "core.strproperty" => Some(Value::String(String::new())),
        "core.objectproperty" | "core.classproperty" => Some(Value::Object {
            index: 0,
            path: "None".into(),
        }),
        "core.structproperty" => match field
            .struct_name
            .as_ref()?
            .rsplit('.')
            .next()?
            .to_lowercase()
            .as_str()
        {
            "vector" => Some(Value::Vector([0.0; 3])),
            "rotator" => Some(Value::Rotator([0; 3])),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::properties::Property;
    fn props(value: Option<Value>) -> Properties {
        Properties {
            native_offset: 0,
            values: value
                .into_iter()
                .map(|value| Property {
                    name: "Health".into(),
                    kind: 2,
                    array_index: 0,
                    struct_name: None,
                    bytes: 4,
                    declared_bytes: 4,
                    payload_offset: 17,
                    value,
                })
                .collect(),
        }
    }
    fn class(name: &str, parent: Option<&str>, value: Option<Value>) -> Class {
        let fields = match &value {
            Some(Value::Int(_)) => vec![Field {
                name: "Health".into(),
                kind: "Core.IntProperty".into(),
                dimension: 1,
                flags: 0,
                struct_name: None,
            }],
            Some(Value::Object { .. }) => vec![Field {
                name: "Health".into(),
                kind: "Core.ObjectProperty".into(),
                dimension: 1,
                flags: 0,
                struct_name: None,
            }],
            _ => vec![],
        };
        Class {
            name: name.into(),
            parent: parent.map(String::from),
            package: name.split('.').next().unwrap().into(),
            source: format!("{name}.u"),
            properties: props(value),
            fields,
        }
    }
    #[test]
    fn ancestry_uses_metadata_and_rejects_missing_links_and_cycles() {
        let mut c = Catalog::default();
        c.insert(class("Engine.Actor", None, None)).unwrap();
        c.insert(class("Game.UnrelatedName", Some("Engine.Actor"), None))
            .unwrap();
        c.insert(class("Game.StaticMeshNamedObject", None, None))
            .unwrap();
        assert!(c
            .derives_from("game.unrelatedname", "engine.actor")
            .unwrap());
        assert!(!c
            .derives_from("Game.StaticMeshNamedObject", "Engine.Actor")
            .unwrap());
        c.insert(class("Game.Broken", Some("Missing.Parent"), None))
            .unwrap();
        assert!(c.derives_from("Game.Broken", "Engine.Actor").is_err());
        c.insert(class("Game.Cycle", Some("Game.Cycle"), None))
            .unwrap();
        assert!(c.derives_from("Game.Cycle", "Engine.Actor").is_err());
    }
    #[test]
    fn parent_overlay_child_delta_child_overlay_and_instance_order() {
        let mut catalog = Catalog::default();
        let mut base = class("Engine.Base", None, Some(Value::Int(10)));
        base.fields.push(Field {
            name: "Health".into(),
            kind: "Core.IntProperty".into(),
            dimension: 1,
            flags: 0,
            struct_name: None,
        });
        catalog.insert(base).unwrap();
        catalog
            .insert(class(
                "Properties.BaseDefaults",
                Some("Engine.Base"),
                Some(Value::Int(20)),
            ))
            .unwrap();
        catalog
            .insert(class("Engine.Child", Some("Engine.Base"), None))
            .unwrap();
        assert!(matches!(
            catalog.resolve("engine.child", "health", 0).unwrap().value,
            Value::Int(20)
        ));
        catalog.classes.get_mut("engine.child").unwrap().properties = props(Some(Value::Int(30)));
        assert!(matches!(
            catalog.resolve("Engine.Child", "Health", 0).unwrap().value,
            Value::Int(30)
        ));
        catalog
            .insert(class(
                "Properties.ChildDefaults",
                Some("Engine.Child"),
                Some(Value::Int(40)),
            ))
            .unwrap();
        catalog
            .insert(class("Engine.Grandchild", Some("Engine.Child"), None))
            .unwrap();
        let resolved = catalog.resolve("Engine.Grandchild", "Health", 0).unwrap();
        assert!(matches!(resolved.value, Value::Int(40)));
        assert_eq!(resolved.declaring_class, "Properties.ChildDefaults");
        assert_eq!(resolved.stage, "properties_override");
        let explicit = catalog
            .instance(
                "Engine.Grandchild",
                &props(Some(Value::Int(50))),
                "Health",
                0,
                "map.ctm",
            )
            .unwrap();
        assert!(matches!(explicit.value, Value::Int(50)));
        assert_eq!(explicit.stage, "instance_delta");
        assert_eq!(explicit.source_package, "map.ctm");
    }
    #[test]
    fn zero_requires_definition_and_config_values_are_not_runtime_complete() {
        let mut catalog = Catalog::default();
        let mut base = class("Engine.Base", None, None);
        base.fields.push(Field {
            name: "Rotation".into(),
            kind: "Core.StructProperty".into(),
            dimension: 1,
            flags: 0,
            struct_name: Some("Core.Rotator".into()),
        });
        base.fields.push(Field {
            name: "Configured".into(),
            kind: "Core.IntProperty".into(),
            dimension: 1,
            flags: 0x4000,
            struct_name: None,
        });
        catalog.insert(base).unwrap();
        assert!(matches!(
            catalog.resolve("Engine.Base", "Rotation", 0).unwrap().value,
            Value::Rotator([0, 0, 0])
        ));
        assert!(catalog.resolve("Engine.Base", "Missing", 0).is_err());
        assert!(catalog.resolve("Engine.Base", "Rotation", 1).is_err());
        assert!(
            catalog
                .resolve("Engine.Base", "Configured", 0)
                .unwrap()
                .external_config_or_localization
        );
        assert!(catalog
            .instance("Engine.Base", &props(None), "Configured", 0, "map")
            .is_err());
        let mut bad_type = props(Some(Value::Int(3)));
        bad_type.values[0].name = "Rotation".into();
        assert!(catalog
            .instance("Engine.Base", &bad_type, "Rotation", 0, "map")
            .is_err());
    }
    #[test]
    fn cycles_missing_parents_wrong_overlay_and_compound_values_fail() {
        let mut catalog = Catalog::default();
        catalog
            .insert(class("Engine.A", Some("Engine.B"), None))
            .unwrap();
        assert!(catalog.resolve("Engine.A", "Health", 0).is_err());
        catalog
            .insert(class("Engine.B", Some("Engine.A"), None))
            .unwrap();
        assert!(catalog.resolve("Engine.A", "Health", 0).is_err());
        let mut catalog = Catalog::default();
        catalog
            .insert(class("Engine.A", None, Some(Value::Int(1))))
            .unwrap();
        catalog
            .insert(class("Properties.ADefaults", Some("Engine.Other"), None))
            .unwrap();
        assert!(catalog.resolve("Engine.A", "Health", 0).is_err());
        let mut catalog = Catalog::default();
        catalog
            .insert(class(
                "Engine.A",
                None,
                Some(Value::Raw { prefix: vec![0] }),
            ))
            .unwrap();
        assert!(catalog.resolve("Engine.A", "Health", 0).is_err());
    }
    #[test]
    fn local_object_paths_keep_declaring_package_and_null_defaults_stay_null() {
        let mut catalog = Catalog::default();
        catalog
            .insert(class(
                "Engine.A",
                None,
                Some(Value::Object {
                    index: 1,
                    path: "Resource".into(),
                }),
            ))
            .unwrap();
        let resolved = catalog.resolve("Engine.A", "Health", 0).unwrap();
        let Value::Object { index, path } = resolved.value else {
            panic!("expected object")
        };
        assert_eq!(index, 1);
        assert_eq!(path, "Engine.Resource");
        let mut null_class = class("Engine.Null", None, None);
        null_class.fields.push(Field {
            name: "Health".into(),
            kind: "Core.ObjectProperty".into(),
            dimension: 1,
            flags: 0,
            struct_name: None,
        });
        catalog.insert(null_class).unwrap();
        let Value::Object { index, path } =
            catalog.resolve("Engine.Null", "Health", 0).unwrap().value
        else {
            panic!("expected null object")
        };
        assert_eq!(index, 0);
        assert_eq!(path, "None");
    }
}
