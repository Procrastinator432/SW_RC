//! Validated scene audio geometry and persistent, position-keyed region caches.
use rc_package::{
    audio_geometry::{Bsp, HearingPath, Region, SavedRadii},
    audio_zones::Distances,
    world_audio::Radii,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Actor {
    pub owner: i32,
    pub mode: u8,
    pub mover: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assets {
    pub bsp: Bsp,
    pub connectivity: Vec<u64>,
    pub distances: Distances,
    pub level_info: i32,
    pub actors: Vec<Actor>,
}
impl Assets {
    pub fn validate(&self) -> Result<(), String> {
        self.bsp.validate()?;
        self.distances.validate()?;
        if self.connectivity.len() != self.bsp.zone_actors.len()
            || self.distances != Distances::build(&self.connectivity)?
            || self.level_info <= 0
            || self.actors.len() > 16
        {
            return Err("Invalid scene audio geometry".into());
        }
        let mut owners = std::collections::BTreeSet::new();
        for actor in &self.actors {
            if actor.owner <= 0 || !owners.insert(actor.owner) {
                return Err("Invalid audio geometry actor".into());
            }
            if rc_package::audio_geometry::hearing_path(Some(actor.mode), actor.mover)
                == HearingPath::GeneralTrace
            {
                return Err(
                    "Audio StaticMeshes mode requires the unported general trace host".into(),
                );
            }
        }
        Ok(())
    }
    pub fn regions(
        &self,
        source: [f32; 3],
        listener: [f32; 3],
    ) -> Result<(CachedRegion, CachedRegion), String> {
        Ok((
            CachedRegion::resolve(&self.bsp, source, self.level_info, None)?,
            CachedRegion::resolve(&self.bsp, listener, self.level_info, None)?,
        ))
    }
    pub fn query(
        &self,
        actor: &Actor,
        source: &CachedRegion,
        listener: &CachedRegion,
        bypass: bool,
    ) -> Result<Hearing, String> {
        let clear = bypass
            || match rc_package::audio_geometry::hearing_path(Some(actor.mode), actor.mover) {
                HearingPath::Bypass => true,
                HearingPath::Bsp => self.bsp.clear(source.position(), listener.position())?,
                HearingPath::GeneralTrace => {
                    return Err("General audio trace is not available".into())
                }
            };
        let distance = self
            .distances
            .get(source.region.zone, listener.region.zone)?;
        Ok(Hearing { clear, distance })
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CachedRegion {
    pub position_bits: [u32; 3],
    pub region: Region,
}
impl CachedRegion {
    pub fn position(&self) -> [f32; 3] {
        self.position_bits.map(f32::from_bits)
    }
    pub fn resolve(
        bsp: &Bsp,
        position: [f32; 3],
        level: i32,
        old: Option<&Self>,
    ) -> Result<Self, String> {
        let bits = position.map(f32::to_bits);
        if let Some(old) = old.filter(|c| c.position_bits == bits) {
            return Ok(old.clone());
        }
        Ok(Self {
            position_bits: bits,
            region: bsp.region(position, level)?,
        })
    }
    pub fn validate(&self, assets: &Assets) -> Result<(), String> {
        if self.region != assets.bsp.region(self.position(), assets.level_info)? {
            return Err("Saved audio region differs from original BSP".into());
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Hearing {
    pub clear: bool,
    pub distance: u8,
}
impl Hearing {
    pub fn factor(self) -> f32 {
        if self.clear {
            1.
        } else {
            rc_package::audio_geometry::zone_multiplier(self.distance)
        }
    }
    pub fn update(self, old: &Radii, base: f32, now: f32) -> Result<Radii, String> {
        rc_package::audio_geometry::update_radii(old, base, self.clear, self.distance, now)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Voice {
    pub source: CachedRegion,
    pub radii: SavedRadii,
}
impl Voice {
    pub fn update(
        source: CachedRegion,
        hearing: Hearing,
        previous: Option<&Self>,
        base: f32,
        now: f32,
    ) -> Result<Self, String> {
        let old = match previous {
            Some(v) => v.radii.restore()?,
            None => Radii::initial(base)?,
        };
        Ok(Self {
            source,
            radii: SavedRadii::capture(&hearing.update(&old, base, now)?)?,
        })
    }
    pub fn validate(&self, assets: &Assets, base: f32, now: f32) -> Result<(), String> {
        self.source.validate(assets)?;
        let r = self.radii.restore()?;
        if (r.changed_at < 0. && r.changed_at != -1.)
            || r.changed_at > f64::from(now)
            || r.priority_radius < 0.
            || r.priority_radius > base
            || r.target < 0.
            || r.target > base
        {
            return Err("Invalid saved geometric sound radii".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn assets() -> Assets {
        let connectivity = vec![1, 2, 6];
        Assets {
            bsp: Bsp {
                nodes: vec![rc_package::audio_geometry::Node {
                    plane: [1., 0., 0., 0.],
                    back: -1,
                    front: -1,
                    flags: 0,
                    zones: [2, 1],
                    leaves: [20, 10],
                }],
                root_outside: true,
                zone_actors: vec![0, 101, 102],
            },
            distances: Distances::build(&connectivity).unwrap(),
            connectivity,
            level_info: 4,
            actors: vec![Actor {
                owner: 950,
                mode: 0,
                mover: false,
            }],
        }
    }
    #[test]
    fn region_cache_invalidation_modes_and_bad_scene_data() {
        let mut a = assets();
        a.validate().unwrap();
        let (source, listener) = a.regions([-100., 0., 0.], [100., 0., 0.]).unwrap();
        assert_eq!(source.region.zone, 2);
        assert_eq!(listener.region.zone, 1);
        assert!(
            !a.query(&a.actors[0], &source, &listener, false)
                .unwrap()
                .clear
        );
        assert_eq!(
            a.query(&a.actors[0], &source, &listener, false)
                .unwrap()
                .factor()
                .to_bits(),
            rc_package::audio_geometry::zone_multiplier(1).to_bits()
        );
        assert!(
            a.query(&a.actors[0], &source, &listener, true)
                .unwrap()
                .clear
        );
        for (mode, mover) in [(1, false), (255, false), (3, true)] {
            assert!(
                a.query(
                    &Actor {
                        owner: 950,
                        mode,
                        mover
                    },
                    &source,
                    &listener,
                    false
                )
                .unwrap()
                .clear
            );
        }
        let retained =
            CachedRegion::resolve(&a.bsp, source.position(), a.level_info, Some(&source)).unwrap();
        assert_eq!(retained, source);
        let moved = CachedRegion::resolve(&a.bsp, listener.position(), a.level_info, Some(&source))
            .unwrap();
        assert_eq!(moved, listener);
        let mut bad = source.clone();
        bad.position_bits[0] = f32::NAN.to_bits();
        assert!(bad.validate(&a).is_err());
        a.distances.bytes[2 * 64 + 1] = 63;
        assert!(a.validate().is_err());
        a = assets();
        a.actors[0].mode = 3;
        assert!(a.validate().is_err());
        a.actors[0].mover = true;
        assert!(a.validate().is_ok());
    }
    #[test]
    fn original_source_initialization_and_first_occluded_radius() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/zone-distances-native.json"))
                .unwrap();
        for original in fixture["source_initial"].as_array().unwrap() {
            let radius = original["radius"].as_f64().unwrap() as f32;
            let r = Radii::initial(radius).unwrap();
            assert_eq!(
                [
                    r.priority_radius.to_bits(),
                    r.current.to_bits(),
                    r.target.to_bits()
                ],
                original["radius_bits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u32)
                    .collect::<Vec<_>>()
                    .as_slice()
            );
            assert_eq!(
                r.changed_at.to_bits(),
                original["changed_at_bits"].as_u64().unwrap()
            );
            let r = Hearing {
                clear: false,
                distance: 1,
            }
            .update(&r, radius, 1. / 60.)
            .unwrap();
            assert_eq!(r.current.to_bits(), r.target.to_bits()); // Sentinel selects immediate first update.
            assert_eq!(
                r.changed_at,
                if radius == 0. {
                    -1.
                } else {
                    (1f32 / 60.) as f64
                }
            );
        }
    }
}
