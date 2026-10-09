//! Explicit diagnostic raster rules, not original Direct3D 8 hardware equivalence.
use rc_package::vertex_shader::Evaluation;
#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub clip: [f64; 4],
    pub varying: [f64; 10],
}
impl Vertex {
    /// T0.x, T1.xy, T2.xy, D0.rgba, fog.x. T0.y stays diagnostic zero.
    pub fn from_output(v: &Evaluation) -> Result<Self, String> {
        for (register, mask) in [(0, 15u8), (1, 15), (3, 1), (4, 3), (5, 3), (11, 1)] {
            for axis in 0..4 {
                if mask & (1 << axis) != 0
                    && (!v.defined[register][axis] || !v.output[register][axis].is_finite())
                {
                    return Err("Incomplete/nonfinite shader raster output".into());
                }
            }
        }
        Ok(Self {
            clip: v.output[0].map(f64::from),
            varying: [
                v.output[3][0],
                v.output[4][0],
                v.output[4][1],
                v.output[5][0],
                v.output[5][1],
                v.output[1][0],
                v.output[1][1],
                v.output[1][2],
                v.output[1][3],
                v.output[11][0],
            ]
            .map(f64::from),
        })
    }
}
#[derive(Clone, Copy, Debug)]
pub enum Blend {
    Opaque,
    SourceAlphaAdditive,
}
#[derive(Clone, Copy, Debug)]
pub struct State {
    pub depth_test: bool,
    pub depth_write: bool,
    /// Diagnostic float alpha > reference/255, after clamping fragment RGBA.
    pub alpha_reference: Option<u8>,
    pub blend: Blend,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub covered: usize,
    pub depth_rejected: usize,
    pub alpha_rejected: usize,
    pub written: usize,
}
#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub varying: [f32; 10],
    pub depth: f32,
    pub pixel: usize,
}
pub struct Target {
    width: usize,
    height: usize,
    pixels: Vec<u32>,
    depth: Vec<f32>,
}
impl Target {
    pub fn new(width: usize, height: usize, background: u32) -> Result<Self, String> {
        if width == 0 || height == 0 || width > 1024 || height > 1024 {
            return Err("Shader raster dimensions outside 1..1024".into());
        }
        Ok(Self {
            width,
            height,
            pixels: vec![background; width * height],
            depth: vec![1.; width * height],
        })
    }
    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }
    pub fn depth(&self) -> &[f32] {
        &self.depth
    }
    /// Per-triangle validation precedes writes. Shader errors can leave prior pixels drawn.
    pub fn draw(
        &mut self,
        vertices: [Vertex; 3],
        state: State,
        shade: &mut dyn FnMut(Sample) -> Result<[f32; 4], String>,
    ) -> Result<Stats, String> {
        if vertices.iter().any(|v| {
            v.clip
                .iter()
                .chain(&v.varying)
                .any(|x| !x.is_finite() || x.abs() > 1e12)
        }) {
            return Err("Invalid/excessive shader raster vertex".into());
        }
        let polygon = clip(&vertices);
        let mut stats = Stats::default();
        for corner in 1..polygon.len().saturating_sub(1) {
            let mut v = [polygon[0], polygon[corner], polygon[corner + 1]];
            let project = |v: Vertex| {
                [
                    (v.clip[0] / v.clip[3] + 1.) * self.width as f64 * 0.5,
                    (1. - v.clip[1] / v.clip[3]) * self.height as f64 * 0.5,
                ]
            };
            let mut p = v.map(project);
            let mut area = edge(p[0], p[1], p[2]);
            if area == 0. {
                continue;
            }
            if area < 0. {
                v.swap(1, 2);
                p.swap(1, 2);
                area = -area;
            }
            let minx = p
                .iter()
                .map(|p| p[0])
                .fold(f64::INFINITY, f64::min)
                .floor()
                .max(0.) as usize;
            let maxx = p
                .iter()
                .map(|p| p[0])
                .fold(f64::NEG_INFINITY, f64::max)
                .ceil()
                .min(self.width as f64) as usize;
            let miny = p
                .iter()
                .map(|p| p[1])
                .fold(f64::INFINITY, f64::min)
                .floor()
                .max(0.) as usize;
            let maxy = p
                .iter()
                .map(|p| p[1])
                .fold(f64::NEG_INFINITY, f64::max)
                .ceil()
                .min(self.height as f64) as usize;
            let invw = v.map(|v| 1. / v.clip[3]);
            for y in miny..maxy {
                for x in minx..maxx {
                    let q = [x as f64 + 0.5, y as f64 + 0.5];
                    let e = [
                        edge(p[1], p[2], q),
                        edge(p[2], p[0], q),
                        edge(p[0], p[1], q),
                    ];
                    let edges = [(p[1], p[2]), (p[2], p[0]), (p[0], p[1])];
                    if (0..3).any(|i| e[i] < 0. || e[i] == 0. && !top_left(edges[i].0, edges[i].1))
                    {
                        continue;
                    }
                    stats.covered += 1;
                    let bary = e.map(|e| e / area);
                    let pixel = y * self.width + x;
                    let z = (((bary[0] * v[0].clip[2] * invw[0] + bary[1] * v[1].clip[2] * invw[1])
                        + bary[2] * v[2].clip[2] * invw[2]) as f32)
                        .clamp(0., 1.);
                    if !z.is_finite() {
                        return Err("Nonfinite shader raster depth".into());
                    }
                    if state.depth_test && z > self.depth[pixel] {
                        stats.depth_rejected += 1;
                        continue;
                    }
                    let denominator = (bary[0] * invw[0] + bary[1] * invw[1]) + bary[2] * invw[2];
                    let varying = std::array::from_fn(|i| {
                        (((bary[0] * v[0].varying[i] * invw[0]
                            + bary[1] * v[1].varying[i] * invw[1])
                            + bary[2] * v[2].varying[i] * invw[2])
                            / denominator) as f32
                    });
                    if varying.iter().any(|v| !v.is_finite()) {
                        return Err("Nonfinite interpolated varying".into());
                    }
                    let color = shade(Sample {
                        varying,
                        depth: z,
                        pixel,
                    })?;
                    if color.iter().any(|v| !v.is_finite()) {
                        return Err("Nonfinite shader raster color".into());
                    }
                    let color = color.map(|v| v.clamp(0., 1.));
                    if state
                        .alpha_reference
                        .is_some_and(|reference| color[3] <= reference as f32 / 255.)
                    {
                        stats.alpha_rejected += 1;
                        continue;
                    }
                    self.pixels[pixel] = match state.blend {
                        Blend::SourceAlphaAdditive => {
                            crate::fragment::composite(color, self.pixels[pixel])?
                        }
                        Blend::Opaque => {
                            0xff000000
                                | ((color[0] * 255.).round() as u32) << 16
                                | ((color[1] * 255.).round() as u32) << 8
                                | (color[2] * 255.).round() as u32
                        }
                    };
                    if state.depth_write {
                        self.depth[pixel] = z;
                    }
                    stats.written += 1;
                }
            }
        }
        Ok(stats)
    }
}
fn edge(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}
fn top_left(a: [f64; 2], b: [f64; 2]) -> bool {
    b[1] < a[1] || b[1] == a[1] && b[0] > a[0]
}
fn distance(v: Vertex, plane: usize) -> f64 {
    let [x, y, z, w] = v.clip;
    match plane {
        0 => w - 1e-6,
        1 => w + x,
        2 => w - x,
        3 => w + y,
        4 => w - y,
        5 => z,
        _ => w - z,
    }
}
fn clip(vertices: &[Vertex; 3]) -> Vec<Vertex> {
    let mut polygon = vertices.to_vec();
    for plane in 0..7 {
        if polygon.is_empty() {
            break;
        }
        let mut result = vec![];
        let mut previous = *polygon.last().unwrap();
        let mut pd = distance(previous, plane);
        for current in polygon {
            let cd = distance(current, plane);
            if (pd >= 0.) != (cd >= 0.) {
                let t = pd / (pd - cd);
                result.push(Vertex {
                    clip: std::array::from_fn(|i| {
                        previous.clip[i] + t * (current.clip[i] - previous.clip[i])
                    }),
                    varying: std::array::from_fn(|i| {
                        previous.varying[i] + t * (current.varying[i] - previous.varying[i])
                    }),
                });
            }
            if cd >= 0. {
                result.push(current);
            }
            previous = current;
            pd = cd;
        }
        polygon = result;
    }
    polygon
}
#[cfg(test)]
mod tests {
    use super::*;
    fn vertex(x: f64, y: f64, z: f64, w: f64, u: f64) -> Vertex {
        Vertex {
            clip: [x, y, z, w],
            varying: [u; 10],
        }
    }
    fn state() -> State {
        State {
            depth_test: true,
            depth_write: true,
            alpha_reference: None,
            blend: Blend::Opaque,
        }
    }
    #[test]
    fn shared_edge_drawn_once_for_both_windings() {
        let a = vertex(-1., 1., 0.5, 1., 0.);
        let b = vertex(1., 1., 0.5, 1., 0.);
        let c = vertex(1., -1., 0.5, 1., 0.);
        let d = vertex(-1., -1., 0.5, 1., 0.);
        for reverse in [false, true] {
            let mut t = Target::new(8, 8, 0xff000000).unwrap();
            let mut hits = [0; 64];
            for mut tri in [[a, b, c], [a, c, d]] {
                if reverse {
                    tri.swap(1, 2);
                }
                t.draw(tri, state(), &mut |s| {
                    hits[s.pixel] += 1;
                    Ok([1.; 4])
                })
                .unwrap();
            }
            assert!(hits.iter().all(|h| *h == 1));
        }
    }
    #[test]
    fn perspective_varying_and_linear_depth() {
        let mut t = Target::new(2, 2, 0).unwrap();
        let mut seen = None;
        t.draw(
            [
                vertex(-1., 1., 0.2, 1., 0.),
                vertex(2., 2., 1.6, 2., 1.),
                vertex(-1., -1., 0.4, 1., 0.),
            ],
            state(),
            &mut |s| {
                if s.pixel == 0 {
                    seen = Some(s);
                }
                Ok([1.; 4])
            },
        )
        .unwrap();
        let s = seen.unwrap();
        assert!((s.varying[0] - 1. / 7.).abs() < 1e-6);
        assert!((s.depth - 0.4).abs() < 1e-6);
    }
    #[test]
    fn clip_planes_and_negative_w_do_not_escape_bounds() {
        for point in [
            [-2., 0., 0.5, 1.],
            [2., 0., 0.5, 1.],
            [0., -2., 0.5, 1.],
            [0., 2., 0.5, 1.],
            [0., 0., -1., 1.],
            [0., 0., 2., 1.],
            [0., 0., 0., -1.],
        ] {
            let outside = Vertex {
                clip: point,
                varying: [point[0]; 10],
            };
            let polygon = clip(&[
                outside,
                vertex(-0.25, -0.25, 0.5, 1., -0.25),
                vertex(0.25, 0.25, 0.5, 1., 0.25),
            ]);
            assert!(!polygon.is_empty());
            for v in polygon {
                assert!((0..7).all(|plane| distance(v, plane) >= -1e-12));
                assert!((v.varying[0] - v.clip[0]).abs() < 1e-12);
            }
        }
        let mut t = Target::new(16, 16, 0).unwrap();
        let s = t
            .draw(
                [
                    vertex(-2., 0., -0.3, 1., 0.),
                    vertex(0., 2., 0.5, 1., 0.),
                    vertex(1., -1., 2., 1., 0.),
                ],
                state(),
                &mut |_| Ok([1.; 4]),
            )
            .unwrap();
        assert!(s.written > 0);
        assert!(t.depth().iter().all(|z| *z >= 0. && *z <= 1.));
        let s = t
            .draw([vertex(-1., 0., 0., -1., 0.); 3], state(), &mut |_| {
                panic!("fully behind")
            })
            .unwrap();
        assert_eq!(s.covered, 0);
    }
    #[test]
    fn alpha_rejection_preserves_depth_and_depth_write_is_independent() {
        let tri = [
            vertex(-1., 1., 0.2, 1., 0.),
            vertex(1., 1., 0.2, 1., 0.),
            vertex(-1., -1., 0.2, 1., 0.),
        ];
        let mut t = Target::new(4, 4, 0xff000000).unwrap();
        let mut st = state();
        st.alpha_reference = Some(128);
        let s = t.draw(tri, st, &mut |_| Ok([1., 0., 0., 0.5])).unwrap();
        assert!(s.alpha_rejected > 0);
        assert!(t.depth().iter().all(|z| *z == 1.));
        let s = t
            .draw(tri, st, &mut |_| Ok([1., 0., 0., 128f32 / 255.]))
            .unwrap();
        assert_eq!(s.alpha_rejected, s.covered);
        st.alpha_reference = None;
        st.depth_write = false;
        t.draw(tri, st, &mut |_| Ok([0., 1., 0., 1.])).unwrap();
        assert!(t.depth().iter().all(|z| *z == 1.));
        st.depth_write = true;
        t.draw(tri, st, &mut |_| Ok([0., 0., 1., 1.])).unwrap();
        let farther = tri.map(|mut v| {
            v.clip[2] = 0.8;
            v
        });
        let s = t
            .draw(farther, st, &mut |_| panic!("depth reject before shading"))
            .unwrap();
        assert_eq!(s.covered, s.depth_rejected);
        let s = t.draw(tri, st, &mut |_| Ok([1.; 4])).unwrap();
        assert_eq!(s.covered, s.written);
        st.depth_test = false;
        let s = t.draw(farther, st, &mut |_| Ok([1.; 4])).unwrap();
        assert_eq!(s.covered, s.written);
    }
    #[test]
    fn additive_composite_clamps_and_invalid_input_is_rejected() {
        let tri = [
            vertex(-1., 1., 0.2, 1., 0.),
            vertex(1., 1., 0.2, 1., 0.),
            vertex(-1., -1., 0.2, 1., 0.),
        ];
        let mut t = Target::new(4, 4, 0xff0000ff).unwrap();
        let mut st = state();
        st.blend = Blend::SourceAlphaAdditive;
        t.draw(tri, st, &mut |_| Ok([2., 0., 0., 0.5])).unwrap();
        assert_eq!(t.pixels()[0], 0xff8000ff);
        let mut bad = tri;
        bad[0].varying[0] = f64::NAN;
        assert!(t.draw(bad, st, &mut |_| Ok([1.; 4])).is_err());
        assert!(Target::new(1025, 1, 0).is_err());
    }
}
