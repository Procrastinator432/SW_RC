//! Explicit mathematical completion of missing serialized BSP collision cells.
//! Only bounded, nondegenerate original half-space intersections are accepted.
//! This is not native LineCheck parity; ordinary HullSet::read remains strict.
use crate::{
    collision::SolidBsp,
    geometry::Bsp,
    hull::{Hull, HullSet},
};
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
fn n(p: [f64; 4]) -> [f64; 3] {
    [p[0], p[1], p[2]]
}
fn bounded_cell(
    mut planes: Vec<[f64; 4]>,
    bounds: [[f64; 3]; 2],
    offset: usize,
) -> Result<Hull, String> {
    if planes.len() > 64 || planes.is_empty() {
        return Err("Missing BSP cell exceeds plane budget".into());
    }
    for p in &mut planes {
        let length = dot(n(*p), n(*p)).sqrt();
        if !length.is_finite() || length < 1e-12 {
            return Err("Invalid missing-cell plane".into());
        }
        *p = p.map(|v| v / length);
    }
    let original = planes.len();
    for axis in 0..3 {
        for (side, bound) in bounds.iter().enumerate() {
            let mut p = [0.; 4];
            p[axis] = if side == 0 { -1. } else { 1. };
            p[3] = p[axis] * bound[axis];
            planes.push(p);
        }
    }
    let mut vertices = vec![];
    for i in 0..planes.len() {
        for j in 0..i {
            for k in 0..j {
                let a = planes[i];
                let b = planes[j];
                let c = planes[k];
                let bc = cross(n(b), n(c));
                let ca = cross(n(c), n(a));
                let ab = cross(n(a), n(b));
                let det = dot(n(a), bc);
                if det.abs() < 1e-10 {
                    continue;
                }
                let v = std::array::from_fn(|d| (a[3] * bc[d] + b[3] * ca[d] + c[3] * ab[d]) / det);
                if planes.iter().all(|p| dot(n(*p), v) <= p[3] + 1e-6) {
                    vertices.push(v);
                }
            }
        }
    }
    if vertices.len() < 4 {
        return Err("Missing BSP cell has no bounded volume".into());
    }
    let origin = vertices[0];
    let delta = |v: [f64; 3]| std::array::from_fn(|i| v[i] - origin[i]);
    let v1 = vertices
        .iter()
        .map(|v| delta(*v))
        .find(|v| dot(*v, *v) > 1e-10)
        .ok_or("Degenerate missing BSP cell")?;
    let v2 = vertices
        .iter()
        .map(|v| cross(v1, delta(*v)))
        .find(|v| dot(*v, *v) > 1e-10)
        .ok_or("Degenerate missing BSP cell")?;
    if !vertices.iter().any(|v| dot(v2, delta(*v)).abs() > 1e-8) {
        return Err("Planar missing BSP cell".into());
    }
    let mut tight = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
    for v in &vertices {
        for axis in 0..3 {
            if (v[axis] - bounds[0][axis]).abs() < 1e-5 || (v[axis] - bounds[1][axis]).abs() < 1e-5
            {
                return Err("Missing BSP cell reaches artificial bounds".into());
            }
            tight[0][axis] = tight[0][axis].min(v[axis]);
            tight[1][axis] = tight[1][axis].max(v[axis]);
        }
    }
    if (0..3).any(|i| tight[1][i] - tight[0][i] < 1e-5) {
        return Err("Degenerate missing BSP cell".into());
    }
    planes.truncate(original);
    Ok(Hull {
        offset,
        planes,
        bounds: tight,
    })
}
pub fn complete(bsp: &Bsp, solid: &SolidBsp) -> Result<HullSet, String> {
    let mut set = HullSet::read(bsp, solid)?;
    if set.missing_solid_leaves == 0 {
        return Ok(set);
    }
    solid.validate()?;
    let expected = set.missing_solid_leaves;
    let mut bounds = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
    for p in &bsp.points {
        for i in 0..3 {
            bounds[0][i] = bounds[0][i].min(p[i] as f64);
            bounds[1][i] = bounds[1][i].max(p[i] as f64);
        }
    }
    if bounds.iter().flatten().any(|v| !v.is_finite()) {
        return Err("Missing BSP cell has no original bounds".into());
    }
    let mut stack = vec![(0, solid.root_outside, Vec::new())];
    let mut budget = bsp.nodes.len().saturating_mul(16);
    let mut added = 0;
    while let Some((idx, outside, path)) = stack.pop() {
        if budget == 0 {
            return Err("Missing-cell traversal exceeds budget".into());
        }
        budget -= 1;
        let node = &bsp.nodes[idx];
        let csg = node.vertex_count != 0 && (node.flags & 31) & 33 == 0;
        for (child, space, sign) in [
            (node.front, outside || csg, -1.),
            (node.back, outside && !csg, 1.),
        ] {
            let mut path = path.clone();
            path.push(node.plane.map(|v| v as f64 * sign));
            if child >= 0 {
                stack.push((child as usize, space, path));
            } else if !space && node.collision_bound == -1 {
                let hull = bounded_cell(path, bounds, solid.hull_words.len() + added)?;
                set.hulls.push(hull);
                added += 1;
            }
        }
    }
    if added != expected {
        return Err("Missing-cell count disagrees with serialized model".into());
    }
    set.missing_solid_leaves = 0;
    Ok(set)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn cube() -> Vec<[f64; 4]> {
        vec![
            [1., 0., 0., 1.],
            [-1., 0., 0., 1.],
            [0., 1., 0., 1.],
            [0., -1., 0., 1.],
            [0., 0., 1., 1.],
            [0., 0., -1., 1.],
        ]
    }
    #[test]
    fn bounded_original_cell_retains_collisions() {
        let h = bounded_cell(cube(), [[-10.; 3], [10.; 3]], 0).unwrap();
        assert_eq!(h.bounds, [[-1.; 3], [1.; 3]]);
        assert!(h
            .sweep([-3., 0., 0.], [3., 0., 0.], [0.1; 3])
            .unwrap()
            .is_some());
        assert!(h
            .sweep([-3., 3., 0.], [3., 3., 0.], [0.1; 3])
            .unwrap()
            .is_none());
    }
    #[test]
    fn unbounded_cell_is_rejected_instead_of_clipping_to_a_box() {
        assert!(bounded_cell(vec![[1., 0., 0., 1.]], [[-10.; 3], [10.; 3]], 0).is_err());
    }
    #[test]
    fn planar_cell_is_rejected() {
        let mut p = cube();
        p.extend([[1., 1., 1., 0.], [-1., -1., -1., 0.]]);
        assert!(bounded_cell(p, [[-10.; 3], [10.; 3]], 0).is_err());
    }
    #[test]
    fn impossible_cell_is_rejected() {
        let mut p = cube();
        p.push([1., 0., 0., -2.]);
        assert!(bounded_cell(p, [[-10.; 3], [10.; 3]], 0).is_err());
    }
}
