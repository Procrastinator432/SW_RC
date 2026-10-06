//! Queries against visible diagnostic geometry; not Unreal collision or pawn sweeps.
use crate::Scene;

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub triangle: usize,
    pub fraction: f64,
    pub position: [f64; 3],
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
impl Scene {
    /// Nearest two-sided triangle hit along a finite segment, including its endpoints.
    /// Render visibility, actor collision flags and solid BSP semantics differ.
    pub fn trace_geometry(&self, start: [f64; 3], end: [f64; 3]) -> Result<Option<Hit>, String> {
        if start
            .iter()
            .chain(end.iter())
            .any(|v| !v.is_finite() || v.abs() > 1e9)
        {
            return Err("Invalid trace endpoints".into());
        }
        let direction = sub(end, start);
        if dot(direction, direction) < 1e-20 {
            return Err("Zero-length trace".into());
        }
        let mut nearest: Option<Hit> = None;
        for (triangle, t) in self.triangles.iter().enumerate() {
            let [a, b, c] = t.points.map(|p| p.map(f64::from));
            let e1 = sub(b, a);
            let e2 = sub(c, a);
            let p = cross(direction, e2);
            let det = dot(e1, p);
            let scale = (dot(e1, e1) * dot(e2, e2) * dot(direction, direction)).sqrt();
            if scale == 0.0 || det.abs() <= scale * 1e-12 {
                continue;
            }
            let from = sub(start, a);
            let u = dot(from, p) / det;
            if !(0.0..=1.0).contains(&u) {
                continue;
            }
            let q = cross(from, e1);
            let v = dot(direction, q) / det;
            if v < 0.0 || u + v > 1.0 {
                continue;
            }
            let fraction = dot(e2, q) / det;
            if !(0.0..=1.0).contains(&fraction)
                || nearest.as_ref().is_some_and(|h| h.fraction <= fraction)
            {
                continue;
            }
            nearest = Some(Hit {
                triangle,
                fraction,
                position: std::array::from_fn(|i| start[i] + direction[i] * fraction),
            });
        }
        Ok(nearest)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use rc_package::geometry::Triangle;
    #[test]
    fn nearest_hit_both_directions_endpoints_and_invalid_segments() {
        let mut s = Scene::new(vec![
            Triangle {
                points: [[0., -1., -1.], [0., 1., -1.], [0., 0., 1.]],
                surface: 0,
            },
            Triangle {
                points: [[1., -1., -1.], [1., 1., -1.], [1., 0., 1.]],
                surface: 0,
            },
        ])
        .unwrap();
        let hit = s
            .trace_geometry([-1., 0., 0.], [2., 0., 0.])
            .unwrap()
            .unwrap();
        assert_eq!(hit.triangle, 0);
        assert!((hit.fraction - 1. / 3.).abs() < 1e-12);
        assert_eq!(
            s.trace_geometry([2., 0., 0.], [-1., 0., 0.])
                .unwrap()
                .unwrap()
                .triangle,
            1
        );
        s.triangles[0].points.swap(0, 2);
        assert_eq!(
            s.trace_geometry([-1., 0., 0.], [0., 0., 0.])
                .unwrap()
                .unwrap()
                .fraction,
            1.
        );
        assert!(s
            .trace_geometry([-1., 2., 0.], [2., 2., 0.])
            .unwrap()
            .is_none());
        assert!(s.trace_geometry([0.; 3], [0.; 3]).is_err());
        assert!(s.trace_geometry([f64::NAN, 0., 0.], [1.; 3]).is_err());
    }
}
