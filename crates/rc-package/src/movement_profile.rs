//! Provenance-preserving original class defaults applied to an explicit PC policy.
use crate::{
    controller::{ControllerOptions, MovementRatios},
    defaults::{Catalog, Resolved},
    physics::PhysicsOptions,
    properties::Value,
};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub struct MovementProfile {
    pub controller: ControllerOptions,
    pub ratios: MovementRatios,
    pub physics: PhysicsOptions,
    pub properties: BTreeMap<String, Resolved>,
}
fn number(properties: &BTreeMap<String, Resolved>, name: &str) -> Result<f64, String> {
    match properties.get(name).map(|v| &v.value) {
        Some(Value::Float(v)) if v.is_finite() => Ok(f64::from(*v)),
        _ => Err(format!("invalid/missing movement property {name}")),
    }
}
impl MovementProfile {
    pub fn read(catalog: &Catalog) -> Result<Self, String> {
        let mut properties = BTreeMap::new();
        for (class, names) in [
            (
                "CTCharacters.PlayerCommando",
                vec![
                    "GroundSpeed",
                    "AccelRate",
                    "DecelRate",
                    "AirControl",
                    "JumpZ",
                    "WalkSpeedRatio",
                    "BackSpeedRatio",
                    "SideSpeedRatio",
                    "CollisionRadius",
                    "CollisionHeight",
                    "MaxFallSpeed",
                ],
            ),
            (
                "Engine.PhysicsVolume",
                vec!["Gravity", "TerminalVelocity", "GroundFriction"],
            ),
        ] {
            for name in names {
                properties.insert(name.into(), catalog.resolve(class, name, 0)?);
            }
        }
        Self::from_properties(properties)
    }
    fn from_properties(properties: BTreeMap<String, Resolved>) -> Result<Self, String> {
        let gravity = match properties.get("Gravity").map(|v| &v.value) {
            Some(Value::Vector(v))
                if v.iter().all(|x| x.is_finite()) && v[0] == 0.0 && v[1] == 0.0 && v[2] < 0.0 =>
            {
                -f64::from(v[2])
            }
            _ => return Err("PC gravity policy requires a downward-only volume default".into()),
        };
        let controller = ControllerOptions {
            speed: number(&properties, "GroundSpeed")?,
            acceleration: number(&properties, "AccelRate")?,
            braking: number(&properties, "DecelRate")?,
            air_control: number(&properties, "AirControl")?,
            jump_speed: number(&properties, "JumpZ")?,
        };
        let ratios = MovementRatios {
            walk: number(&properties, "WalkSpeedRatio")?,
            back: number(&properties, "BackSpeedRatio")?,
            side: number(&properties, "SideSpeedRatio")?,
        };
        let radius = number(&properties, "CollisionRadius")?;
        let height = number(&properties, "CollisionHeight")?;
        let terminal = number(&properties, "TerminalVelocity")?;
        if [
            controller.speed,
            controller.acceleration,
            controller.braking,
            controller.jump_speed,
            radius,
            height,
            terminal,
        ]
        .iter()
        .any(|v| *v <= 0.0 || *v > 1e6)
            || gravity > 10000.0
            || !(0.0..=1.0).contains(&controller.air_control)
            || [ratios.walk, ratios.back, ratios.side]
                .iter()
                .any(|v| !(0.0..=1.0).contains(v))
        {
            return Err("unsupported original movement parameter range".into());
        }
        // MAXSTEPHEIGHT/MINFLOORZ from recovered Engine.Actor source; others are own policy.
        let physics = PhysicsOptions {
            extent: [radius, radius, height],
            gravity,
            terminal_speed: terminal,
            step_height: 35.0,
            minimum_up: 0.7,
            skin: 0.5,
            support_distance: 2.0,
            max_iterations: 8,
        };
        Ok(Self {
            controller,
            ratios,
            physics,
            properties,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_wrong_typed_and_nonvertical_gravity_do_not_fall_back() {
        let mut properties = BTreeMap::new();
        let field = |value| Resolved {
            value,
            declaring_class: "fixture".into(),
            source_package: "fixture".into(),
            stage: "class_delta".into(),
            payload_offset: None,
            external_config_or_localization: false,
        };
        assert!(MovementProfile::from_properties(properties.clone()).is_err());
        properties.insert("Gravity".into(), field(Value::Vector([1.0, 0.0, -100.0])));
        assert!(MovementProfile::from_properties(properties.clone()).is_err());
        properties.insert("Gravity".into(), field(Value::Vector([0.0, 0.0, -100.0])));
        for name in [
            "GroundSpeed",
            "AccelRate",
            "DecelRate",
            "JumpZ",
            "CollisionRadius",
            "CollisionHeight",
            "TerminalVelocity",
            "MaxFallSpeed",
            "GroundFriction",
        ] {
            properties.insert(name.into(), field(Value::Float(10.0)));
        }
        for name in [
            "AirControl",
            "WalkSpeedRatio",
            "BackSpeedRatio",
            "SideSpeedRatio",
        ] {
            properties.insert(name.into(), field(Value::Float(0.5)));
        }
        let p = MovementProfile::from_properties(properties.clone()).unwrap();
        assert_eq!(p.physics.terminal_speed, 10.0);
        assert_eq!(p.physics.gravity, 100.0);
        properties.insert("TerminalVelocity".into(), field(Value::Float(0.0)));
        assert!(MovementProfile::from_properties(properties).is_err());
    }
}
