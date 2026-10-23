//! Original PC audio BSP queries. No triangle approximation or playback host.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub plane: [f32; 4],
    pub back: i32,
    pub front: i32,
    pub flags: u8,
    /// Original order: back, front.
    pub zones: [u8; 2],
    pub leaves: [i32; 2],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bsp {
    pub nodes: Vec<Node>,
    pub root_outside: bool,
    /// Original object references. Zero falls back to the supplied LevelInfo.
    pub zone_actors: Vec<i32>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Region {
    pub actor: i32,
    pub leaf: i32,
    pub zone: u8,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HearingPath {
    Bsp,
    Bypass,
    /// Original SingleLineCheck mask 0x286, zero extent, source actor ignored.
    GeneralTrace,
}
pub fn hearing_path(actor_mode: Option<u8>, mover: bool) -> HearingPath {
    match if mover { 1 } else { actor_mode.unwrap_or(0) } {
        0 | 2 => HearingPath::Bsp,
        3 => HearingPath::GeneralTrace,
        _ => HearingPath::Bypass,
    }
}
impl Bsp {
    pub fn validate(&self) -> Result<(), String> {
        if self.nodes.len() > 1_000_000 || self.zone_actors.len() > 64 {
            return Err("Audio BSP exceeds supported limits".into());
        }
        let mut colors = vec![0u8; self.nodes.len()];
        for (i, n) in self.nodes.iter().enumerate() {
            if n.plane.iter().any(|v| !v.is_finite() || v.abs() > 1e6)
                || n.plane[..3].iter().all(|v| *v == 0.)
                || n.zones.iter().any(|z| {
                    *z > 63
                        || (!self.zone_actors.is_empty()
                            && usize::from(*z) >= self.zone_actors.len())
                })
            {
                return Err("Invalid audio BSP plane or zone".into());
            }
            for c in [n.back, n.front] {
                if c < -1 || c >= self.nodes.len() as i32 {
                    return Err(format!("Invalid audio BSP child at {i}"));
                }
            }
        }
        for root in 0..self.nodes.len() {
            if colors[root] != 0 {
                continue;
            }
            let mut stack = vec![(root, false)];
            while let Some((i, exiting)) = stack.pop() {
                if exiting {
                    colors[i] = 2;
                    continue;
                }
                if colors[i] == 1 {
                    return Err("Cyclic audio BSP".into());
                }
                if colors[i] == 2 {
                    continue;
                }
                colors[i] = 1;
                stack.push((i, true));
                for c in [self.nodes[i].back, self.nodes[i].front] {
                    if c >= 0 {
                        stack.push((c as usize, false));
                    }
                }
            }
        }
        Ok(())
    }
    /// FastLineCheck 10552a60 / 105520a0. Endpoint dot products have different
    /// SSE addition order; branch selection uses the float SIGN BIT.
    pub fn clear(&self, start: [f32; 3], end: [f32; 3]) -> Result<bool, String> {
        self.validate()?;
        point(start)?;
        point(end)?;
        if self.nodes.is_empty() {
            return Ok(self.root_outside);
        }
        let mut stack = vec![(0i32, start, end, self.root_outside)];
        let mut budget = self.nodes.len().saturating_mul(8) + 1;
        while let Some((mut index, a, b, mut outside)) = stack.pop() {
            while index != -1 {
                if budget == 0 {
                    return Err("Audio BSP query budget exceeded".into());
                }
                budget -= 1;
                let n = &self.nodes[index as usize];
                let p = n.plane;
                let db = ((p[1] * b[1] + p[2] * b[2]) + p[0] * b[0]) - p[3];
                let da = ((p[1] * a[1] + p[0] * a[0]) + p[2] * a[2]) - p[3];
                let sb = db.to_bits() >> 31 == 0;
                let sa = da.to_bits() >> 31 == 0;
                let space = |side: bool| side ^ ((side ^ outside) & (n.flags & 1 != 0));
                if sa != sb {
                    let t = db / (db - da);
                    let middle = std::array::from_fn(|i| (a[i] - b[i]) * t + b[i]);
                    // Original visits the start segment first, then the end.
                    stack.push((if sb { n.front } else { n.back }, middle, b, space(sb)));
                    index = if sa { n.front } else { n.back };
                    outside = space(sa);
                    // This half's end changes; an explicit work item avoids recursion.
                    stack.push((index, a, middle, outside));
                    index = -1;
                    outside = true;
                } else {
                    index = if sb { n.front } else { n.back };
                    outside = space(sb);
                }
            }
            if !outside {
                return Ok(false);
            }
        }
        Ok(true)
    }
    /// PointRegion 105538e0. On-plane points choose front; no zones selects 0.
    pub fn region(&self, position: [f32; 3], default_actor: i32) -> Result<Region, String> {
        self.validate()?;
        point(position)?;
        let mut out = Region {
            actor: default_actor,
            leaf: -1,
            zone: 0,
        };
        if self.nodes.is_empty() {
            return Ok(out);
        }
        let mut index = 0;
        let mut budget = self.nodes.len() + 1;
        loop {
            if budget == 0 {
                return Err("Audio region query budget exceeded".into());
            }
            budget -= 1;
            let n = &self.nodes[index];
            let p = n.plane;
            let d = ((p[1] * position[1] + p[2] * position[2]) + p[0] * position[0]) - p[3];
            let side = usize::from(d >= 0.);
            let child = if side == 0 { n.back } else { n.front };
            if child >= 0 {
                index = child as usize;
                continue;
            }
            out.leaf = n.leaves[side];
            if !self.zone_actors.is_empty() {
                out.zone = n.zones[side];
            }
            if let Some(&actor) = self.zone_actors.get(usize::from(out.zone)) {
                if actor != 0 {
                    out.actor = actor;
                }
            }
            return Ok(out);
        }
    }
}
/// Persist all original radius bits, including the widened engine clock. JSON
/// decimal f64 parsing must not move a stored clock by one bit.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedRadii {
    pub radius_bits: [u32; 3],
    pub changed_at_bits: u64,
}
impl SavedRadii {
    pub fn capture(r: &crate::world_audio::Radii) -> Result<Self, String> {
        let saved = Self {
            radius_bits: [
                r.priority_radius.to_bits(),
                r.current.to_bits(),
                r.target.to_bits(),
            ],
            changed_at_bits: r.changed_at.to_bits(),
        };
        saved.restore()?;
        Ok(saved)
    }
    pub fn restore(&self) -> Result<crate::world_audio::Radii, String> {
        let [priority_radius, current, target] = self.radius_bits.map(f32::from_bits);
        let changed_at = f64::from_bits(self.changed_at_bits);
        if [priority_radius, current, target]
            .iter()
            .any(|v| !v.is_finite())
            || !changed_at.is_finite()
        {
            return Err("Invalid saved audio radii".into());
        }
        Ok(crate::world_audio::Radii {
            priority_radius,
            current,
            target,
            changed_at,
        })
    }
}
/// Compose a resolved original zone distance with UpdateRadii. The caller owns
/// the zone-distance matrix and time; this does not infer a factor from a wall.
pub fn update_radii(
    previous: &crate::world_audio::Radii,
    base: f32,
    clear: bool,
    zone_distance: u8,
    now: f32,
) -> Result<crate::world_audio::Radii, String> {
    previous.update(
        base,
        if clear {
            None
        } else {
            Some(zone_multiplier(zone_distance))
        },
        now,
    )
}
fn point(p: [f32; 3]) -> Result<(), String> {
    if p.iter().any(|v| !v.is_finite() || v.abs() > 1e6) {
        return Err("Invalid audio query point".into());
    }
    Ok(())
}
/// Read original node-side zones/leaf indices and zone actor references without
/// changing the separate physics BSP. Runtime zone distances are NOT serialized here.
pub fn read_model(
    pkg: &crate::Package,
    data: &[u8],
    export: &crate::Export,
) -> Result<Bsp, String> {
    let geometry = crate::geometry::read_bsp(pkg, data, export)?;
    let collision = crate::collision::read_model_collision(pkg, data, export, &geometry)?;
    let payload = pkg.payload(data, export)?;
    let props = crate::properties::read(pkg, data, export)?;
    let mut r = crate::Reader::at(payload, props.native_offset)?;
    r.take(41)?; // Primitive bounds: two vectors, byte, vector, float.
    for _ in 0..2 {
        let count = r.count(12)?;
        r.take(count * 12)?;
    }
    let count = r.count(71)?;
    let mut nodes = Vec::with_capacity(count);
    for original in &geometry.nodes {
        r.take(25)?; // plane, zone mask, flags
        for _ in 0..7 {
            r.index()?;
        }
        r.take(16)?; // sphere
        let zones = [r.byte()?, r.byte()?];
        r.byte()?;
        let leaves = [r.i32()?, r.i32()?];
        r.take(12)?;
        nodes.push(Node {
            plane: original.plane,
            back: original.back,
            front: original.front,
            flags: original.flags,
            zones,
            leaves,
        });
    }
    let mut r = crate::Reader::at(payload, geometry.geometry_bytes)?;
    r.u32()?;
    let count = r.u32()?;
    let mut zone_actors = Vec::new();
    for _ in 0..count {
        let actor = r.index()?;
        pkg.object_path(actor)?;
        zone_actors.push(actor);
        r.take(20)?;
    }
    let bsp = Bsp {
        nodes,
        root_outside: collision.root_outside,
        zone_actors,
    };
    bsp.validate()?;
    Ok(bsp)
}

/// CalculateRadiusMultiplier: 0.85^(distance_byte squared), rounded by bundled
/// original CRT to f32. Lookup preserves exact subnormals across platforms.
pub fn zone_multiplier(distance: u8) -> f32 {
    f32::from_bits(ZONE_BITS[usize::from(distance)])
}
const ZONE_BITS: [u32; 256] = [
    1065353216, 1062836634, 1057333812, 1047342334, 1033375988, 1015865964, 993827783, 968257073,
    939457058, 906018148, 867949272, 826674232, 781638265, 732301773, 679741888, 622311854,
    561770187, 496443002, 428014633, 354684494, 278164715, 197271385, 112682327, 24796435, 15804,
    5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0,
];

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Deserialize)]
    struct Fixture {
        geometry: Vec<Case>,
        policies: Vec<Policy>,
        zone_multiplier_bits: Vec<u32>,
        radius_sequences: Vec<Sequence>,
    }
    #[derive(Deserialize)]
    struct Case {
        bsp: Bsp,
        start: [f32; 3],
        end: [f32; 3],
        clear: bool,
        region: Region,
    }
    #[derive(Deserialize)]
    struct Policy {
        mode: u8,
        mover: bool,
        trace: bool,
        audible: bool,
        trace_called: bool,
    }
    #[derive(Deserialize)]
    struct Sequence {
        base_radius: f32,
        steps: Vec<Step>,
    }
    #[derive(Deserialize)]
    struct Step {
        clear: bool,
        distance: u8,
        now: f32,
        priority_bits: u32,
        current_bits: u32,
        target_bits: u32,
        changed_at_bits: u64,
    }
    #[test]
    fn original_zone_radius_sequences_survive_serialization() {
        for sequence in fixture().radius_sequences {
            let base = sequence.base_radius;
            let mut state = crate::world_audio::Radii {
                priority_radius: base,
                current: base,
                target: base,
                changed_at: 0.,
            };
            for step in sequence.steps {
                let bytes = serde_json::to_vec(&SavedRadii::capture(&state).unwrap()).unwrap();
                let restored = serde_json::from_slice::<SavedRadii>(&bytes)
                    .unwrap()
                    .restore()
                    .unwrap();
                let expected =
                    update_radii(&state, base, step.clear, step.distance, step.now).unwrap();
                state = update_radii(&restored, base, step.clear, step.distance, step.now).unwrap();
                assert_eq!(state, expected);
                assert_eq!(
                    [
                        state.priority_radius.to_bits(),
                        state.current.to_bits(),
                        state.target.to_bits()
                    ],
                    [step.priority_bits, step.current_bits, step.target_bits]
                );
                assert_eq!(state.changed_at.to_bits(), step.changed_at_bits);
            }
        }
    }
    fn fixture() -> Fixture {
        serde_json::from_str(include_str!("../../../fixtures/audio-geometry-native.json")).unwrap()
    }
    #[test]
    fn original_bsp_lines_and_regions() {
        for c in fixture().geometry {
            assert_eq!(c.bsp.clear(c.start, c.end).unwrap(), c.clear);
            assert_eq!(c.bsp.region(c.start, 999).unwrap(), c.region);
        }
    }
    #[test]
    fn original_actor_hearing_dispatch() {
        for c in fixture().policies {
            let path = hearing_path(Some(c.mode), c.mover);
            assert_eq!(path == HearingPath::GeneralTrace, c.trace_called);
            assert_eq!(
                match path {
                    HearingPath::Bsp => false,
                    HearingPath::Bypass => true,
                    HearingPath::GeneralTrace => c.trace,
                },
                c.audible
            );
        }
        assert_eq!(hearing_path(None, false), HearingPath::Bsp);
    }
    #[test]
    fn original_zone_radius_lookup() {
        for (n, bits) in fixture().zone_multiplier_bits.into_iter().enumerate() {
            assert_eq!(zone_multiplier(n as u8).to_bits(), bits);
        }
        assert_eq!(zone_multiplier(0), 1.);
        assert_eq!(zone_multiplier(255), 0.);
    }
    #[test]
    fn invalid_geometry_is_rejected_and_roundtrip_preserves_queries() {
        let mut saved = SavedRadii {
            radius_bits: [1f32.to_bits(); 3],
            changed_at_bits: 1f64.to_bits(),
        };
        saved.radius_bits[0] = f32::NAN.to_bits();
        assert!(saved.restore().is_err());
        saved.radius_bits[0] = 1f32.to_bits();
        saved.changed_at_bits = f64::INFINITY.to_bits();
        assert!(saved.restore().is_err());
        let mut c = fixture().geometry.remove(1);
        let saved = serde_json::to_vec(&c.bsp).unwrap();
        let copy: Bsp = serde_json::from_slice(&saved).unwrap();
        assert_eq!(copy.clear(c.start, c.end).unwrap(), c.clear);
        assert!(copy.clear([f32::NAN, 0., 0.], c.end).is_err());
        c.bsp.nodes[0].front = 0;
        assert!(c.bsp.validate().is_err());
        c.bsp.nodes[0].front = -2;
        assert!(c.bsp.validate().is_err());
        c.bsp.nodes[0].front = -1;
        c.bsp.nodes[0].plane[0] = f32::INFINITY;
        assert!(c.bsp.validate().is_err());
    }
}
