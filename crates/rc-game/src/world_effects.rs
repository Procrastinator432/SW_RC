//! Selected original factory/prop effects, anchored in the original map.
use crate::{particles, Game};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teleport_action: Option<u32>,
    pub sound: Option<usize>,
    pub ambient: Option<usize>,
    pub original: u32,
    pub source: String,
    pub event: Option<String>,
    pub disabled: Option<u32>,
    pub location: [f32; 3],
    pub rotation: [i32; 3],
    pub templates: Vec<particles::Template>,
}
impl Binding {
    pub fn matrix(&self) -> Result<[[f32; 4]; 4], String> {
        Ok(rc_package::mesh_transform::rotation_matrix(
            self.rotation,
            self.location.map(f32::to_bits),
        )?
        .map(|row| row.map(f32::from_bits)))
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<(u32, [f32; 3], [i32; 3])>,
    pub started: Vec<u32>,
    pub live: Vec<Burst>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Burst {
    pub original: u32,
    pub particles: particles::State,
}
pub fn prepare(bindings: &[Binding], scene: &mut rc_render::Scene) -> Result<Vec<usize>, String> {
    if bindings.len() > 16 {
        return Err("Too many selected world effects".into());
    }
    let mut ids = std::collections::BTreeSet::new();
    let mut bases = vec![];
    for b in bindings {
        if b.original == 0
            || !ids.insert(b.original)
            || b.source.len() > 128
            || b.location.iter().any(|v| !v.is_finite() || v.abs() > 1e7)
            || b.teleport_action
                .is_some_and(|action| b.original != 823 || action != 2395)
            || b.templates.is_empty()
            || b.templates.len() > 16
            || b.templates.iter().any(|t| t.body)
            || b.event.is_some() == b.disabled.is_some()
        {
            return Err("Invalid world effect binding".into());
        }
        b.matrix()?;
        let mut templates = std::collections::BTreeSet::new();
        bases.push(scene.textures.len());
        for t in &b.templates {
            t.validate()?;
            if !templates.insert(t.original) {
                return Err("Duplicate world particle template".into());
            }
            scene.textures.push(rc_render::Texture {
                width: t.material.width,
                height: t.material.height,
                pixels: t.material.pixels.clone(),
            });
        }
    }
    Ok(bases)
}
impl State {
    pub fn relocate(
        &mut self,
        original: u32,
        position: [f32; 3],
        rotation: [i32; 3],
    ) -> Result<(), String> {
        if self.started.contains(&original)
            || self.locations.len() >= 16
            || position.iter().any(|v| !v.is_finite() || v.abs() > 1e7)
        {
            return Err("Invalid effect relocation".into());
        }
        if let Some(entry) = self.locations.iter_mut().find(|(id, _, _)| *id == original) {
            entry.1 = position;
            entry.2 = rotation;
        } else {
            self.locations.push((original, position, rotation));
        }
        Ok(())
    }
    fn anchor(&self, binding: &Binding) -> Result<[[f32; 4]; 4], String> {
        if let Some((_, location, rotation)) = self
            .locations
            .iter()
            .find(|(id, _, _)| *id == binding.original)
        {
            Ok(
                rc_package::mesh_transform::rotation_matrix(*rotation, location.map(f32::to_bits))?
                    .map(|row| row.map(f32::from_bits)),
            )
        } else {
            binding.matrix()
        }
    }

    pub fn validate(&self, bindings: &[Binding]) -> Result<(), String> {
        let ids: std::collections::BTreeSet<_> =
            self.locations.iter().map(|(id, _, _)| *id).collect();
        if ids.len() != self.locations.len()
            || self.locations.len() > bindings.len()
            || self.locations.iter().any(|(id, p, _)| {
                !bindings
                    .iter()
                    .any(|b| b.original == *id && b.teleport_action.is_some())
                    || p.iter().any(|v| !v.is_finite() || v.abs() > 1e7)
            })
        {
            return Err("Invalid relocated effect checkpoint".into());
        }
        if self.started.len() > bindings.len() || self.live.len() > bindings.len() {
            return Err("World effect checkpoint count".into());
        }
        let triggered: std::collections::BTreeSet<_> = self.started.iter().copied().collect();
        let live: std::collections::BTreeSet<_> = self.live.iter().map(|b| b.original).collect();
        if triggered.len() != self.started.len()
            || live.len() != self.live.len()
            || !live.is_subset(&triggered)
            || triggered
                .iter()
                .any(|id| !bindings.iter().any(|b| b.original == *id))
        {
            return Err("World effect checkpoint identity".into());
        }
        if bindings.iter().any(|b| {
            b.teleport_action.is_some()
                && self.started.contains(&b.original)
                && !ids.contains(&b.original)
        }) {
            return Err("Missing captured factory location".into());
        }
        for burst in &self.live {
            let b = bindings
                .iter()
                .find(|b| b.original == burst.original)
                .unwrap();
            burst.particles.validate(&b.templates)?;
            if !burst.particles.active || !burst.particles.bodies.is_empty() {
                return Err("Invalid world burst state".into());
            }
        }
        Ok(())
    }
}
impl Game {
    pub(crate) fn world_ambient_loops(&self) -> Vec<(usize, i32)> {
        if self.state.phase != crate::Phase::Playing {
            return vec![];
        }
        let Some(p) = &self.opening else {
            return vec![];
        };
        let Some(s) = self
            .state
            .opening
            .as_ref()
            .and_then(|o| o.world_effects.as_ref())
        else {
            return vec![];
        };
        s.live
            .iter()
            .filter_map(|burst| {
                p.assets
                    .world_effects
                    .iter()
                    .find(|b| b.original == burst.original)
                    .and_then(|b| b.ambient.map(|role| (role, b.original as i32)))
            })
            .collect()
    }
    pub(crate) fn queue_world_event(&mut self, tag: &str) {
        if !self.opening.as_ref().is_some_and(|o| {
            o.assets
                .world_effects
                .iter()
                .any(|b| b.event.as_deref() == Some(tag))
        }) {
            return;
        }
        if let Some(state) = &mut self.state.opening {
            if state.events.len() >= 256 {
                state.events.remove(0);
            }
            state
                .events
                .push((self.state.statistics.ticks, 0, tag.into()));
        }
    }
    pub(crate) fn world_effect_tick(&mut self) -> Result<(), String> {
        let Some(opening) = &self.opening else {
            return Ok(());
        };
        let state = self.state.opening.as_mut().unwrap();
        let Some(fx) = &mut state.world_effects else {
            return Ok(());
        };
        for b in &opening.assets.world_effects {
            let event = b
                .event
                .as_ref()
                .is_some_and(|tag| state.events.iter().any(|(_, _, event)| event == tag));
            let disabled = b.disabled.is_some_and(|id| {
                self.dynamic
                    .as_ref()
                    .and_then(|d| d.objects.iter().position(|o| o.original == id))
                    .is_some_and(|i| self.state.dynamic.as_ref().is_some_and(|s| s[i]))
            });
            if (event || disabled) && !fx.started.contains(&b.original) {
                fx.started.push(b.original);
                fx.live.push(Burst {
                    original: b.original,
                    particles: particles::State::burst(&b.templates, b.original),
                });
                if let Some(sound) = b.sound {
                    let mission = self
                        .state
                        .mission
                        .as_mut()
                        .ok_or("Missing world effect audio")?;
                    mission.audio_sequence += 1;
                    if mission.events.len() >= 256 {
                        mission.events.remove(0);
                    }
                    mission.events.push(crate::combat::Event {
                        sequence: mission.audio_sequence,
                        tick: self.state.statistics.ticks,
                        sound,
                        owner: b.original as i32,
                    });
                }
            }
        }
        let anchors = opening
            .assets
            .world_effects
            .iter()
            .map(|b| Ok((b.original, fx.anchor(b)?)))
            .collect::<Result<Vec<_>, String>>()?;
        for burst in &mut fx.live {
            let b = opening
                .assets
                .world_effects
                .iter()
                .find(|b| b.original == burst.original)
                .unwrap();
            let anchor = anchors.iter().find(|(id, _)| *id == b.original).unwrap().1;
            burst
                .particles
                .tick(&b.templates, true, &anchor, &anchor, &[]);
        }
        fx.live.retain(|burst| {
            !burst.particles.finished(
                &opening
                    .assets
                    .world_effects
                    .iter()
                    .find(|b| b.original == burst.original)
                    .unwrap()
                    .templates,
            )
        });
        Ok(())
    }
    pub(crate) fn world_effect_geometry(
        &self,
        basis: [[f32; 3]; 4],
    ) -> Result<Vec<rc_render::effects::TriangleEffect>, String> {
        let Some(opening) = &self.opening else {
            return Ok(vec![]);
        };
        let Some(state) = self
            .state
            .opening
            .as_ref()
            .and_then(|o| o.world_effects.as_ref())
        else {
            return Ok(vec![]);
        };
        let mut geometry = vec![];
        for burst in &state.live {
            let i = opening
                .assets
                .world_effects
                .iter()
                .position(|b| b.original == burst.original)
                .ok_or("World effect identity")?;
            let b = &opening.assets.world_effects[i];
            let anchor = state.anchor(b)?;
            geometry.extend(burst.particles.geometry(
                &b.templates,
                opening.world_effect_bases[i],
                &anchor,
                &anchor,
                basis,
            ));
        }
        Ok(geometry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relocation_is_frozen_and_saved_at_original_script_action() {
        let binding = Binding {
            teleport_action: Some(2395),
            sound: None,
            ambient: None,
            original: 823,
            source: "CTEffects.Explosion_Dirt_lrg".into(),
            event: Some("PlayerExplosionFactory".into()),
            disabled: None,
            location: [3000., 2100., -1873.],
            rotation: [0; 3],
            templates: vec![],
        };
        let mut state = State::default();
        state
            .relocate(823, [20., 30., -2000.], [0, 16384, 0])
            .unwrap();
        let saved = serde_json::to_vec(&state).unwrap();
        let restored: State = serde_json::from_slice(&saved).unwrap();
        assert_eq!(
            restored.anchor(&binding).unwrap()[3][..3],
            [20., 30., -2000.]
        );
        let matrix = restored.anchor(&binding).unwrap();
        assert!(matrix[0][0].abs() < 0.001 && (matrix[0][1] - 1.).abs() < 0.001);
        state.started.push(823);
        assert!(state.relocate(823, [40.; 3], [0; 3]).is_err());
        assert!(State::default()
            .relocate(823, [f32::NAN; 3], [0; 3])
            .is_err());
    }
}
