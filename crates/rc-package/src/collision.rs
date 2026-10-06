//! Original Model BSP point-space/zero-extent checks. No mesh or pawn collision.
use crate::{geometry::Bsp, Export, Package, Reader};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct SolidNode {
    pub plane: [f32; 4],
    pub back: i32,
    pub front: i32,
    pub flags: u8,
}
#[derive(Clone, Debug, Serialize)]
pub struct SolidBsp {
    pub nodes: Vec<SolidNode>,
    pub root_outside: bool,
    pub linked: bool,
    pub zones: usize,
    pub bounds: usize,
    pub hull_indices: usize,
    pub hull_words: Vec<u32>,
    pub leaves: usize,
    pub lights: usize,
    pub decoded_bytes: usize,
    pub remaining_bytes: usize,
}
pub fn read_model_collision(
    pkg: &Package,
    data: &[u8],
    export: &Export,
    bsp: &Bsp,
) -> Result<SolidBsp, String> {
    if !(151..=159).contains(&pkg.summary.version) || pkg.summary.licensee_version > 1 {
        return Err("Unsupported Model collision variant".into());
    }
    let payload = pkg.payload(data, export)?;
    let mut r = Reader::at(payload, bsp.geometry_bytes)?;
    r.u32()?; // zones bitmask/count companion at UModel +0xe0
    let zones = r.u32()? as usize;
    if zones > 64 {
        return Err("Too many BSP zones".into());
    }
    for _ in 0..zones {
        let object = r.index()?;
        pkg.object_path(object)?;
        r.take(20)?;
    }
    pkg.object_path(r.index()?)?; // Polys reference
    let bounds = r.count(25)?;
    r.take(bounds * 25)?;
    let hull_indices = r.count(4)?;
    let hull_words = (0..hull_indices)
        .map(|_| r.u32())
        .collect::<Result<Vec<_>, _>>()?;
    let leaves = r.count(if pkg.summary.version < 156 { 11 } else { 10 })?;
    for _ in 0..leaves {
        r.index()?;
        r.index()?;
        if pkg.summary.version < 156 {
            r.index()?;
        }
        r.take(8)?;
    }
    let lights = r.count(1)?;
    for _ in 0..lights {
        pkg.object_path(r.index()?)?;
    }
    let outside = r.u32()?;
    let linked = r.u32()?;
    if outside > 1 || linked > 1 {
        return Err("Invalid BSP boolean state".into());
    }
    let result = SolidBsp {
        nodes: bsp
            .nodes
            .iter()
            .map(|n| SolidNode {
                plane: n.plane,
                back: n.back,
                front: n.front,
                flags: n.flags & 31,
            })
            .collect(),
        root_outside: outside != 0,
        linked: linked != 0,
        zones,
        bounds,
        hull_indices,
        hull_words,
        leaves,
        lights,
        decoded_bytes: r.pos,
        remaining_bytes: payload.len() - r.pos,
    };
    result.validate()?;
    Ok(result)
}
impl SolidBsp {
    pub fn validate(&self) -> Result<(), String> {
        let mut color = vec![0u8; self.nodes.len()];
        for (i, n) in self.nodes.iter().enumerate() {
            if n.plane.iter().any(|v| !v.is_finite() || v.abs() > 1e9)
                || n.plane[..3].iter().all(|&v| v == 0.)
            {
                return Err("Invalid BSP plane".into());
            }
            for child in [n.back, n.front] {
                if child < -1 || child >= self.nodes.len() as i32 {
                    return Err(format!("BSP child outside nodes at {i}"));
                }
            }
        }
        for root in 0..self.nodes.len() {
            if color[root] != 0 {
                continue;
            }
            let mut stack = vec![(root, false)];
            while let Some((i, exit)) = stack.pop() {
                if exit {
                    color[i] = 2;
                    continue;
                }
                if color[i] == 1 {
                    return Err("Cycle in BSP children".into());
                }
                if color[i] == 2 {
                    continue;
                }
                color[i] = 1;
                stack.push((i, true));
                for child in [self.nodes[i].back, self.nodes[i].front] {
                    if child >= 0 {
                        stack.push((child as usize, false));
                    }
                }
            }
        }
        Ok(())
    }
    fn point_valid(point: [f32; 3]) -> Result<(), String> {
        if point.iter().any(|v| !v.is_finite() || v.abs() > 1e9) {
            Err("Invalid BSP query point".into())
        } else {
            Ok(())
        }
    }
    fn side(n: &SolidNode, p: [f32; 3]) -> (bool, f32) {
        let d = p[0] * n.plane[0] + p[1] * n.plane[1] + p[2] * n.plane[2] - n.plane[3];
        (!d.is_sign_negative(), d)
    }
    fn next_outside(n: &SolidNode, side: bool, outside: bool) -> bool {
        if n.flags & 1 != 0 {
            outside
        } else {
            side
        }
    }
    pub fn point_outside(&self, point: [f32; 3]) -> Result<bool, String> {
        Self::point_valid(point)?;
        let mut outside = self.root_outside;
        let mut index = if self.nodes.is_empty() { -1 } else { 0 };
        for _ in 0..=self.nodes.len() {
            if index == -1 {
                return Ok(outside);
            }
            let n = self.nodes.get(index as usize).ok_or("Invalid BSP child")?;
            let (side, _) = Self::side(n, point);
            outside = Self::next_outside(n, side, outside);
            index = if side { n.front } else { n.back };
        }
        Err("BSP query cycle".into())
    }
    /// Boolean analogue of UModel::FastLineCheck; zero extent, no hit normal/time.
    pub fn line_clear(&self, start: [f32; 3], end: [f32; 3]) -> Result<bool, String> {
        Self::point_valid(start)?;
        Self::point_valid(end)?;
        let mut stack = vec![(
            if self.nodes.is_empty() { -1 } else { 0 },
            start,
            end,
            self.root_outside,
        )];
        let mut budget = self.nodes.len().saturating_mul(4) + 1;
        while let Some((index, a, b, outside)) = stack.pop() {
            if budget == 0 {
                return Err("BSP traversal budget exceeded".into());
            }
            budget -= 1;
            if index == -1 {
                if !outside {
                    return Ok(false);
                }
                continue;
            }
            let n = self.nodes.get(index as usize).ok_or("Invalid BSP child")?;
            let (sa, da) = Self::side(n, a);
            let (sb, db) = Self::side(n, b);
            let child = |side| if side { n.front } else { n.back };
            if sa == sb {
                stack.push((child(sa), a, b, Self::next_outside(n, sa, outside)));
            } else {
                let fraction = da / (da - db);
                let split = std::array::from_fn(|i| a[i] + (b[i] - a[i]) * fraction);
                stack.push((child(sb), split, b, Self::next_outside(n, sb, outside)));
                stack.push((child(sa), a, split, Self::next_outside(n, sa, outside)));
            }
        }
        Ok(true)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn solid(nodes: Vec<SolidNode>, root_outside: bool) -> SolidBsp {
        SolidBsp {
            nodes,
            root_outside,
            linked: false,
            zones: 0,
            bounds: 0,
            hull_indices: 0,
            hull_words: vec![],
            leaves: 0,
            lights: 0,
            decoded_bytes: 0,
            remaining_bytes: 0,
        }
    }
    #[test]
    fn solid_slab_blocks_crossing_with_both_endpoints_outside() {
        let bsp = solid(
            vec![
                SolidNode {
                    plane: [1., 0., 0., 1.],
                    back: 1,
                    front: -1,
                    flags: 0,
                },
                SolidNode {
                    plane: [-1., 0., 0., 1.],
                    back: -1,
                    front: -1,
                    flags: 0,
                },
            ],
            false,
        );
        bsp.validate().unwrap();
        assert!(bsp.point_outside([-2., 0., 0.]).unwrap());
        assert!(!bsp.point_outside([0., 0., 0.]).unwrap());
        assert!(bsp.point_outside([2., 0., 0.]).unwrap());
        assert!(!bsp.line_clear([-2., 0., 0.], [2., 0., 0.]).unwrap());
        assert!(!bsp.line_clear([2., 0., 0.], [-2., 0., 0.]).unwrap());
        assert!(bsp.line_clear([2., 0., 0.], [2., 5., 0.]).unwrap());
        assert!(!bsp.line_clear([0.; 3], [0.; 3]).unwrap());
        assert!(bsp.line_clear([f32::NAN, 0., 0.], [0.; 3]).is_err());
    }
    #[test]
    fn non_csg_retains_outside_and_malformed_trees_are_rejected() {
        let mut bsp = solid(
            vec![SolidNode {
                plane: [1., 0., 0., 0.],
                back: -1,
                front: -1,
                flags: 1,
            }],
            true,
        );
        assert!(bsp.point_outside([-1., 0., 0.]).unwrap());
        assert!(bsp.line_clear([-1., 0., 0.], [1., 0., 0.]).unwrap());
        bsp.root_outside = false;
        assert!(!bsp.point_outside([1., 0., 0.]).unwrap());
        bsp.nodes[0].front = 0;
        assert!(bsp.validate().is_err());
        assert!(bsp.point_outside([1., 0., 0.]).is_err());
        bsp.nodes[0].front = 9;
        assert!(bsp.validate().is_err());
        bsp.nodes[0].front = -1;
        bsp.nodes[0].plane[0] = f32::NAN;
        assert!(bsp.validate().is_err());
        assert!(solid(vec![], true).line_clear([0.; 3], [1.; 3]).unwrap());
        assert!(!solid(vec![], false).point_outside([0.; 3]).unwrap());
    }
    #[test]
    fn original_tail_layout_and_all_truncations_for_both_leaf_variants() {
        for version in [151, 159] {
            let pkg = Package {
                summary: crate::Summary {
                    version,
                    licensee_version: 1,
                    flags: 0,
                    name_count: 0,
                    name_offset: 0,
                    export_count: 0,
                    export_offset: 0,
                    import_count: 0,
                    import_offset: 0,
                },
                names: vec![],
                imports: vec![],
                exports: vec![],
            };
            let mut bytes = vec![];
            bytes.extend_from_slice(&0u32.to_le_bytes());
            bytes.extend_from_slice(&1u32.to_le_bytes());
            bytes.push(0);
            bytes.extend_from_slice(&[0; 20]);
            bytes.push(0);
            bytes.push(1);
            bytes.extend_from_slice(&[0; 25]); // one bound
            bytes.push(1);
            bytes.extend_from_slice(&(-1i32).to_le_bytes()); // one hull value
            bytes.push(1);
            bytes.extend_from_slice(if version < 156 { &[0; 3] } else { &[0; 2] });
            bytes.extend_from_slice(&[0; 8]);
            bytes.extend_from_slice(&[1, 0]); // light reference
            bytes.extend_from_slice(&1u32.to_le_bytes());
            bytes.extend_from_slice(&0u32.to_le_bytes());
            let mut e = Export {
                class: 0,
                super_class: 0,
                outer: 0,
                extra: None,
                name: "Model".into(),
                flags: 0,
                serial_size: bytes.len() as u32,
                serial_offset: 0,
            };
            let bsp = Bsp {
                vectors: vec![],
                points: vec![],
                nodes: vec![],
                surfaces: vec![],
                vertices: vec![],
                geometry_bytes: 0,
                unparsed_tail_bytes: bytes.len(),
            };
            let decoded = read_model_collision(&pkg, &bytes, &e, &bsp).unwrap();
            assert_eq!(
                (
                    decoded.zones,
                    decoded.bounds,
                    decoded.hull_indices,
                    decoded.leaves,
                    decoded.lights
                ),
                (1, 1, 1, 1, 1)
            );
            assert!(decoded.root_outside);
            assert_eq!(decoded.remaining_bytes, 0);
            for length in 0..bytes.len() {
                e.serial_size = length as u32;
                assert!(read_model_collision(&pkg, &bytes[..length], &e, &bsp).is_err());
            }
            e.serial_size = bytes.len() as u32;
            let pos = bytes.len() - 8;
            bytes[pos..pos + 4].copy_from_slice(&2u32.to_le_bytes());
            assert!(read_model_collision(&pkg, &bytes, &e, &bsp).is_err());
        }
    }
}
