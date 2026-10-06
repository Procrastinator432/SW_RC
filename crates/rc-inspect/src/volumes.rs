//! Shared static PhysicsVolume loader for audits and combined movement probes.
use rc_package::{
    collision,
    defaults::Catalog,
    geometry,
    mesh_query::ActorTransform,
    physics::PhysicsOptions,
    properties::{self, Value},
    volumes::*,
    Package,
};
pub struct LoadedVolumes {
    pub world: VolumeWorld,
    pub actors: Vec<serde_json::Value>,
    pub unidentified: Vec<String>,
    pub errors: usize,
}
pub fn load(
    pkg: &Package,
    data: &[u8],
    catalog: &Catalog,
    source: &str,
    physics: PhysicsOptions,
) -> Result<LoadedVolumes, String> {
    let mut failures = 0;
    let settings =
        |value: &dyn Fn(&str) -> Result<Value, String>| -> Result<VolumeSettings, String> {
            let vector = |name| match value(name)? {
                Value::Vector(v) => Ok::<[f32; 3], String>(v),
                _ => Err(format!("{name} not Vector")),
            };
            let float = |name| match value(name)? {
                Value::Float(v) if v.is_finite() => Ok(v),
                _ => Err(format!("{name} not finite Float")),
            };
            let water = match value("bWaterVolume")? {
                Value::Bool(v) => v,
                _ => return Err("bWaterVolume not Bool".into()),
            };
            Ok(VolumeSettings {
                gravity: vector("Gravity")?,
                terminal_speed: float("TerminalVelocity")?,
                ground_friction: float("GroundFriction")?,
                water,
                zone_velocity: vector("ZoneVelocity")?,
            })
        };
    let default = settings(&|name| {
        Ok(catalog
            .resolve("Engine.DefaultPhysicsVolume", name, 0)?
            .value)
    })?;
    let mut world = VolumeWorld {
        default: default.clone(),
        volumes: vec![],
    };
    let mut actors = Vec::new();
    let mut unidentified = Vec::new();
    for (i, e) in pkg.exports.iter().enumerate() {
        let class = pkg.object_path(e.class)?;
        let derived = match catalog.derives_from(&class, "Engine.PhysicsVolume") {
            Ok(v) => v,
            Err(_) => {
                if class.to_ascii_lowercase().contains("volume") {
                    unidentified.push(class);
                }
                continue;
            }
        };
        if !derived {
            continue;
        }

        let name = pkg.object_path(i as i32 + 1)?;
        let result = (|| -> Result<PhysicsVolume, String> {
            let props = properties::read(pkg, data, e)?;
            let value = |field: &str| Ok(catalog.instance(&class, &props, field, 0, source)?.value);
            let config = settings(&value)?;
            let priority = match value("Priority")? {
                Value::Int(v) => v,
                _ => return Err("Priority not Int".into()),
            };
            let shape = (|| -> Result<VolumeShape, String> {
                let brush = catalog.instance(&class, &props, "Brush", 0, source)?;
                let index = match brush.value {
                    Value::Object { index, .. } => index,
                    _ => return Err("Brush not Object".into()),
                };
                if index == 0 {
                    return Ok(VolumeShape::Empty);
                }
                if index < 0 || brush.stage != "instance_delta" {
                    return Err("volume Brush is not a local instance Model".into());
                }
                let model = pkg
                    .exports
                    .get(index as usize - 1)
                    .ok_or("invalid Brush export")?;
                if !pkg
                    .object_path(model.class)?
                    .eq_ignore_ascii_case("Engine.Model")
                {
                    return Err("Brush not Model".into());
                }
                let bsp = geometry::read_bsp(pkg, data, model)?;
                let solid = collision::read_model_collision(pkg, data, model, &bsp)?;
                let vector = |field| match value(field)? {
                    Value::Vector(v) => Ok::<[f32; 3], String>(v),
                    _ => Err("transform not Vector".into()),
                };
                let rotation = match value("Rotation")? {
                    Value::Rotator(v) => v,
                    _ => return Err("Rotation not Rotator".into()),
                };
                let scale = match value("DrawScale")? {
                    Value::Float(v) => v,
                    _ => return Err("DrawScale not Float".into()),
                };
                Ok(VolumeShape::Brush {
                    bsp: Box::new(bsp),
                    solid,
                    transform: ActorTransform {
                        location: vector("Location")?,
                        rotation,
                        scale: vector("DrawScale3D")?.map(|v| v * scale),
                        pivot: vector("PrePivot")?,
                    },
                })
            })()
            .unwrap_or_else(VolumeShape::Unsupported);
            Ok(PhysicsVolume {
                name: name.clone(),
                priority,
                settings: config,
                shape,
            })
        })();
        match result {
            Ok(volume) => {
                failures += usize::from(matches!(volume.shape, VolumeShape::Unsupported(_)));
                actors.push(serde_json::json!({"actor":name,"class":class,"priority":volume.priority,"settings":volume.settings,"pc_physics_policy":volume.settings.apply(physics),"shape_error":if let VolumeShape::Unsupported(e)=&volume.shape {Some(e)} else {None}}));
                world.volumes.push(volume);
            }
            Err(error) => {
                failures += 1;
                actors.push(serde_json::json!({"actor":name,"class":class,"error":error}));
                world.volumes.push(PhysicsVolume {
                    name,
                    priority: i32::MAX,
                    settings: default.clone(),
                    shape: VolumeShape::Unsupported(error),
                });
            }
        }
    }
    Ok(LoadedVolumes {
        world,
        actors,
        unidentified,
        errors: failures,
    })
}
