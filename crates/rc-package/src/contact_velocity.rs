//! Closest feasible velocity in the cone n·v >= 0 of simultaneous contacts.
//! In three dimensions the optimum lies in the interior, on one plane, on a
//! pair's intersection line, or at the origin. No iterative convergence budget.
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
pub fn project(velocity: [f64; 3], normals: &[[f64; 3]]) -> Result<[f64; 3], String> {
    if velocity.iter().any(|v| !v.is_finite()) || normals.len() > 64 {
        return Err("Invalid contact velocity input".into());
    }
    let normals = normals
        .iter()
        .map(|n| {
            let length = dot(*n, *n).sqrt();
            if !length.is_finite() || length < 1e-12 {
                return Err("Invalid contact normal".into());
            }
            Ok(n.map(|v| v / length))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let feasible = |v: [f64; 3]| normals.iter().all(|n| dot(*n, v) >= -1e-8);
    if feasible(velocity) {
        return Ok(velocity);
    }
    let mut best = [0.; 3];
    let mut cost = dot(velocity, velocity);
    let mut consider = |v: [f64; 3]| {
        let delta = std::array::from_fn(|i| v[i] - velocity[i]);
        let c = dot(delta, delta);
        if c < cost && feasible(v) {
            cost = c;
            best = v;
        }
    };
    for n in &normals {
        let inward = dot(*n, velocity);
        consider(std::array::from_fn(|i| velocity[i] - n[i] * inward));
    }
    for (i, a) in normals.iter().enumerate() {
        for b in &normals[i + 1..] {
            let axis = [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ];
            let length = dot(axis, axis).sqrt();
            if length < 1e-12 {
                continue;
            }
            let axis = axis.map(|v| v / length);
            let speed = dot(axis, velocity);
            consider(axis.map(|v| v * speed));
        }
    }
    Ok(best)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn acute_barrier_corner_projects_without_an_iteration_failure() {
        let n = [[1., 0., 0.], [-0.999, 0.04, 0.]];
        assert_eq!(project([-100., -100., 50.], &n).unwrap(), [0., 0., 50.]);
        assert_eq!(project([0., 10., 50.], &n).unwrap(), [0., 10., 50.]);
    }
    #[test]
    fn scaled_duplicate_planes_and_three_sided_corner() {
        assert_eq!(
            project([-3., 4., 5.], &[[2., 0., 0.], [1., 0., 0.]]).unwrap(),
            [0., 4., 5.]
        );
        assert_eq!(
            project([-3., -4., -5.], &[[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]).unwrap(),
            [0.; 3]
        );
        assert!(project([0.; 3], &[[0.; 3]]).is_err());
        assert_eq!(project([1., 2., 3.], &[]).unwrap(), [1., 2., 3.]);
    }
}
