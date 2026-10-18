//! Selected free rigid boxes from original KMeshProps, with a portable impulse
//! solver. This is not a binary-compatible recreation of MathEngine Karma.
use crate::{Game, STEP};
use serde::{Deserialize, Serialize};
type V = [f64; 3];
type M = [V; 3];
fn add(a: V, b: V) -> V {
    std::array::from_fn(|i| a[i] + b[i])
}
fn sub(a: V, b: V) -> V {
    std::array::from_fn(|i| a[i] - b[i])
}
fn mul(a: V, s: f64) -> V {
    a.map(|v| v * s)
}
fn dot(a: V, b: V) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(a: V) -> V {
    mul(a, 1. / dot(a, a).sqrt())
}
fn mv(m: M, v: V) -> V {
    m.map(|row| dot(row, v))
}
fn transpose(m: M) -> M {
    std::array::from_fn(|i| std::array::from_fn(|j| m[j][i]))
}
fn mm(a: M, b: M) -> M {
    std::array::from_fn(|i| std::array::from_fn(|j| dot(a[i], transpose(b)[j])))
}
fn inverse(m: M) -> Option<M> {
    let c = [cross(m[1], m[2]), cross(m[2], m[0]), cross(m[0], m[1])];
    let det = dot(m[0], c[0]);
    (det > 1e-12 && det.is_finite()).then(|| transpose(c).map(|r| mul(r, 1. / det)))
}
fn rotation(q: [f64; 4]) -> M {
    let [w, x, y, z] = q;
    [
        [
            1. - 2. * (y * y + z * z),
            2. * (x * y - z * w),
            2. * (x * z + y * w),
        ],
        [
            2. * (x * y + z * w),
            1. - 2. * (x * x + z * z),
            2. * (y * z - x * w),
        ],
        [
            2. * (x * z - y * w),
            2. * (y * z + x * w),
            1. - 2. * (x * x + y * y),
        ],
    ]
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub original: u32,
    pub center: V,
    pub vertices: [V; 8],
    /// Mass-scaled original inertia, rotated to the initial world basis.
    pub inertia: M,
    pub mass: f64,
    pub friction: f64,
    pub restitution: f64,
    pub linear_damping: f64,
    pub angular_damping: f64,
    pub gravity: V,
    pub initial_velocity: V,
    pub initial_angular_velocity: V,
}
impl Binding {
    pub fn validate(&self) -> Result<(), String> {
        if self.original == 0
            || self
                .center
                .iter()
                .chain(self.vertices.iter().flatten())
                .chain(self.inertia.iter().flatten())
                .chain(self.gravity.iter())
                .chain(self.initial_velocity.iter())
                .chain(self.initial_angular_velocity.iter())
                .chain([
                    &self.mass,
                    &self.friction,
                    &self.restitution,
                    &self.linear_damping,
                    &self.angular_damping,
                ])
                .any(|v| !v.is_finite() || v.abs() > 1e8)
            || !(0.001..=10000.).contains(&self.mass)
            || !(0. ..=10.).contains(&self.friction)
            || !(0. ..=1.).contains(&self.restitution)
            || self.linear_damping < 0.
            || self.angular_damping < 0.
            || inverse(self.inertia).is_none()
            || (0..3).any(|i| self.inertia[i][i] <= 0.)
            || (0..3)
                .any(|i| (0..3).any(|j| (self.inertia[i][j] - self.inertia[j][i]).abs() > 1e-6))
        {
            return Err("Invalid selected rigid body".into());
        }
        let axes = self.axes();
        if axes.iter().flatten().any(|v| !v.is_finite())
            || dot(axes[0], axes[1]).abs() > 1e-5
            || dot(axes[0], axes[2]).abs() > 1e-5
            || dot(axes[1], axes[2]).abs() > 1e-5
        {
            return Err("Invalid rigid box axes".into());
        }
        Ok(())
    }
    fn axes(&self) -> M {
        [
            unit(sub(self.vertices[4], self.vertices[0])),
            unit(sub(self.vertices[1], self.vertices[0])),
            unit(sub(self.vertices[3], self.vertices[0])),
        ]
    }
    pub fn initial(&self) -> Body {
        Body {
            original: self.original,
            active: false,
            center: self.center,
            orientation: [1., 0., 0., 0.],
            velocity: self.initial_velocity,
            angular_velocity: self.initial_angular_velocity,
            quiet_steps: 0,
            sleeping: false,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Body {
    pub original: u32,
    pub active: bool,
    pub center: V,
    pub orientation: [f64; 4],
    pub velocity: V,
    pub angular_velocity: V,
    pub quiet_steps: u32,
    pub sleeping: bool,
}
impl Body {
    pub fn valid(&self, b: &Binding) -> bool {
        self.original == b.original
            && self
                .center
                .iter()
                .chain(self.orientation.iter())
                .chain(self.velocity.iter())
                .chain(self.angular_velocity.iter())
                .all(|v| v.is_finite() && v.abs() < 1e8)
            && (self.orientation.iter().map(|v| v * v).sum::<f64>() - 1.).abs() < 1e-8
            && self.quiet_steps <= 480
            && (!self.sleeping
                || (self.active
                    && self.quiet_steps == 480
                    && self.velocity == [0.; 3]
                    && self.angular_velocity == [0.; 3]))
            && (self.active || self == &b.initial())
    }
    pub fn point(&self, b: &Binding, p: V) -> V {
        add(
            self.center,
            mv(rotation(self.orientation), sub(p, b.center)),
        )
    }
    fn vertices(&self, b: &Binding) -> [V; 8] {
        b.vertices.map(|v| self.point(b, v))
    }
    fn inverse_inertia(&self, b: &Binding) -> M {
        let r = rotation(self.orientation);
        mm(
            mm(r, inverse(b.inertia).expect("validated inertia")),
            transpose(r),
        )
    }
    fn velocity_at(&self, p: V) -> V {
        add(
            self.velocity,
            cross(self.angular_velocity, sub(p, self.center)),
        )
    }
    fn impulse(&mut self, b: &Binding, p: V, j: V) {
        self.velocity = add(self.velocity, mul(j, 1. / b.mass));
        self.angular_velocity = add(
            self.angular_velocity,
            mv(self.inverse_inertia(b), cross(sub(p, self.center), j)),
        );
    }
    fn denominator(&self, b: &Binding, p: V, n: V) -> f64 {
        1. / b.mass
            + dot(
                n,
                cross(
                    mv(self.inverse_inertia(b), cross(sub(p, self.center), n)),
                    sub(p, self.center),
                ),
            )
    }
}
#[derive(Clone)]
pub struct Terrain {
    pub points: [V; 3],
    bounds: [V; 2],
}
impl Terrain {
    pub fn new(points: [V; 3]) -> Option<Self> {
        (dot(
            cross(sub(points[1], points[0]), sub(points[2], points[0])),
            cross(sub(points[1], points[0]), sub(points[2], points[0])),
        ) > 1e-12)
            .then(|| Self {
                points,
                bounds: bounds(&points),
            })
    }
}
fn bounds(points: &[V]) -> [V; 2] {
    std::array::from_fn(|side| {
        std::array::from_fn(|i| {
            points.iter().map(|p| p[i]).fold(
                if side == 0 {
                    f64::INFINITY
                } else {
                    f64::NEG_INFINITY
                },
                |a, b| if side == 0 { a.min(b) } else { a.max(b) },
            )
        })
    })
}
fn overlaps(a: [V; 2], b: [V; 2]) -> bool {
    (0..3).all(|i| a[0][i] <= b[1][i] + 0.02 && b[0][i] <= a[1][i] + 0.02)
}
fn projection(points: &[V], n: V) -> [f64; 2] {
    points
        .iter()
        .fold([f64::INFINITY, f64::NEG_INFINITY], |r, p| {
            let d = dot(*p, n);
            [r[0].min(d), r[1].max(d)]
        })
}
fn support_center(points: &[V], n: V, minimum: bool) -> V {
    let support = projection(points, n)[usize::from(!minimum)];
    let mut sum = [0.; 3];
    let mut count = 0.;
    for p in points
        .iter()
        .filter(|p| (dot(**p, n) - support).abs() < 0.05)
    {
        sum = add(sum, *p);
        count += 1.;
    }
    mul(sum, 1. / count)
}
#[derive(Clone, Copy)]
struct Contact {
    point: V,
    normal: V,
    depth: f64,
}
fn separation(a: &[V], b: &[V], axes: impl Iterator<Item = V>) -> Option<(V, f64)> {
    let mut result = ([0.; 3], f64::INFINITY);
    for n in axes {
        if dot(n, n) < 1e-14 {
            continue;
        }
        let n = unit(n);
        let pa = projection(a, n);
        let pb = projection(b, n);
        let positive = pb[1] - pa[0];
        let negative = pa[1] - pb[0];
        if positive < -0.02 || negative < -0.02 {
            return None;
        }
        let (normal, depth) = if positive < negative {
            (n, positive)
        } else {
            (mul(n, -1.), negative)
        };
        if depth < result.1 {
            result = (normal, depth);
        }
    }
    Some(result)
}
fn triangle_contacts(vertices: &[V; 8], axes: M, t: &Terrain) -> Vec<Contact> {
    if !overlaps(bounds(vertices), t.bounds) {
        return vec![];
    }
    let edges = std::array::from_fn::<_, 3, _>(|i| sub(t.points[(i + 1) % 3], t.points[i]));
    let normal = cross(edges[0], edges[1]);
    let candidates = axes
        .into_iter()
        .chain([normal])
        .chain(axes.into_iter().flat_map(|a| edges.map(|e| cross(a, e))));
    let Some((n, depth)) = separation(vertices, &t.points, candidates) else {
        return vec![];
    };
    let minimum = projection(vertices, n)[0];
    let tn = unit(normal);
    let inside = |p: V| (0..3).all(|i| dot(cross(edges[i], sub(p, t.points[i])), tn) >= -0.05);
    let mut contacts = vec![];
    for v in vertices {
        if dot(*v, n) - minimum > 0.05 {
            continue;
        }
        let p = sub(*v, mul(tn, dot(sub(*v, t.points[0]), tn)));
        if inside(p) {
            contacts.push(Contact {
                point: p,
                normal: n,
                depth,
            });
        }
    }
    if contacts.is_empty() {
        let p = vertices
            .iter()
            .min_by(|a, b| dot(**a, n).total_cmp(&dot(**b, n)))
            .unwrap();
        contacts.push(Contact {
            point: *p,
            normal: n,
            depth,
        });
    }
    contacts
}
struct Constraint {
    contact: Contact,
    normal_impulse: f64,
    tangent_impulse: V,
    target: f64,
}
fn apply_static(body: &mut Body, b: &Binding, constraint: &mut Constraint) {
    let c = constraint.contact;
    let vn = dot(body.velocity_at(c.point), c.normal);
    let total = (constraint.normal_impulse
        + (constraint.target - vn) / body.denominator(b, c.point, c.normal))
    .max(0.);
    let j = total - constraint.normal_impulse;
    constraint.normal_impulse = total;
    body.impulse(b, c.point, mul(c.normal, j));
    let v = body.velocity_at(c.point);
    let tangent = sub(v, mul(c.normal, dot(v, c.normal)));
    let speed = dot(tangent, tangent).sqrt();
    if speed > 1e-10 {
        let t = mul(tangent, 1. / speed);
        let mut friction = sub(
            constraint.tangent_impulse,
            mul(t, speed / body.denominator(b, c.point, t)),
        );
        let magnitude = dot(friction, friction).sqrt();
        let limit = b.friction * total;
        if magnitude > limit {
            friction = mul(friction, limit / magnitude);
        }
        body.impulse(b, c.point, sub(friction, constraint.tangent_impulse));
        constraint.tangent_impulse = friction;
    }
}
pub fn step(bindings: &[Binding], bodies: &mut [Body], terrain: &[Terrain], dt: f64) {
    step_with_obstacles(bindings, bodies, terrain, None, None, dt);
}
fn step_with_obstacles(
    bindings: &[Binding],
    bodies: &mut [Body],
    terrain: &[Terrain],
    prepared: Option<&Prepared>,
    pawn: Option<(V, V)>,
    dt: f64,
) {
    for _ in 0..8 {
        let h = dt / 8.;
        for (b, s) in bindings
            .iter()
            .zip(bodies.iter_mut())
            .filter(|(_, s)| s.active && !s.sleeping)
        {
            s.velocity = mul(
                add(s.velocity, mul(b.gravity, h)),
                1. / (1. + b.linear_damping * h),
            );
            let r = rotation(s.orientation);
            let inertia = mm(mm(r, b.inertia), transpose(r));
            let gyroscopic = mv(
                s.inverse_inertia(b),
                cross(s.angular_velocity, mv(inertia, s.angular_velocity)),
            );
            s.angular_velocity = sub(s.angular_velocity, mul(gyroscopic, h));
            s.angular_velocity = mul(s.angular_velocity, 1. / (1. + b.angular_damping * h));
            s.center = add(s.center, mul(s.velocity, h));
            let [w, x, y, z] = s.orientation;
            let [a, b, c] = mul(s.angular_velocity, h * 0.5);
            let dq = [
                -a * x - b * y - c * z,
                a * w + b * z - c * y,
                -a * z + b * w + c * x,
                a * y - b * x + c * w,
            ];
            for (v, d) in s.orientation.iter_mut().zip(dq) {
                *v += d;
            }
            let length = s.orientation.iter().map(|v| v * v).sum::<f64>().sqrt();
            s.orientation = s.orientation.map(|v| v / length);
        }
        let mut touched = vec![false; bodies.len()];
        let mut constraints: Vec<Vec<Constraint>> = bindings
            .iter()
            .zip(bodies.iter())
            .enumerate()
            .map(|(i, (b, s))| {
                if !s.active || s.sleeping {
                    return vec![];
                }
                let vertices = s.vertices(b);
                let axes = b.axes().map(|a| mv(rotation(s.orientation), a));
                let body_bounds = bounds(&vertices);
                let indices =
                    prepared.map_or_else(|| (0..terrain.len()).collect(), |p| p.query(body_bounds));
                indices
                    .into_iter()
                    .map(|i| &terrain[i])
                    .filter(|t| overlaps(body_bounds, t.bounds))
                    .flat_map(|t| triangle_contacts(&vertices, axes, t))
                    .map(|c| {
                        touched[i] = true;
                        let vn = dot(s.velocity_at(c.point), c.normal);
                        Constraint {
                            contact: c,
                            normal_impulse: 0.,
                            tangent_impulse: [0.; 3],
                            target: (0.15 * (c.depth - 0.02).max(0.) / h).min(50.)
                                + if vn < -20. { -b.restitution * vn } else { 0. },
                        }
                    })
                    .collect()
            })
            .collect();
        for _ in 0..6 {
            for (i, (b, s)) in bindings
                .iter()
                .zip(bodies.iter_mut())
                .enumerate()
                .filter(|(_, (_, s))| s.active && !s.sleeping)
            {
                for c in &mut constraints[i] {
                    apply_static(s, b, c);
                }
            }
            for i in 0..bodies.len() {
                for j in i + 1..bodies.len() {
                    let (left, right) = bodies.split_at_mut(j);
                    let a = &mut left[i];
                    let b = &mut right[0];
                    if !a.active || !b.active || (a.sleeping && b.sleeping) {
                        continue;
                    }
                    let ba = &bindings[i];
                    let bb = &bindings[j];
                    let va = a.vertices(ba);
                    let vb = b.vertices(bb);
                    if !overlaps(bounds(&va), bounds(&vb)) {
                        continue;
                    }
                    let aa = ba.axes().map(|v| mv(rotation(a.orientation), v));
                    let ab = bb.axes().map(|v| mv(rotation(b.orientation), v));
                    let Some((n, depth)) = separation(
                        &va,
                        &vb,
                        aa.into_iter()
                            .chain(ab)
                            .chain(aa.into_iter().flat_map(|x| ab.map(|y| cross(x, y)))),
                    ) else {
                        continue;
                    };
                    let pa = support_center(&va, n, true);
                    let pb = support_center(&vb, n, false);
                    let point = mul(add(pa, pb), 0.5);
                    let relative = dot(sub(a.velocity_at(point), b.velocity_at(point)), n);
                    let impulse = ((0.1 * (depth - 0.02).max(0.) / h).min(30.) - relative).max(0.)
                        / (a.denominator(ba, point, n) + b.denominator(bb, point, n));
                    if impulse > 0.001 {
                        if a.sleeping {
                            a.quiet_steps = 0;
                        }
                        if b.sleeping {
                            b.quiet_steps = 0;
                        }
                        a.sleeping = false;
                        b.sleeping = false;
                    }
                    a.impulse(ba, point, mul(n, impulse));
                    b.impulse(bb, point, mul(n, -impulse));
                    let relative = sub(a.velocity_at(point), b.velocity_at(point));
                    let tangent = sub(relative, mul(n, dot(relative, n)));
                    let speed = dot(tangent, tangent).sqrt();
                    if speed > 1e-10 {
                        let t = mul(tangent, 1. / speed);
                        let friction = (speed
                            / (a.denominator(ba, point, t) + b.denominator(bb, point, t)))
                        .min(ba.friction * bb.friction * impulse);
                        a.impulse(ba, point, mul(t, -friction));
                        b.impulse(bb, point, mul(t, friction));
                    }
                    touched[i] = true;
                    touched[j] = true;
                }
            }
        }
        if let Some((center, extent)) = pawn {
            for (b, s) in bindings
                .iter()
                .zip(bodies.iter_mut())
                .filter(|(_, s)| s.active)
            {
                resolve_pawn(b, s, center, extent);
            }
        }
        for (i, s) in bodies
            .iter_mut()
            .enumerate()
            .filter(|(_, s)| s.active && !s.sleeping)
        {
            // Retain the quiet window through sub-millimetre contact bounce.
            // A body leaving an edge accelerates beyond this bound in <6 ms,
            // well before the one-second sleep window can complete.
            if (touched[i] || s.quiet_steps > 0)
                && dot(s.velocity, s.velocity) < 25.
                && dot(s.angular_velocity, s.angular_velocity) < 0.0004
            {
                s.quiet_steps = (s.quiet_steps + 1).min(480);
            } else {
                s.quiet_steps = 0;
            }
            if s.quiet_steps == 480 {
                s.sleeping = true;
                s.velocity = [0.; 3];
                s.angular_velocity = [0.; 3];
            }
        }
    }
}
pub struct Prepared {
    pub terrain: Vec<Terrain>,
    cells: std::collections::BTreeMap<(i32, i32), Vec<usize>>,
    large: Vec<usize>,
}
impl Prepared {
    pub fn new(terrain: Vec<Terrain>) -> Self {
        let mut result = Self {
            terrain,
            cells: std::collections::BTreeMap::new(),
            large: vec![],
        };
        for (i, t) in result.terrain.iter().enumerate() {
            let lo = t.bounds[0].map(|v| (v / 256.).floor() as i32);
            let hi = t.bounds[1].map(|v| (v / 256.).floor() as i32);
            if i64::from(hi[0] - lo[0] + 1) * i64::from(hi[1] - lo[1] + 1) > 4096 {
                result.large.push(i);
                continue;
            }
            for x in lo[0]..=hi[0] {
                for y in lo[1]..=hi[1] {
                    result.cells.entry((x, y)).or_default().push(i);
                }
            }
        }
        result
    }
    fn query(&self, bounds: [V; 2]) -> Vec<usize> {
        let lo = bounds[0].map(|v| ((v - 0.02) / 256.).floor() as i32);
        let hi = bounds[1].map(|v| ((v + 0.02) / 256.).floor() as i32);
        let mut result = self.large.clone();
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                if let Some(indices) = self.cells.get(&(x, y)) {
                    result.extend(indices);
                }
            }
        }
        result.sort_unstable();
        result.dedup();
        result
    }
}
fn resolve_pawn(b: &Binding, s: &mut Body, center: V, extent: V) {
    let vertices: [V; 8] = std::array::from_fn(|m| {
        std::array::from_fn(|i| {
            center[i]
                + if m & (1 << i) == 0 {
                    -extent[i]
                } else {
                    extent[i]
                }
        })
    });
    let body = s.vertices(b);
    if !overlaps(bounds(&body), bounds(&vertices)) {
        return;
    }
    let axes = b.axes().map(|n| mv(rotation(s.orientation), n));
    let world = [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]];
    let Some((n, depth)) = separation(
        &body,
        &vertices,
        axes.into_iter()
            .chain(world)
            .chain(axes.into_iter().flat_map(|a| world.map(|w| cross(a, w)))),
    ) else {
        return;
    };
    if depth <= 0. {
        return;
    }
    s.sleeping = false;
    s.quiet_steps = 0;
    s.center = add(s.center, mul(n, depth + 0.021));
    let point = body
        .iter()
        .min_by(|a, b| dot(**a, n).total_cmp(&dot(**b, n)))
        .copied()
        .unwrap();
    let velocity = dot(s.velocity_at(point), n);
    if velocity < 0. {
        let j = -velocity / s.denominator(b, point, n);
        s.impulse(b, point, mul(n, j));
    }
}
fn collider(b: &Binding, s: &Body) -> rc_package::hull::HullSet {
    let vertices = s.vertices(b);
    let axes = b.axes().map(|n| mv(rotation(s.orientation), n));
    let planes = axes
        .into_iter()
        .flat_map(|n| [n, mul(n, -1.)])
        .map(|n| {
            let w = projection(&vertices, n)[1];
            [n[0], n[1], n[2], w]
        })
        .collect();
    rc_package::hull::HullSet {
        hulls: vec![rc_package::hull::Hull {
            offset: 0,
            planes,
            bounds: bounds(&vertices),
        }],
        missing_solid_leaves: 0,
        empty_root_solid: false,
    }
}
impl Game {
    pub(crate) fn rigid_tick(&mut self) {
        let Some(p) = &self.opening else {
            return;
        };
        if p.assets.rigid.is_empty() {
            return;
        }
        let mut extent = self.profile.physics.extent;
        if self.state.crouched {
            extent[2] = 56.;
        }
        let pawn = (
            self.state.pawn.body.position,
            extent.map(|v| v + self.profile.physics.skin + 0.1),
        );
        let Some(s) = self.state.opening.as_mut().and_then(|s| s.rigid.as_mut()) else {
            return;
        };
        let Some(dynamic) = &self.dynamic else {
            return;
        };
        let disabled = self.state.dynamic.as_ref().unwrap();
        for body in s.iter_mut() {
            if let Some(i) = dynamic
                .objects
                .iter()
                .position(|p| p.original == body.original)
            {
                body.active = disabled[i];
            }
        }
        if s.iter().all(|b| !b.active || b.sleeping) {
            return;
        }
        step_with_obstacles(
            &p.assets.rigid,
            s,
            &p.rigid.terrain,
            Some(&p.rigid),
            Some(pawn),
            STEP,
        );
        self.sync_dynamic();
    }
    pub(crate) fn rigid_pose(&self, original: u32) -> Option<(&Binding, &Body)> {
        let p = self.opening.as_ref()?;
        let i = p.assets.rigid.iter().position(|b| b.original == original)?;
        let s = self.state.opening.as_ref()?.rigid.as_ref()?.get(i)?;
        s.active.then_some((&p.assets.rigid[i], s))
    }
    pub(crate) fn rigid_collider(&self, original: u32) -> Option<rc_package::hull::HullSet> {
        let (b, s) = self.rigid_pose(original)?;
        Some(collider(b, s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn box_at(center: V, tilt: f64) -> Binding {
        let (sin, cos) = tilt.sin_cos();
        let rotate = |v: V| [cos * v[0] + sin * v[2], v[1], -sin * v[0] + cos * v[2]];
        let vertices = [5, 7, 3, 1, 4, 6, 0, 2].map(|mask| {
            add(
                center,
                rotate(std::array::from_fn(
                    |i| if mask & (1 << i) == 0 { -1. } else { 1. } * [10., 80., 100.][i],
                )),
            )
        });
        Binding {
            original: 1,
            center,
            vertices,
            inertia: [[6000., 0., 0.], [0., 6000., 0.], [0., 0., 6000.]],
            mass: 1.,
            friction: 2.,
            restitution: 0.,
            linear_damping: 0.,
            angular_damping: 0.,
            gravity: [0., 0., -950.],
            initial_velocity: [0.; 3],
            initial_angular_velocity: [0.; 3],
        }
    }
    fn floor() -> Vec<Terrain> {
        vec![
            Terrain::new([
                [-1000., -1000., 0.],
                [1000., -1000., 0.],
                [1000., 1000., 0.],
            ])
            .unwrap(),
            Terrain::new([
                [-1000., -1000., 0.],
                [1000., 1000., 0.],
                [-1000., 1000., 0.],
            ])
            .unwrap(),
        ]
    }
    #[test]
    fn terrain_index_keeps_contact_tolerance_and_matches_brute_force() {
        let wall = Terrain::new([[256., -1., -1.], [256., 1., -1.], [256., 0., 1.]]).unwrap();
        let prepared = Prepared::new(vec![wall]);
        assert_eq!(
            prepared.query([[255., -0.5, -0.5], [255.99, 0.5, 0.5]]),
            [0]
        );
        let mut terrain = floor();
        terrain.push(
            Terrain::new([
                [-100000., -100000., -1000.],
                [100000., -100000., -1000.],
                [100000., 100000., -1000.],
            ])
            .unwrap(),
        );
        let prepared = Prepared::new(terrain.clone());
        assert_eq!(prepared.large, [2]);
        let b = box_at([255., -256., 150.], 0.25);
        let mut a = vec![b.initial()];
        a[0].active = true;
        let mut indexed = a.clone();
        for _ in 0..180 {
            step(std::slice::from_ref(&b), &mut a, &terrain, STEP);
            step_with_obstacles(
                std::slice::from_ref(&b),
                &mut indexed,
                &terrain,
                Some(&prepared),
                None,
                STEP,
            );
            assert_eq!(a, indexed);
        }
    }
    #[test]
    fn gravity_tips_the_body_and_contact_stops_it_without_a_rest_pose() {
        let b = box_at([0., 0., 150.], 0.25);
        b.validate().unwrap();
        let mut bodies = vec![b.initial()];
        bodies[0].active = true;
        let initial = bodies[0].clone();
        for _ in 0..1200 {
            step(std::slice::from_ref(&b), &mut bodies, &floor(), STEP);
            assert!(bodies[0].valid(&b));
        }
        let s = &bodies[0];
        let low = s
            .vertices(&b)
            .iter()
            .map(|v| v[2])
            .fold(f64::INFINITY, f64::min);
        assert!(s.center[2] < initial.center[2] - 50., "{s:?}");
        assert!(s.orientation[0].abs() < 0.98, "{s:?}");
        assert!(low > -0.5 && low < 0.5, "low={low}, {s:?}");
        assert!(dot(s.velocity, s.velocity) < 2., "{s:?}");
        assert!(s.sleeping, "Contact jitter must eventually sleep: {s:?}");
    }
    #[test]
    fn sleeping_and_inactive_bodies_do_not_drift_and_resume_is_exact() {
        let b = box_at([0., 0., 150.], 0.25);
        let mut a = vec![b.initial()];
        step(std::slice::from_ref(&b), &mut a, &floor(), STEP);
        assert_eq!(a[0], b.initial());
        a[0].active = true;
        for _ in 0..100 {
            step(std::slice::from_ref(&b), &mut a, &floor(), STEP);
        }
        let bytes = serde_json::to_vec(&a).unwrap();
        let mut restored: Vec<Body> = serde_json::from_slice(&bytes).unwrap();
        for _ in 0..80 {
            step(std::slice::from_ref(&b), &mut a, &floor(), STEP);
            step(std::slice::from_ref(&b), &mut restored, &floor(), STEP);
        }
        assert_eq!(a, restored);
        restored[0].orientation = [0.; 4];
        assert!(!restored[0].valid(&b));
    }
    #[test]
    fn falling_body_respects_the_character_and_uses_the_same_query_shape() {
        let b = box_at([0., 0., 160.], 0.25);
        let mut states = vec![b.initial()];
        states[0].active = true;
        let pawn = [75., 0., 40.];
        let extent = [20., 20., 40.];
        let obstacle = (pawn, extent.map(|v| v + 0.6));
        for _ in 0..600 {
            step_with_obstacles(
                std::slice::from_ref(&b),
                &mut states,
                &floor(),
                None,
                Some(obstacle),
                STEP,
            );
            assert!(states[0].valid(&b));
            assert!(
                collider(&b, &states[0])
                    .sweep_motion(pawn, pawn, extent)
                    .unwrap()
                    .is_none(),
                "{:?}",
                states[0]
            );
        }
    }
    #[test]
    fn bodies_exchange_impulses_and_do_not_pass_through_each_other() {
        let mut a = box_at([-35., 0., 150.], 0.);
        a.gravity = [0.; 3];
        a.initial_velocity = [60., 0., 0.];
        let mut b = box_at([35., 0., 150.], 0.);
        b.gravity = [0.; 3];
        b.original = 2;
        b.initial_velocity = [-60., 0., 0.];
        let bindings = [a, b];
        let mut states: Vec<_> = bindings.iter().map(Binding::initial).collect();
        for s in &mut states {
            s.active = true;
        }
        for _ in 0..100 {
            step(&bindings, &mut states, &[], STEP);
        }
        assert!(states[0].center[0] < states[1].center[0]);
        assert!(
            states[0].velocity[0] < 40. && states[1].velocity[0] > -40.,
            "{states:?}"
        );
        assert!((states[0].velocity[0] + states[1].velocity[0]).abs() < 1e-8);
    }
}
