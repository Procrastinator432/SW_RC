//! Bounded sprite emitter host for the selected original Bacta templates.
//! Native spawn/range/curve rules are hosted at fixed ticks; RNG and skeletal
//! animation selection remain explicit port policies, not global engine parity.
use crate::visual::Material;
use rc_package::geometry::Triangle;
use rc_render::{
    effects::{Blend, TriangleEffect},
    FaceTexture,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Template {
    pub original: u32,
    pub body: bool,
    pub parameters: serde_json::Value,
    pub material: Material,
}
impl Template {
    pub fn number(&self, key: &str, default: f32) -> f32 {
        self.parameters[key].as_f64().map_or(default, |n| n as f32)
    }
    pub fn flag(&self, key: &str, default: bool) -> bool {
        self.parameters[key].as_bool().unwrap_or(default)
    }
    fn range(&self, key: &str) -> [f32; 2] {
        range(&self.parameters[key])
    }
    fn vector(&self, key: &str) -> [f32; 3] {
        vector(&self.parameters[key])
    }
    fn random_vector(&self, key: &str, seed: &mut u32) -> [f32; 3] {
        ["X", "Y", "Z"].map(|axis| sample(range(&self.parameters[key][axis]), seed))
    }
    pub fn validate(&self) -> Result<(), String> {
        let p = &self.parameters;
        if self.original == 0
            || !p.is_object()
            || !(1. ..=128.).contains(&self.number("MaxParticles", 0.))
            || ![1., 3., 5., 6.].contains(&self.number("DrawStyle", -1.))
            || !(0. ..=2.).contains(&self.number("UseDirectionAs", 0.))
            || !(0. ..=1.).contains(&self.number("CoordinateSystem", 0.))
            || ![0., 1.].contains(&self.number("StartLocationShape", 0.))
            || !(0. ..=60.).contains(&self.number("LocationChangeTime", 1.))
            || self.flag("UseSubdivisionScale", false)
            || self.range("LifetimeRange")[0] <= 0.
            || self.range("LifetimeRange")[1] > 15.
            || self.material.width == 0
            || self.material.height == 0
            || self.material.width > 512
            || self.material.height > 512
            || self.material.pixels.len() != self.material.width * self.material.height
        {
            return Err("Invalid selected particle template".into());
        }
        fn check(v: &serde_json::Value) -> bool {
            match v {
                serde_json::Value::Number(n) => {
                    n.as_f64().is_some_and(|f| f.is_finite() && f.abs() <= 1e7)
                }
                serde_json::Value::Array(a) => a.len() <= 256 && a.iter().all(check),
                serde_json::Value::Object(m) => m.len() <= 128 && m.values().all(check),
                _ => true,
            }
        }
        if !check(p) {
            return Err("Invalid particle parameter limits".into());
        }
        if self.flag("BlendBetweenSubdivisions", false)
            && (self.number("DrawStyle", 3.) == 1.
                || ["TextureUSubdivisions", "TextureVSubdivisions"]
                    .iter()
                    .any(|key| !(1. ..=16.).contains(&self.number(key, 1.))))
        {
            return Err("Unsupported particle atlas blend".into());
        }
        Ok(())
    }
}
fn range(v: &serde_json::Value) -> [f32; 2] {
    ["Min", "Max"].map(|s| v[s].as_f64().unwrap_or(0.) as f32)
}
fn vector(v: &serde_json::Value) -> [f32; 3] {
    std::array::from_fn(|i| v[i].as_f64().unwrap_or(0.) as f32)
}
fn random(seed: &mut u32) -> f32 {
    *seed = seed.wrapping_mul(214013).wrapping_add(2531011);
    ((*seed >> 16) & 32767) as f32 / 32767.
}
fn sample(r: [f32; 2], seed: &mut u32) -> f32 {
    r[0] + (r[1] - r[0]) * random(seed)
}
fn transform(v: [f32; 3], m: &[[f32; 4]; 4], point: bool) -> [f32; 3] {
    std::array::from_fn(|i| {
        v[0] * m[0][i] + v[1] * m[1][i] + v[2] * m[2][i] + if point { m[3][i] } else { 0. }
    })
}
fn normalize(v: [f32; 3]) -> [f32; 3] {
    let length = v.iter().map(|v| v * v).sum::<f32>().sqrt();
    if length < 1e-6 {
        [0.; 3]
    } else {
        v.map(|v| v / length)
    }
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Particle {
    pub age: f32,
    pub life: f32,
    pub position: [f32; 3],
    pub old_position: [f32; 3],
    pub velocity: [f32; 3],
    pub size: [f32; 3],
    pub color: [f32; 3],
    pub spin: f32,
    pub spin_rate: f32,
    pub subdivision: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Emitter {
    pub original: u32,
    pub delay: f32,
    pub fraction: f32,
    #[serde(default)]
    pub location_time: f32,
    pub spawned: u32,
    pub particles: Vec<Particle>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Instance {
    pub emitters: Vec<Emitter>,
    pub fading: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    pub seed: u32,
    pub eye: Option<Instance>,
    pub bodies: Vec<Instance>,
    pub active: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            seed: 736,
            eye: None,
            bodies: vec![],
            active: false,
        }
    }
}
impl Instance {
    fn new(templates: &[Template], body: bool, seed: &mut u32) -> Self {
        Self {
            fading: false,
            emitters: templates
                .iter()
                .filter(|t| t.body == body)
                .map(|t| Emitter {
                    original: t.original,
                    delay: sample(t.range("InitialDelayRange"), seed),
                    fraction: 0.,
                    location_time: 0.,
                    spawned: 0,
                    particles: vec![],
                })
                .collect(),
        }
    }
    fn empty(&self) -> bool {
        self.emitters.iter().all(|e| e.particles.is_empty())
    }
}
impl State {
    pub fn burst(templates: &[Template], seed: u32) -> Self {
        let mut state = Self {
            seed,
            active: true,
            ..Self::default()
        };
        state.eye = Some(Instance::new(templates, false, &mut state.seed));
        state
    }
    pub fn finished(&self, templates: &[Template]) -> bool {
        self.eye.as_ref().is_none_or(|i| {
            i.emitters.iter().all(|e| {
                e.particles.is_empty()
                    && templates
                        .iter()
                        .find(|t| t.original == e.original)
                        .is_some_and(|t| e.spawned >= t.number("MaxParticles", 10.) as u32)
            })
        })
    }
    pub fn validate(&self, templates: &[Template]) -> Result<(), String> {
        if self.bodies.len() > 16 || self.active != self.eye.is_some() {
            return Err("Invalid particle instances".into());
        }
        for instance in self.eye.iter().chain(&self.bodies) {
            let body = !self
                .eye
                .as_ref()
                .is_some_and(|eye| std::ptr::eq(eye, instance));
            let selected: Vec<_> = templates.iter().filter(|t| t.body == body).collect();
            if instance.emitters.len() != selected.len() {
                return Err("Particle template count mismatch".into());
            }
            for (e, t) in instance.emitters.iter().zip(selected) {
                if e.original != t.original
                    || e.spawned > t.number("MaxParticles", 0.) as u32
                    || e.particles.len() > t.number("MaxParticles", 0.) as usize
                    || !e.delay.is_finite()
                    || !(0. ..=60.).contains(&e.delay)
                    || !e.location_time.is_finite()
                    || !(0. ..=60.).contains(&e.location_time)
                    || !e.fraction.is_finite()
                    || !(0. ..1.).contains(&e.fraction)
                    || e.particles.iter().any(|p| {
                        p.position
                            .iter()
                            .chain(&p.old_position)
                            .chain(&p.velocity)
                            .chain(&p.size)
                            .chain(&p.color)
                            .chain([&p.age, &p.life, &p.spin, &p.spin_rate])
                            .any(|v| !v.is_finite() || v.abs() > 1e8)
                            || p.age < -15.
                            || p.life <= 0.
                            || p.life > 15.
                            || p.age > p.life + crate::STEP as f32
                            || p.subdivision > 4096
                    })
                {
                    return Err("Invalid particle checkpoint".into());
                }
            }
        }
        Ok(())
    }
    /// Body joints are the host's selected original mesh pose in pawn-local space.
    pub fn tick(
        &mut self,
        templates: &[Template],
        active: bool,
        eye: &[[f32; 4]; 4],
        body: &[[f32; 4]; 4],
        joints: &[[f32; 3]],
    ) {
        if active && !self.active {
            self.eye = Some(Instance::new(templates, false, &mut self.seed));
            self.bodies
                .push(Instance::new(templates, true, &mut self.seed));
        } else if !active && self.active {
            self.eye = None;
            for b in &mut self.bodies {
                b.fading = true;
            }
        }
        self.active = active;
        if let Some(instance) = &mut self.eye {
            tick_instance(instance, templates, &mut self.seed, eye, &[]);
        }
        for instance in &mut self.bodies {
            tick_instance(instance, templates, &mut self.seed, body, joints);
        }
        self.bodies.retain(|b| !b.fading || !b.empty());
    }
    pub fn geometry(
        &self,
        templates: &[Template],
        texture_base: usize,
        eye: &[[f32; 4]; 4],
        body: &[[f32; 4]; 4],
        camera: [[f32; 3]; 4],
    ) -> Vec<TriangleEffect> {
        let mut result = vec![];
        for (instance, anchor) in self
            .eye
            .iter()
            .map(|i| (i, eye))
            .chain(self.bodies.iter().map(|i| (i, body)))
        {
            for emitter in &instance.emitters {
                let Some((index, t)) = templates
                    .iter()
                    .enumerate()
                    .find(|(_, t)| t.original == emitter.original)
                else {
                    continue;
                };
                for p in &emitter.particles {
                    if p.age < 0. {
                        continue;
                    }
                    let relative = t.number("CoordinateSystem", 0.) == 1.;
                    let position = if relative {
                        transform(p.position, anchor, true)
                    } else {
                        p.position
                    };
                    let old = if relative {
                        transform(p.old_position, anchor, true)
                    } else {
                        p.old_position
                    };
                    let mut axis0 = camera[1];
                    let mut axis1 = camera[2];
                    let direction = t.number("UseDirectionAs", 0.) as u32;
                    if direction != 0 {
                        let velocity = normalize(std::array::from_fn(|i| position[i] - old[i]));
                        let eye_ray = std::array::from_fn(|i| position[i] - camera[0][i]);
                        let perpendicular = normalize(cross(velocity, eye_ray));
                        if velocity != [0.; 3] && perpendicular != [0.; 3] {
                            if direction == 1 {
                                axis0 = perpendicular;
                                axis1 = velocity;
                            } else {
                                axis0 = velocity;
                                axis1 = perpendicular;
                            }
                        }
                    }
                    let angle = (p.spin + p.spin_rate * p.age) * std::f32::consts::TAU;
                    if t.flag("SpinParticles", false) {
                        let (s, c) = angle.sin_cos();
                        let old = axis0;
                        axis0 = std::array::from_fn(|i| old[i] * c + axis1[i] * s);
                        axis1 = std::array::from_fn(|i| axis1[i] * c - old[i] * s);
                    }
                    let scale = size_scale(t, p.age, p.life);
                    let size = [
                        p.size[0] * scale,
                        if direction == 0 {
                            p.size[0] * scale
                        } else {
                            p.size[1] * scale
                        },
                    ];
                    let points: [[f32; 3]; 4] = std::array::from_fn(|n| {
                        std::array::from_fn(|i| {
                            position[i]
                                + axis0[i] * size[0] * if n & 1 == 0 { -1. } else { 1. }
                                + axis1[i] * size[1] * if n & 2 == 0 { -1. } else { 1. }
                        })
                    });
                    let grid = [
                        t.number("TextureUSubdivisions", 1.).max(1.) as u32,
                        t.number("TextureVSubdivisions", 1.).max(1.) as u32,
                    ];
                    let count = grid[0] * grid[1];
                    let start = t.number("SubdivisionStart", 0.);
                    let end = t.number("SubdivisionEnd", 0.);
                    let frame = if end == 0. {
                        (p.age / p.life).clamp(0., 1.) * count as f32
                    } else {
                        start + (p.age / p.life).clamp(0., 1.) * (end - start).max(1.)
                    };
                    let cell = if t.flag("UseRandomSubdivision", false) {
                        p.subdivision % count
                    } else {
                        (frame as u32).min(count - 1)
                    };
                    let uv: [[f32; 2]; 4] = std::array::from_fn(|n| {
                        [
                            (cell / grid[1]) as f32 / grid[0] as f32
                                + if n & 1 == 0 { 0. } else { 1. / grid[0] as f32 },
                            (cell % grid[1]) as f32 / grid[1] as f32
                                + if n & 2 == 0 { 1. / grid[1] as f32 } else { 0. },
                        ]
                    });
                    let color = particle_color(t, p);
                    for ids in [[0, 1, 3], [0, 3, 2]] {
                        result.push(TriangleEffect {
                            triangle: Triangle {
                                points: ids.map(|i| points[i]),
                                surface: 0,
                            },
                            face: FaceTexture {
                                texture: texture_base + index,
                                uv: ids.map(|i| uv[i]),
                            },
                            color,
                            blend: match t.number("DrawStyle", 3.) as u32 {
                                1 => Blend::Alpha,
                                6 => Blend::Screen,
                                5 => Blend::Darken,
                                _ => Blend::Additive,
                            },
                            depth_test: t.flag("ZTest", true),
                            alpha_reference: t
                                .flag("AlphaTest", true)
                                .then_some(t.number("AlphaRef", 0.) as u8),
                            atlas_blend: t.flag("BlendBetweenSubdivisions", false).then_some(
                                rc_render::effects::AtlasBlend {
                                    grid: grid.map(|v| v as u8),
                                    fraction: frame.fract(),
                                },
                            ),
                        });
                    }
                }
            }
        }
        result
    }
}
fn spawn(
    t: &Template,
    seed: &mut u32,
    age: f32,
    location_time: f32,
    anchor: &[[f32; 4]; 4],
    joints: &[[f32; 3]],
) -> Particle {
    let mut position = if t.number("StartLocationShape", 0.) == 1. {
        let duration = t.number("LocationChangeTime", 1.);
        let fraction = if duration > 0. {
            (location_time / duration).clamp(0., 1.)
        } else {
            1.
        };
        ["X", "Y", "Z"].map(|axis| {
            let start = range(&t.parameters["StartLocationRange"][axis]);
            let end = range(&t.parameters["EndLocationRange"][axis]);
            sample(
                std::array::from_fn(|i| start[i] + (end[i] - start[i]) * fraction),
                seed,
            )
        })
    } else {
        t.random_vector("StartLocationRange", seed)
    };
    let offset = t.vector("StartLocationOffset");
    if t.number("UseSkeletalLocationAs", 0.) != 0. && !joints.is_empty() {
        let n = (sample(t.range("RelativeBoneIndexRange"), seed) * joints.len() as f32) as usize;
        let joint = joints[n.min(joints.len() - 1)];
        for i in 0..3 {
            position[i] += joint[i];
        }
    }
    let mut velocity = t.random_vector("StartVelocityRange", seed);
    if t.number("CoordinateSystem", 0.) == 0. {
        position = transform(position, anchor, true);
        velocity = transform(velocity, anchor, false);
    }
    for i in 0..3 {
        position[i] += offset[i];
    }
    let birth_time = age;
    let life = sample(t.range("LifetimeRange"), seed);
    let age = age + sample(t.range("InitialTimeRange"), seed);
    let size = t.random_vector("StartSizeRange", seed);
    let size = if t.flag("UniformSize", false) {
        [size[0]; 3]
    } else {
        size
    };
    let color = t.random_vector("ColorMultiplierRange", seed);
    let spin = t.random_vector("StartSpinRange", seed)[0];
    let rate = t.random_vector("SpinsPerSecondRange", seed)[0];
    let chance = t.vector("SpinCCWorCW")[0];
    let spin_rate = rate * if random(seed) < chance { -1. } else { 1. };
    let old_position = position;
    let acceleration = t.vector("Acceleration");
    for i in 0..3 {
        velocity[i] += acceleration[i] * birth_time;
        position[i] += velocity[i] * birth_time;
    }
    Particle {
        age,
        life,
        position,
        old_position,
        velocity,
        size,
        color,
        spin,
        spin_rate,
        subdivision: {
            let count = (t.number("TextureUSubdivisions", 1.)
                * t.number("TextureVSubdivisions", 1.))
            .max(1.);
            let start = t.number("SubdivisionStart", 0.);
            let end = t.number("SubdivisionEnd", 0.);
            (if end == 0. {
                random(seed) * count
            } else {
                start + random(seed) * (end - start).max(1.)
            }) as u32
        },
    }
}
fn tick_instance(
    instance: &mut Instance,
    templates: &[Template],
    seed: &mut u32,
    anchor: &[[f32; 4]; 4],
    joints: &[[f32; 3]],
) {
    let dt = crate::STEP as f32;
    for e in &mut instance.emitters {
        let t = templates.iter().find(|t| t.original == e.original).unwrap();
        if e.delay > 0. {
            e.delay = (e.delay - dt).max(0.);
            if e.delay > 0. {
                continue;
            }
        }
        if t.number("StartLocationShape", 0.) == 1. {
            e.location_time =
                (e.location_time + dt).min(t.number("LocationChangeTime", 1.).max(0.));
        }
        let max = t.number("MaxParticles", 10.) as u32;
        let existing = e.particles.len();
        if !instance.fading && e.spawned < max {
            let rate = if t.flag("AutomaticInitialSpawning", true) {
                max as f32 / ((t.range("LifetimeRange")[0] + t.range("LifetimeRange")[1]) * 0.5)
            } else {
                t.number("InitialParticlesPerSecond", 0.)
            };
            let total = e.fraction + rate * dt;
            let count = (total as u32).min(max - e.spawned);
            let old = e.fraction;
            e.fraction = total.fract();
            for n in 0..count {
                e.particles.push(spawn(
                    t,
                    seed,
                    if rate > 0. {
                        (old / rate + dt - 1. / rate) - n as f32 / rate
                    } else {
                        0.
                    },
                    e.location_time,
                    anchor,
                    joints,
                ));
                e.spawned += 1;
            }
        }
        for (index, p) in e.particles.iter_mut().enumerate() {
            let mut newborn = index >= existing;
            if !newborn {
                p.age += dt;
                if p.age > p.life {
                    if instance.fading || !t.flag("RespawnDeadParticles", true) {
                        continue;
                    }
                    *p = spawn(
                        t,
                        seed,
                        (p.age - p.life).rem_euclid(p.life),
                        e.location_time,
                        anchor,
                        joints,
                    );
                    newborn = true;
                }
                if !newborn {
                    p.old_position = p.position;
                    let acceleration = t.vector("Acceleration");
                    for (i, a) in acceleration.into_iter().enumerate() {
                        p.velocity[i] += a * dt;
                        p.position[i] += p.velocity[i] * dt;
                    }
                }
            }
            // Native UpdateParticles clamps and applies freshly sampled loss after movement.
            let limit = t.vector("MaxAbsVelocity");
            let loss = t.random_vector("VelocityLossRange", seed);
            for i in 0..3 {
                if limit[i] > 0. {
                    p.velocity[i] = p.velocity[i].clamp(-limit[i], limit[i]);
                }
                p.velocity[i] -= loss[i] * p.velocity[i] * dt;
            }
        }
        e.particles.retain(|p| p.age <= p.life);
    }
}
fn size_scale(t: &Template, age: f32, life: f32) -> f32 {
    if !t.flag("UseSizeScale", false) {
        return 1.;
    }
    if t.flag("UseRegularSizeScale", true) {
        return 1. / (age + 1.);
    }
    curve(
        t,
        "SizeScale",
        "RelativeSize",
        ((t.number("SizeScaleRepeats", 0.) + 1.) * (age / life).clamp(0., 1.)).fract(),
        1.,
    )
}
fn curve(t: &Template, key: &str, value: &str, time: f32, default: f32) -> f32 {
    let Some(curve) = t.parameters[key].as_array() else {
        return default;
    };
    let mut previous = (0., default);
    for point in curve {
        let end = point["RelativeTime"].as_f64().unwrap_or(0.) as f32;
        let val = point[value].as_f64().unwrap_or(f64::from(default)) as f32;
        if time <= end {
            let f = if end == 0. {
                1.
            } else {
                (time - previous.0) / (end - previous.0)
            };
            return previous.1 + (val - previous.1) * f;
        }
        previous = (end, val);
    }
    default
}
fn particle_color(t: &Template, p: &Particle) -> [f32; 4] {
    let mut color = [1.; 4];
    if t.flag("UseColorScale", false) {
        let time =
            ((t.number("ColorScaleRepeats", 0.) + 1.) * (p.age / p.life).clamp(0., 1.)).fract();
        if let Some(points) = t.parameters["ColorScale"].as_array() {
            let mut previous = (0., [1.; 4]);
            for point in points {
                let end = point["RelativeTime"].as_f64().unwrap_or(0.) as f32;
                let val = std::array::from_fn(|i| point["Color"][i].as_f64().unwrap_or(1.) as f32);
                if time <= end {
                    let f = if end == 0. {
                        1.
                    } else {
                        (time - previous.0) / (end - previous.0)
                    };
                    color = std::array::from_fn(|i| previous.1[i] + (val[i] - previous.1[i]) * f);
                    break;
                }
                previous = (end, val);
            }
        }
    }
    for (c, m) in color[..3].iter_mut().zip(p.color) {
        *c *= m;
    }
    if t.number("DrawStyle", 3.) != 1. {
        color[3] = 1.;
    }
    let fade = if t.flag("FadeOut", false)
        && p.age > t.number("FadeOutStartTime", 0.)
        && p.life != t.number("FadeOutStartTime", 0.)
    {
        Some((
            "FadeOutFactor",
            (p.age - t.number("FadeOutStartTime", 0.))
                / (p.life - t.number("FadeOutStartTime", 0.)),
        ))
    } else if t.flag("FadeIn", false)
        && p.age < t.number("FadeInEndTime", 0.)
        && t.number("FadeInEndTime", 0.) != 0.
    {
        Some((
            "FadeInFactor",
            (t.number("FadeInEndTime", 0.) - p.age) / t.number("FadeInEndTime", 0.),
        ))
    } else {
        None
    };
    if let Some((key, f)) = fade {
        for (i, c) in color.iter_mut().enumerate() {
            if (t.number("DrawStyle", 3.) == 1. && i == 3)
                || (t.number("DrawStyle", 3.) != 1. && i < 3)
            {
                *c -= t.parameters[key][["X", "Y", "Z", "W"][i]]
                    .as_f64()
                    .unwrap_or(1.) as f32
                    * f;
            }
        }
    }
    let opacity = t.number("Opacity", 1.);
    for (i, c) in color.iter_mut().enumerate() {
        if (t.number("DrawStyle", 3.) == 1. && i == 3) || (t.number("DrawStyle", 3.) != 1. && i < 3)
        {
            *c *= opacity;
        }
        *c = c.clamp(0., 1.);
    }
    color
}

#[cfg(test)]
mod tests {
    use super::*;
    fn template(original: u32, body: bool) -> Template {
        Template {
            original,
            body,
            parameters: serde_json::json!({"MaxParticles":2,"DrawStyle":6,"InitialParticlesPerSecond":1000,"AutomaticInitialSpawning":false,"RespawnDeadParticles":!body,"LifetimeRange":{"Min":0.2,"Max":0.2},"StartSizeRange":{"X":{"Min":1,"Max":1},"Y":{"Min":1,"Max":1}},"ColorMultiplierRange":{"X":{"Min":1,"Max":1},"Y":{"Min":1,"Max":1},"Z":{"Min":1,"Max":1}}}),
            material: Material {
                source: "test".into(),
                chain: vec![],
                opacity: serde_json::Value::Null,
                width: 1,
                height: 1,
                pixels: vec![0xffffffff],
                uv_scale: 1.,
                uv_transform: None,
            },
        }
    }
    fn identity() -> [[f32; 4]; 4] {
        std::array::from_fn(|r| std::array::from_fn(|c| if r == c { 1. } else { 0. }))
    }
    #[test]
    fn moving_box_and_post_movement_velocity_loss_use_original_rules() {
        let mut t = template(1, false);
        t.parameters["StartLocationShape"] = 1.into();
        t.parameters["LocationChangeTime"] = 1.into();
        t.parameters["StartLocationRange"] = serde_json::json!({"X":{"Min":10,"Max":10}});
        t.parameters["EndLocationRange"] = serde_json::json!({"X":{"Min":30,"Max":30}});
        let p = spawn(&t, &mut 1, 0., 0.5, &identity(), &[]);
        assert_eq!(p.position, [20., 0., 0.]);
        t.parameters["StartLocationOffset"] = serde_json::json!([2, 0, 0]);
        t.parameters["InitialTimeRange"] = serde_json::json!({"Min":1,"Max":1});
        t.parameters["StartVelocityRange"] = serde_json::json!({"Y":{"Min":10,"Max":10}});
        let rotated =
            rc_package::mesh_transform::rotation_matrix([0, 16384, 0], [0f32.to_bits(); 3])
                .unwrap()
                .map(|r| r.map(f32::from_bits));
        let particle = spawn(&t, &mut 1, 0., 0.5, &rotated, &[]);
        assert!(
            (particle.position[0] - 2.).abs() < 0.001 && (particle.position[1] - 20.).abs() < 0.001
        );
        assert_eq!(particle.age, 1.);
        t.parameters["StartLocationOffset"] = serde_json::json!([0, 0, 0]);
        t.parameters["InitialTimeRange"] = serde_json::json!({"Min":0,"Max":0});
        t.parameters["StartLocationShape"] = 0.into();
        t.parameters["MaxParticles"] = 1.into();
        t.parameters["StartVelocityRange"] = serde_json::json!({"X":{"Min":120,"Max":120}});
        t.parameters["Acceleration"] = serde_json::json!([60, 0, 0]);
        t.parameters["MaxAbsVelocity"] = serde_json::json!([30, 0, 0]);
        t.parameters["VelocityLossRange"] = serde_json::json!({"X":{"Min":6,"Max":6}});
        let mut state = State::burst(std::slice::from_ref(&t), 1);
        for _ in 0..2 {
            state.tick(
                std::slice::from_ref(&t),
                true,
                &identity(),
                &identity(),
                &[],
            );
        }
        let p = &state.eye.as_ref().unwrap().emitters[0].particles[0];
        assert!((p.velocity[0] - 25.2).abs() < 0.001);
        assert!(p.position[0] > 2.);
    }
    #[test]
    fn original_endstate_destroys_eye_and_preserves_only_living_body_particles() {
        let t = [template(1, false), template(2, true)];
        let mut s = State::default();
        let m = identity();
        s.tick(&t, true, &m, &m, &[]);
        assert_eq!(s.eye.as_ref().unwrap().emitters[0].particles.len(), 2);
        assert_eq!(s.bodies[0].emitters[0].particles.len(), 2);
        s.tick(&t, false, &m, &m, &[]);
        assert!(s.eye.is_none() && !s.bodies.is_empty() && s.bodies[0].fading);
        for _ in 0..20 {
            s.tick(&t, false, &m, &m, &[]);
        }
        assert!(s.bodies.is_empty());
    }
    #[test]
    fn restart_keeps_old_body_tail_and_checkpoint_rng_continues_exactly() {
        let t = [template(1, false), template(2, true)];
        let mut s = State::default();
        let m = identity();
        for active in [true, false, true] {
            s.tick(&t, active, &m, &m, &[]);
        }
        assert_eq!(s.bodies.len(), 2);
        s.validate(&t).unwrap();
        let mut restored: State = serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
        for _ in 0..100 {
            s.tick(&t, true, &m, &m, &[]);
            restored.tick(&t, true, &m, &m, &[]);
        }
        assert_eq!(
            serde_json::to_value(s).unwrap(),
            serde_json::to_value(restored).unwrap()
        );
    }
    #[test]
    fn delay_and_fractional_spawn_are_not_a_one_frame_burst() {
        let mut t = template(1, false);
        t.parameters["InitialParticlesPerSecond"] = serde_json::json!(3.);
        t.parameters["InitialDelayRange"] = serde_json::json!({"Min":0.5,"Max":0.5});
        let t = [t];
        let mut s = State::default();
        let m = identity();
        for _ in 0..40 {
            s.tick(&t, true, &m, &m, &[]);
        }
        assert!(s.eye.as_ref().unwrap().emitters[0].particles.is_empty());
        for _ in 0..20 {
            s.tick(&t, true, &m, &m, &[]);
        }
        assert_eq!(s.eye.as_ref().unwrap().emitters[0].spawned, 1);
    }
    #[test]
    fn invalid_checkpoint_capacity_and_template_identity_are_rejected() {
        let t = [template(1, false)];
        let m = identity();
        let mut s = State::default();
        s.tick(&t, true, &m, &m, &[]);
        s.eye.as_mut().unwrap().emitters[0].original = 9;
        assert!(s.validate(&t).is_err());
        s.eye.as_mut().unwrap().emitters[0].original = 1;
        s.eye.as_mut().unwrap().emitters[0].particles[0].life = f32::NAN;
        assert!(s.validate(&t).is_err());
    }
    #[test]
    fn size_color_curves_and_original_rgb_opacity_are_explicit() {
        let mut t = template(1, false);
        t.parameters["UseSizeScale"] = serde_json::json!(true);
        t.parameters["UseRegularSizeScale"] = serde_json::json!(false);
        t.parameters["SizeScale"] = serde_json::json!([{"RelativeTime":0,"RelativeSize":0.5},{"RelativeTime":1,"RelativeSize":1}]);
        t.parameters["Opacity"] = serde_json::json!(0.25);
        assert!((size_scale(&t, 0.1, 0.2) - 0.75).abs() < 1e-6);
        let p = spawn(&t, &mut 1, 0., 0., &identity(), &[]);
        assert_eq!(particle_color(&t, &p), [0.25, 0.25, 0.25, 1.]);
    }
}
