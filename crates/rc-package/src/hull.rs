//! Original collision hulls and mathematical AABB sweeps, not native LineCheck parity.
use crate::{collision::SolidBsp, geometry::Bsp};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize)]
pub struct Hull {
    pub offset: usize,
    pub planes: Vec<[f64; 4]>,
    pub bounds: [[f64; 3]; 2],
}
#[derive(Debug, Serialize)]
pub struct HullSet {
    pub hulls: Vec<Hull>,
    pub missing_solid_leaves: usize,
    pub empty_root_solid: bool,
}
#[derive(Debug, Serialize)]
pub struct SweepHit {
    pub hull_offset: Option<usize>,
    pub fraction: f64,
    pub exit_fraction: f64,
    pub normal: [f64; 3],
    pub start_overlapping: bool,
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn normal(p: [f64; 4]) -> [f64; 3] {
    [p[0], p[1], p[2]]
}

impl HullSet {
    /// Transform original local hulls to world space before world-axis AABB support.
    /// Transforming only the extent as a vector would lose rotated-box edge axes.
    pub fn transformed(
        &self,
        transform: &crate::mesh_query::ActorTransform,
    ) -> Result<Self, String> {
        if transform
            .location
            .into_iter()
            .chain(transform.scale)
            .chain(transform.pivot)
            .any(|v| !v.is_finite())
            || transform.scale.contains(&0.0)
        {
            return Err("invalid hull actor transform".into());
        }
        let columns: [[f64; 3]; 3] = std::array::from_fn(|axis| {
            let mut basis = [0.0; 3];
            basis[axis] = 1.0;
            crate::mesh::transform_point(basis, [0.0; 3], transform.rotation, [1.0; 3], [0.0; 3])
                .map(|v| f64::from(v) * f64::from(transform.scale[axis]))
        });
        let cofactors = [
            cross(columns[1], columns[2]),
            cross(columns[2], columns[0]),
            cross(columns[0], columns[1]),
        ];
        let determinant = dot(columns[0], cofactors[0]);
        if determinant == 0.0 || !determinant.is_finite() {
            return Err("singular hull actor transform".into());
        }
        let location = transform.location.map(f64::from);
        let pivot = transform.pivot.map(f64::from);
        let point = |p: [f64; 3]| -> [f64; 3] {
            std::array::from_fn(|i| {
                location[i]
                    + (0..3)
                        .map(|j| columns[j][i] * (p[j] - pivot[j]))
                        .sum::<f64>()
            })
        };
        let plane = |p: [f64; 4]| -> Result<[f64; 4], String> {
            let n = std::array::from_fn(|i| {
                (0..3)
                    .map(|j| cofactors[j][i] * p[j] / determinant)
                    .sum::<f64>()
            });
            let w = p[3] - dot(normal(p), pivot) + dot(n, location);
            let length = dot(n, n).sqrt();
            if length == 0.0 || !length.is_finite() || !w.is_finite() {
                return Err("hull plane transform overflow".into());
            }
            Ok([n[0] / length, n[1] / length, n[2] / length, w / length])
        };
        let mut hulls = Vec::new();
        for h in &self.hulls {
            if h.planes.len() > 64 {
                return Err("expected original hull before actor transformation".into());
            }
            let mut planes = h
                .planes
                .iter()
                .copied()
                .map(plane)
                .collect::<Result<Vec<_>, _>>()?;
            // Preserve the original bounding faces as oriented world-space planes.
            for axis in 0..3 {
                for side in 0..2 {
                    let mut p = [0.0; 4];
                    p[axis] = if side == 0 { -1.0 } else { 1.0 };
                    p[3] = p[axis] * h.bounds[side][axis];
                    planes.push(plane(p)?);
                }
            }
            let mut bounds = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
            for mask in 0..8 {
                let corner = point(std::array::from_fn(|i| h.bounds[(mask >> i) & 1][i]));
                for (i, v) in corner.into_iter().enumerate() {
                    bounds[0][i] = bounds[0][i].min(v);
                    bounds[1][i] = bounds[1][i].max(v);
                }
            }
            if bounds.iter().flatten().any(|v| !v.is_finite()) {
                return Err("hull bounds transform overflow".into());
            }
            hulls.push(Hull {
                offset: h.offset,
                planes,
                bounds,
            });
        }
        Ok(Self {
            hulls,
            missing_solid_leaves: self.missing_solid_leaves,
            empty_root_solid: self.empty_root_solid,
        })
    }
    pub fn read(bsp: &Bsp, solid: &SolidBsp) -> Result<Self, String> {
        solid.validate()?;
        if bsp.nodes.len() != solid.nodes.len() {
            return Err("hull/BSP node mismatch".into());
        }
        let mut decoded = BTreeMap::new();
        let planes: Vec<_> = bsp.nodes.iter().map(|n| n.plane).collect();
        for node in &bsp.nodes {
            for child in [node.front, node.back] {
                if child < -1 || child >= bsp.nodes.len() as i32 {
                    return Err("invalid hull BSP child".into());
                }
            }
            if node.collision_bound == -1 {
                continue;
            }
            let offset =
                usize::try_from(node.collision_bound).map_err(|_| "negative hull offset")?;
            if decoded.contains_key(&offset) {
                continue;
            }
            decoded.insert(offset, decode_hull(offset, &solid.hull_words, &planes)?);
        }
        let mut selected = BTreeSet::new();
        let mut missing = 0;
        let mut stack = if bsp.nodes.is_empty() {
            vec![]
        } else {
            vec![(0, solid.root_outside)]
        };
        let mut visited = BTreeSet::new();
        while let Some((index, outside)) = stack.pop() {
            if !visited.insert((index, outside)) {
                continue;
            }
            let n = &bsp.nodes[index];
            // FBspNode loading masks flags to 0x1f before the runtime 0x21 check.
            let csg = n.vertex_count != 0 && (n.flags & 0x1f) & 0x21 == 0;
            for (child, space) in [(n.front, outside || csg), (n.back, outside && !csg)] {
                if child >= 0 {
                    stack.push((child as usize, space));
                } else if !space {
                    if n.collision_bound == -1 {
                        missing += 1;
                    } else {
                        selected.insert(n.collision_bound as usize);
                    }
                }
            }
        }
        Ok(Self {
            hulls: selected
                .into_iter()
                .map(|offset| decoded.remove(&offset).unwrap())
                .collect(),
            missing_solid_leaves: missing,
            empty_root_solid: bsp.nodes.is_empty() && !solid.root_outside,
        })
    }
    pub fn sweep(
        &self,
        start: [f64; 3],
        end: [f64; 3],
        extent: [f64; 3],
    ) -> Result<Option<SweepHit>, String> {
        self.sweep_with_policy(start, end, extent, false)
    }
    /// Excludes merely tangential/departing contact, but retains penetration.
    pub fn sweep_motion(
        &self,
        start: [f64; 3],
        end: [f64; 3],
        extent: [f64; 3],
    ) -> Result<Option<SweepHit>, String> {
        self.sweep_with_policy(start, end, extent, true)
    }
    fn sweep_with_policy(
        &self,
        start: [f64; 3],
        end: [f64; 3],
        extent: [f64; 3],
        motion: bool,
    ) -> Result<Option<SweepHit>, String> {
        if start
            .into_iter()
            .chain(end)
            .chain(extent)
            .any(|v| !v.is_finite() || v.abs() > 1e9)
            || extent.iter().any(|v| *v < 0.0)
        {
            return Err("invalid hull sweep input".into());
        }
        if self.missing_solid_leaves > 0 {
            return Err("solid BSP leaves without collision hull; extent query incomplete".into());
        }
        if self.empty_root_solid {
            return Ok(Some(SweepHit {
                hull_offset: None,
                fraction: 0.0,
                exit_fraction: 1.0,
                normal: [0.0; 3],
                start_overlapping: true,
            }));
        }
        let mut nearest = None;
        for hull in &self.hulls {
            if let Some(hit) = hull.sweep_with_policy(start, end, extent, motion)? {
                if nearest.as_ref().is_none_or(|current: &SweepHit| {
                    hit.fraction < current.fraction
                        || (hit.fraction == current.fraction
                            && hit.start_overlapping
                            && !current.start_overlapping)
                }) {
                    nearest = Some(hit);
                }
            }
        }
        Ok(nearest)
    }
}
fn decode_hull(offset: usize, words: &[u32], nodes: &[[f32; 4]]) -> Result<Hull, String> {
    let mut cursor = offset;
    let mut planes = Vec::new();
    loop {
        let word = *words.get(cursor).ok_or("hull missing terminator")?;
        cursor += 1;
        if word == u32::MAX {
            break;
        }
        if planes.len() >= 64 {
            return Err("hull exceeds native 64-plane limit".into());
        }
        let index = word & !0x40000000;
        let mut p = nodes
            .get(index as usize)
            .ok_or("hull plane reference out of range")?
            .map(f64::from);
        if word & 0x40000000 != 0 {
            p = p.map(|v| -v);
        }
        let length = dot(normal(p), normal(p)).sqrt();
        if p.iter().any(|v| !v.is_finite()) || length < 1e-12 {
            return Err("invalid hull plane".into());
        }
        planes.push(p.map(|v| v / length));
    }
    let raw = words
        .get(cursor..cursor.checked_add(6).ok_or("hull offset overflow")?)
        .ok_or("truncated hull bounds")?;
    let bounds = std::array::from_fn(|side| {
        std::array::from_fn(|i| f64::from(f32::from_bits(raw[side * 3 + i])))
    });
    if (0..3).any(|i| {
        !bounds[0][i].is_finite() || !bounds[1][i].is_finite() || bounds[0][i] > bounds[1][i]
    }) {
        return Err("invalid hull bounds".into());
    }
    Ok(Hull {
        offset,
        planes,
        bounds,
    })
}
impl Hull {
    /// Add box-axis and edge/box-axis separating planes before support expansion.
    /// Positive combinations of hull faces are valid even for nonadjacent pairs.
    fn sweep_planes(&self) -> Vec<[f64; 4]> {
        let mut planes = self.planes.clone();
        for axis in 0..3 {
            for side in 0..2 {
                let mut p = [0.0; 4];
                p[axis] = if side == 0 { -1.0 } else { 1.0 };
                p[3] = p[axis] * self.bounds[side][axis];
                planes.push(p);
            }
        }
        for (i, p) in self.planes.iter().enumerate() {
            for q in &self.planes[..i] {
                let (a, b) = (normal(*p), normal(*q));
                let edge = cross(a, b);
                let det = dot(edge, edge);
                if det < 1e-12 {
                    continue;
                }
                let ab = dot(a, b);
                for axis in 0..3 {
                    let mut basis = [0.0; 3];
                    basis[axis] = 1.0;
                    let mut n = cross(basis, edge);
                    let length = dot(n, n).sqrt();
                    if length < 1e-12 {
                        continue;
                    }
                    n = n.map(|v| v / length);
                    let mut alpha = (dot(n, a) - ab * dot(n, b)) / det;
                    let mut beta = (dot(n, b) - ab * dot(n, a)) / det;
                    if alpha < 0.0 && beta < 0.0 {
                        n = n.map(|v| -v);
                        alpha = -alpha;
                        beta = -beta;
                    }
                    if alpha >= -1e-10 && beta >= -1e-10 {
                        planes.push([n[0], n[1], n[2], alpha * p[3] + beta * q[3]]);
                    }
                }
            }
        }
        planes
    }
    pub fn sweep(
        &self,
        start: [f64; 3],
        end: [f64; 3],
        extent: [f64; 3],
    ) -> Result<Option<SweepHit>, String> {
        self.sweep_with_policy(start, end, extent, false)
    }
    /// Excludes merely tangential/departing contact, but retains penetration.
    pub fn sweep_motion(
        &self,
        start: [f64; 3],
        end: [f64; 3],
        extent: [f64; 3],
    ) -> Result<Option<SweepHit>, String> {
        self.sweep_with_policy(start, end, extent, true)
    }
    fn sweep_with_policy(
        &self,
        start: [f64; 3],
        end: [f64; 3],
        extent: [f64; 3],
        motion: bool,
    ) -> Result<Option<SweepHit>, String> {
        if start
            .into_iter()
            .chain(end)
            .chain(extent)
            .any(|v| !v.is_finite() || v.abs() > 1e9)
            || extent.iter().any(|v| *v < 0.0)
        {
            return Err("invalid hull sweep input".into());
        }
        if self.planes.len() > 70
            || self
                .planes
                .iter()
                .any(|p| p.iter().any(|v| !v.is_finite()) || dot(normal(*p), normal(*p)) < 1e-12)
            || (0..3).any(|i| {
                !self.bounds[0][i].is_finite()
                    || !self.bounds[1][i].is_finite()
                    || self.bounds[0][i] > self.bounds[1][i]
            })
        {
            return Err("invalid sweep hull".into());
        }
        if (0..3).any(|i| {
            start[i].min(end[i]) - extent[i] > self.bounds[1][i]
                || start[i].max(end[i]) + extent[i] < self.bounds[0][i]
        }) {
            return Ok(None);
        }
        let (mut enter, mut exit) = (0.0f64, 1.0f64);
        let mut hit_normal = [0.0; 3];
        let mut overlapping = true;
        for p in self.sweep_planes() {
            let n = normal(p);
            let support = (0..3).map(|i| n[i].abs() * extent[i]).sum::<f64>();
            let a = dot(n, start) - p[3] - support;
            let b = dot(n, end) - p[3] - support;
            // Own f64 boundary tolerance, not a recovered native epsilon.
            if motion && (-1e-9..=1e-9).contains(&a) && b >= a - 1e-9 {
                // A separating/tangent plane protects the whole convex path.
                return Ok(None);
            }
            overlapping &= if motion { a < -1e-9 } else { a <= 0.0 };
            if motion && a.abs() <= 1e-9 && b < a - 1e-9 {
                hit_normal = n;
            }
            if a > 0.0 && b > 0.0 {
                return Ok(None);
            }
            if a <= 0.0 && b <= 0.0 {
                continue;
            }
            let fraction = a / (a - b);
            if a > b {
                if fraction > enter {
                    enter = fraction;
                    hit_normal = n;
                }
            } else {
                exit = exit.min(fraction);
            }
            if enter > exit {
                return Ok(None);
            }
        }
        Ok(Some(SweepHit {
            hull_offset: Some(self.offset),
            fraction: enter,
            exit_fraction: exit,
            normal: hit_normal,
            start_overlapping: overlapping,
        }))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hull_reference_flip_bounds_limits_and_truncations() {
        let mut words = vec![0x40000000, u32::MAX];
        words.extend([-1.0f32, -2.0, -3.0, 1.0, 2.0, 3.0].map(f32::to_bits));
        let hull = decode_hull(0, &words, &[[1.0, 0.0, 0.0, 1.0]]).unwrap();
        assert_eq!(hull.planes[0], [-1.0, 0.0, 0.0, -1.0]);
        for end in 0..words.len() {
            assert!(decode_hull(0, &words[..end], &[[1.0, 0.0, 0.0, 1.0]]).is_err());
        }
        words[0] = 1;
        assert!(decode_hull(0, &words, &[[1.0, 0.0, 0.0, 1.0]]).is_err());
        words[0] = 0;
        words[2] = f32::NAN.to_bits();
        assert!(decode_hull(0, &words, &[[1.0, 0.0, 0.0, 1.0]]).is_err());
        assert!(decode_hull(0, &vec![0; 65], &[[1.0, 0.0, 0.0, 1.0]]).is_err());
    }
    #[test]
    fn box_sweep_fraction_overlap_direction_and_invalid_extent() {
        let h = Hull {
            offset: 7,
            planes: vec![],
            bounds: [[-1.0; 3], [1.0; 3]],
        };
        for (a, b, sign) in [(-5.0, 5.0, -1.0), (5.0, -5.0, 1.0)] {
            let hit = h
                .sweep([a, 0.0, 0.0], [b, 0.0, 0.0], [0.5; 3])
                .unwrap()
                .unwrap();
            assert!((hit.fraction - 0.35).abs() < 1e-10);
            assert_eq!(hit.normal, [sign, 0.0, 0.0]);
            assert!(!hit.start_overlapping);
        }
        assert!(h
            .sweep([-5.0, 2.0, 0.0], [5.0, 2.0, 0.0], [0.5; 3])
            .unwrap()
            .is_none());
        assert!(
            h.sweep([0.0; 3], [0.0; 3], [0.5; 3])
                .unwrap()
                .unwrap()
                .start_overlapping
        );
        assert!(h.sweep([0.0; 3], [0.0; 3], [-1.0; 3]).is_err());
    }
    #[test]
    fn solid_leaf_selection_not_csg_and_missing_hulls_are_explicit() {
        let mut bsp = Bsp {
            vectors: vec![],
            points: vec![],
            nodes: vec![crate::geometry::Node {
                plane: [1.0, 0.0, 0.0, 0.0],
                vertex_start: 0,
                surface: 0,
                back: -1,
                front: -1,
                coplanar: -1,
                collision_bound: 0,
                vertex_count: 3,
                flags: 0,
            }],
            surfaces: vec![],
            vertices: vec![],
            geometry_bytes: 0,
            unparsed_tail_bytes: 0,
        };
        let mut words = vec![0, u32::MAX];
        words.extend([-1.0f32, -1.0, -1.0, 0.0, 1.0, 1.0].map(f32::to_bits));
        let solid = SolidBsp {
            nodes: vec![crate::collision::SolidNode {
                plane: [1.0, 0.0, 0.0, 0.0],
                back: -1,
                front: -1,
                flags: 0,
            }],
            root_outside: true,
            linked: false,
            zones: 0,
            bounds: 0,
            hull_indices: words.len(),
            hull_words: words,
            leaves: 0,
            lights: 0,
            decoded_bytes: 0,
            remaining_bytes: 0,
        };
        let set = HullSet::read(&bsp, &solid).unwrap();
        assert_eq!(set.hulls.len(), 1);
        assert_eq!(set.missing_solid_leaves, 0);
        bsp.nodes[0].collision_bound = -1;
        let set = HullSet::read(&bsp, &solid).unwrap();
        assert_eq!(set.missing_solid_leaves, 1);
        assert!(set.sweep([0.0; 3], [1.0; 3], [0.1; 3]).is_err());
        bsp.nodes[0].flags = 1;
        assert!(HullSet::read(&bsp, &solid).unwrap().hulls.is_empty());
        bsp.nodes[0].collision_bound = 0;
        bsp.nodes[0].flags = 0x20;
        assert_eq!(HullSet::read(&bsp, &solid).unwrap().hulls.len(), 1);
        bsp.nodes[0].front = 9;
        assert!(HullSet::read(&bsp, &solid).is_err());
    }
    #[test]
    fn transformed_hull_preserves_oriented_bounds_pivot_and_negative_scale() {
        let set = HullSet {
            hulls: vec![Hull {
                offset: 0,
                planes: vec![],
                bounds: [[-1.0; 3], [1.0; 3]],
            }],
            missing_solid_leaves: 0,
            empty_root_solid: false,
        };
        let t = crate::mesh_query::ActorTransform {
            location: [10.0, 20.0, 30.0],
            rotation: [0, 16384, 0],
            scale: [-2.0, 3.0, 1.0],
            pivot: [1.0, 0.0, 0.0],
        };
        let world = set.transformed(&t).unwrap();
        let hit = world
            .sweep([10.0, 10.0, 30.0], [10.0, 30.0, 30.0], [0.5; 3])
            .unwrap()
            .unwrap();
        assert!((hit.fraction - 0.475).abs() < 1e-6);
        assert!(hit.normal[1] < -0.999);
        let rotated = crate::mesh_query::ActorTransform {
            location: [0.0; 3],
            rotation: [0, 8192, 0],
            scale: [1.0; 3],
            pivot: [0.0; 3],
        };
        assert!(set
            .transformed(&rotated)
            .unwrap()
            .sweep([1.3, 1.3, 0.0], [1.3, 1.3, 0.0], [0.01; 3])
            .unwrap()
            .is_none());
        let mut bad = t;
        bad.scale[0] = 0.0;
        assert!(set.transformed(&bad).is_err());
    }
    #[test]
    fn edge_bevel_prevents_false_tetrahedron_corner_hit() {
        let h = Hull {
            offset: 0,
            planes: vec![
                [-1.0, 0.0, 0.0, 0.0],
                [0.0, -1.0, 0.0, 0.0],
                [0.0, 0.0, -1.0, 0.0],
                [
                    1.0 / 3.0f64.sqrt(),
                    1.0 / 3.0f64.sqrt(),
                    1.0 / 3.0f64.sqrt(),
                    1.0 / 3.0f64.sqrt(),
                ],
            ],
            bounds: [[0.0; 3], [1.0; 3]],
        };
        assert!(h
            .sweep([-0.1, 0.7, 0.7], [-0.1, 0.7, 0.7], [0.1; 3])
            .unwrap()
            .is_none());
        assert!(h
            .sweep([0.1, 0.1, 0.1], [0.1, 0.1, 0.1], [0.05; 3])
            .unwrap()
            .is_some());
    }
}
