//! Portable depth-tested geometry renderer with basic diffuse textures. No game simulation.
pub mod fragment;
pub mod hologram_pass;
pub mod query;
pub mod shader_raster;
pub mod skeletal;
use rc_package::{
    geometry::{read_bsp, Triangle},
    read_package,
};
pub struct Scene {
    pub triangles: Vec<Triangle>,
    pub center: [f32; 3],
    pub radius: f32,
    pub textures: Vec<Texture>,
    pub appearance: Vec<Option<FaceTexture>>,
    pub starts: Vec<rc_package::level::PlayerStart>,
    pub solid: Option<rc_package::collision::SolidBsp>,
}
#[derive(Clone)]
pub struct Texture {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>,
}
#[derive(Clone, Copy)]
pub struct FaceTexture {
    pub uv: [[f32; 2]; 3],
    pub texture: usize,
}
/// Scene-relative diagnostic camera. Free positions are measured in scene radii.
#[derive(Clone, Copy)]
pub struct View {
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    pub position: Option<[f32; 3]>,
}
impl Scene {
    pub fn from_map(data: &[u8]) -> Result<Self, String> {
        if data.starts_with(b"RCSC") {
            return Self::from_snapshot(data);
        }
        let pkg = read_package(data)?;
        let binding = rc_package::level::world_binding(&pkg, data)?;
        let e = &pkg.exports[binding.model_export];
        let bsp = read_bsp(&pkg, data, e)?;
        let triangles = bsp
            .triangles()?
            .into_iter()
            .filter(|t| bsp.surfaces[t.surface].flags & 0x81 == 0)
            .collect();
        let mut scene = Self::new(triangles)?;
        scene.starts = rc_package::level::player_starts(&pkg, data)?;
        scene.solid = Some(rc_package::collision::read_model_collision(
            &pkg, data, e, &bsp,
        )?);
        Ok(scene)
    }
    /// Versioned diagnostic snapshot of original world-space triangles, not a game save.
    pub fn from_snapshot(data: &[u8]) -> Result<Self, String> {
        let header = data.get(..12).ok_or("Truncated scene header")?;
        let version = u32::from_le_bytes(header[4..8].try_into().unwrap());
        if &header[..4] != b"RCSC" || ![1, 2, 3].contains(&version) {
            return Err("Unsupported scene format".into());
        }
        let count = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
        if count > 1_000_000 {
            return Err("Scene triangle count exceeds limit".into());
        }
        let start = match version {
            3 => 20,
            2 => 16,
            _ => 12,
        };
        let stride = if version >= 2 { 68 } else { 40 };
        let end = start + count * stride;
        if data.len() < end || (version == 1 && data.len() != end) {
            return Err("Scene triangle count does not match payload".into());
        }
        let triangles = data[start..end]
            .chunks_exact(stride)
            .map(|chunk| {
                let points = std::array::from_fn(|i| {
                    std::array::from_fn(|j| {
                        let offset = (i * 3 + j) * 4;
                        f32::from_le_bytes(chunk[offset..offset + 4].try_into().unwrap())
                    })
                });
                Triangle {
                    points,
                    surface: u32::from_le_bytes(chunk[36..40].try_into().unwrap()) as usize,
                }
            })
            .collect();
        let mut scene = Self::new(triangles)?;
        if version >= 2 {
            let texture_count = u32::from_le_bytes(
                data.get(12..16)
                    .ok_or("Truncated texture count")?
                    .try_into()
                    .unwrap(),
            ) as usize;
            if texture_count > 1024 {
                return Err("Too many scene textures".into());
            }
            for chunk in data[start..end].chunks_exact(stride) {
                let index = i32::from_le_bytes(chunk[64..68].try_into().unwrap());
                if index < -1 || index >= texture_count as i32 {
                    return Err("Invalid scene texture reference".into());
                }
                let uv: [[f32; 2]; 3] = std::array::from_fn(|i| {
                    std::array::from_fn(|j| {
                        let offset = 40 + (i * 2 + j) * 4;
                        f32::from_le_bytes(chunk[offset..offset + 4].try_into().unwrap())
                    })
                });
                if uv.iter().flatten().any(|v| !v.is_finite()) {
                    return Err("Nonfinite scene UV".into());
                }
                scene.appearance.push((index >= 0).then_some(FaceTexture {
                    uv,
                    texture: index as usize,
                }));
            }
            let mut pos = end;
            let mut total = 0;
            for _ in 0..texture_count {
                let header = data.get(pos..pos + 8).ok_or("Truncated scene texture")?;
                pos += 8;
                let width = u32::from_le_bytes(header[..4].try_into().unwrap()) as usize;
                let height = u32::from_le_bytes(header[4..].try_into().unwrap()) as usize;
                if width == 0 || height == 0 || width > 4096 || height > 4096 {
                    return Err("Invalid scene texture dimensions".into());
                }
                total += width * height;
                if total > 16_777_216 {
                    return Err("Scene texture budget exceeded".into());
                }
                let bytes = data
                    .get(pos..pos + width * height * 4)
                    .ok_or("Truncated scene pixels")?;
                pos += bytes.len();
                scene.textures.push(Texture {
                    width,
                    height,
                    pixels: bytes
                        .chunks_exact(4)
                        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                        .collect(),
                });
            }
            if version == 3 {
                let count = u32::from_le_bytes(data[16..20].try_into().unwrap()) as usize;
                if count > 4096 {
                    return Err("Too many PlayerStart anchors".into());
                }
                for _ in 0..count {
                    let len = u32::from_le_bytes(
                        data.get(pos..pos + 4)
                            .ok_or("Truncated anchor name")?
                            .try_into()
                            .unwrap(),
                    ) as usize;
                    pos += 4;
                    if len == 0 || len > 1024 {
                        return Err("Invalid anchor name length".into());
                    }
                    let actor = std::str::from_utf8(
                        data.get(pos..pos + len).ok_or("Truncated anchor name")?,
                    )
                    .map_err(|_| "Invalid anchor UTF-8")?
                    .to_owned();
                    pos += len;
                    let bytes = data
                        .get(pos..pos + 24)
                        .ok_or("Truncated anchor transform")?;
                    pos += 24;
                    let location: [f32; 3] = std::array::from_fn(|i| {
                        f32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap())
                    });
                    if location.iter().any(|v| !v.is_finite() || v.abs() > 1e9) {
                        return Err("Invalid anchor position".into());
                    }
                    let rotation = std::array::from_fn(|i| {
                        i32::from_le_bytes(bytes[12 + i * 4..16 + i * 4].try_into().unwrap())
                    });
                    scene.starts.push(rc_package::level::PlayerStart {
                        actor,
                        location,
                        rotation,
                    });
                }
            }
            if pos != data.len() {
                return Err("Trailing scene bytes".into());
            }
        }
        Ok(scene)
    }
    pub fn snapshot(&self) -> Result<Vec<u8>, String> {
        if self.triangles.len() > 1_000_000 {
            return Err("Scene exceeds snapshot triangle limit".into());
        }
        let mut result = b"RCSC".to_vec();
        let anchored = !self.starts.is_empty();
        let textured = !self.textures.is_empty() || anchored;
        if !self.textures.is_empty() && self.appearance.len() != self.triangles.len() {
            return Err("Appearance count does not match triangles".into());
        }
        result.extend_from_slice(
            &(if anchored {
                3u32
            } else if textured {
                2u32
            } else {
                1u32
            })
            .to_le_bytes(),
        );
        result.extend_from_slice(&(self.triangles.len() as u32).to_le_bytes());
        if textured {
            result.extend_from_slice(&(self.textures.len() as u32).to_le_bytes());
        }
        if anchored {
            if self.starts.len() > 4096 {
                return Err("Too many PlayerStart anchors".into());
            }
            result.extend_from_slice(&(self.starts.len() as u32).to_le_bytes());
        }
        for (i, t) in self.triangles.iter().enumerate() {
            for p in t.points {
                for f in p {
                    result.extend_from_slice(&f.to_le_bytes());
                }
            }
            let surface = u32::try_from(t.surface).map_err(|_| "Surface identifier too large")?;
            result.extend_from_slice(&surface.to_le_bytes());
            if textured {
                let face = self.appearance.get(i).copied().flatten();
                let uv = face.map_or([[0.0; 2]; 3], |f| f.uv);
                for vertex in uv {
                    for f in vertex {
                        result.extend_from_slice(&f.to_le_bytes());
                    }
                }
                result.extend_from_slice(&face.map_or(-1, |f| f.texture as i32).to_le_bytes());
            }
        }
        if textured {
            for t in &self.textures {
                result.extend_from_slice(&(t.width as u32).to_le_bytes());
                result.extend_from_slice(&(t.height as u32).to_le_bytes());
                for pixel in &t.pixels {
                    result.extend_from_slice(&pixel.to_le_bytes());
                }
            }
        }
        if anchored {
            for start in &self.starts {
                if start.actor.is_empty() || start.actor.len() > 1024 {
                    return Err("Invalid anchor name length".into());
                }
                result.extend_from_slice(&(start.actor.len() as u32).to_le_bytes());
                result.extend_from_slice(start.actor.as_bytes());
                for v in start.location {
                    result.extend_from_slice(&v.to_le_bytes());
                }
                for v in start.rotation {
                    result.extend_from_slice(&v.to_le_bytes());
                }
            }
        }
        Self::from_snapshot(&result)?; // validate references/dimensions before emitting
        Ok(result)
    }
    pub fn new(triangles: Vec<Triangle>) -> Result<Self, String> {
        if triangles.is_empty() {
            return Err("World BSP contains no triangles".into());
        }
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for t in &triangles {
            for p in t.points {
                for i in 0..3 {
                    if !p[i].is_finite() || p[i].abs() > 1e9 {
                        return Err("Unsupported geometry extent".into());
                    }
                    min[i] = min[i].min(p[i]);
                    max[i] = max[i].max(p[i]);
                }
            }
        }
        let center = std::array::from_fn(|i| (min[i] + max[i]) * 0.5);
        let radius = length(sub(max, min)) * 0.5;
        if radius < 0.001 {
            return Err("Degenerate world bounds".into());
        }
        Ok(Self {
            triangles,
            center,
            radius,
            textures: vec![],
            appearance: vec![],
            starts: vec![],
            solid: None,
        })
    }
    /// Exact actor anchor, without pawn eye height or game spawn-selection logic.
    pub fn start_view(&self, index: usize) -> Result<View, String> {
        let start = self
            .starts
            .get(index)
            .ok_or("PlayerStart index outside scene")?;
        if start.rotation[2] & 65532 != 0 {
            return Err("PlayerStart roll is not supported by the diagnostic camera".into());
        }
        // The engine's trig table discards the bottom two bits of each rotator component.
        let radians = |angle: i32| ((angle & 65532) as f32) * std::f32::consts::TAU / 65536.0;
        let pitch = -radians(start.rotation[0])
            .sin()
            .atan2(radians(start.rotation[0]).cos());
        if pitch.abs() > 1.5 {
            return Err("PlayerStart pitch exceeds diagnostic camera range".into());
        }
        Ok(View {
            yaw: radians(start.rotation[1]) + std::f32::consts::PI,
            pitch,
            zoom: 1.0,
            position: Some(mul(sub(start.location, self.center), 1.0 / self.radius)),
        })
    }
    /// ARGB8888 pixels, top row first. Yaw/pitch radians, zoom 0.5 to 32.
    pub fn render(
        &self,
        width: usize,
        height: usize,
        yaw: f32,
        pitch: f32,
        zoom: f32,
    ) -> Result<Vec<u32>, String> {
        self.render_view(
            width,
            height,
            View {
                yaw,
                pitch,
                zoom,
                position: None,
            },
        )
    }
    pub fn render_view(&self, width: usize, height: usize, view: View) -> Result<Vec<u32>, String> {
        let View {
            yaw,
            pitch,
            zoom,
            position,
        } = view;
        if width == 0
            || height == 0
            || width > 2048
            || height > 2048
            || !yaw.is_finite()
            || !pitch.is_finite()
            || !zoom.is_finite()
            || position.is_some_and(|p| p.iter().any(|v| !v.is_finite() || v.abs() > 100.0))
        {
            return Err("Invalid render parameters".into());
        }
        let pitch = pitch.clamp(-1.5, 1.5);
        let zoom = zoom.clamp(0.5, 32.0);
        let (sy, cy) = yaw.sin_cos();
        let (sp, cp) = pitch.sin_cos();
        let direction = [cy * cp, sy * cp, sp];
        let right = [-sy, cy, 0.0];
        let up = [-cy * sp, -sy * sp, cp];
        let eye = add(
            self.center,
            position.map_or_else(
                || mul(direction, self.radius * 2.8 / zoom),
                |p| mul(p, self.radius),
            ),
        );
        let forward = mul(direction, -1.0);
        let focal = height as f32 * 1.1;
        let near = self.radius * 0.002;
        let mut pixels = vec![0xff101923; width * height];
        let mut depth = vec![f32::INFINITY; width * height];
        for (triangle_index, t) in self.triangles.iter().enumerate() {
            let appearance = self.appearance.get(triangle_index).copied().flatten();
            let vertices: [[f32; 5]; 3] = std::array::from_fn(|i| {
                let d = sub(t.points[i], eye);
                let uv = appearance.map_or([0.0; 2], |f| f.uv[i]);
                [dot(d, right), dot(d, up), dot(d, forward), uv[0], uv[1]]
            });
            let polygon = clip_near(&vertices, near);
            for corner in 1..polygon.len().saturating_sub(1) {
                let clipped = [polygon[0], polygon[corner], polygon[corner + 1]];
                let projected = clipped.map(|p| {
                    [
                        width as f32 * 0.5 + p[0] * focal / p[2],
                        height as f32 * 0.5 - p[1] * focal / p[2],
                        p[2],
                    ]
                });
                let [a, b, c] = projected;
                let area = edge(a, b, c[0], c[1]);
                if area.abs() < 0.001 {
                    continue;
                }
                let xmin = a[0].min(b[0]).min(c[0]).floor().max(0.0) as usize;
                let xmax = a[0]
                    .max(b[0])
                    .max(c[0])
                    .ceil()
                    .clamp(0.0, width as f32 - 1.0) as usize;
                let ymin = a[1].min(b[1]).min(c[1]).floor().max(0.0) as usize;
                let ymax = a[1]
                    .max(b[1])
                    .max(c[1])
                    .ceil()
                    .clamp(0.0, height as f32 - 1.0) as usize;
                let normal = cross(sub(t.points[1], t.points[0]), sub(t.points[2], t.points[0]));
                let lighting = (dot(normal, [0.3, 0.4, 0.85]).abs() / length(normal).max(0.001))
                    .clamp(0.0, 1.0)
                    * 0.6
                    + 0.4;
                let seed = (t.surface % 100) as u32;
                let r = ((90 + seed * 37 % 100) as f32 * lighting) as u32;
                let g = ((130 + seed * 19 % 100) as f32 * lighting) as u32;
                let blue = ((140 + seed * 11 % 100) as f32 * lighting) as u32;
                let color = 0xff000000 | r << 16 | g << 8 | blue;
                for y in ymin..=ymax {
                    for x in xmin..=xmax {
                        let xf = x as f32 + 0.5;
                        let yf = y as f32 + 0.5;
                        let w0 = edge(b, c, xf, yf) / area;
                        let w1 = edge(c, a, xf, yf) / area;
                        let w2 = 1.0 - w0 - w1;
                        if w0 < -0.00001 || w1 < -0.00001 || w2 < -0.00001 {
                            continue;
                        }
                        let z = 1.0 / (w0 / a[2] + w1 / b[2] + w2 / c[2]);
                        let i = y * width + x;
                        if z < depth[i] {
                            let color = if let Some(face) = appearance {
                                let texture = self
                                    .textures
                                    .get(face.texture)
                                    .ok_or("Invalid texture reference")?;
                                if texture.width == 0
                                    || texture.height == 0
                                    || texture.pixels.len() != texture.width * texture.height
                                {
                                    return Err("Invalid texture pixels".into());
                                }
                                let uv: [f32; 2] = std::array::from_fn(|j| {
                                    (w0 * clipped[0][3 + j] / a[2]
                                        + w1 * clipped[1][3 + j] / b[2]
                                        + w2 * clipped[2][3 + j] / c[2])
                                        * z
                                });
                                let tx = ((uv[0].rem_euclid(1.0) * texture.width as f32) as usize)
                                    .min(texture.width - 1);
                                let ty = ((uv[1].rem_euclid(1.0) * texture.height as f32) as usize)
                                    .min(texture.height - 1);
                                let pixel = texture.pixels[ty * texture.width + tx];
                                if pixel >> 24 < 128 {
                                    continue;
                                }
                                let channel = |shift: u32| {
                                    ((((pixel >> shift) & 255u32) as f32 * lighting) as u32)
                                        << shift
                                };
                                0xff000000u32 | channel(16) | channel(8) | channel(0)
                            } else {
                                color
                            };
                            depth[i] = z;
                            pixels[i] = color;
                        }
                    }
                }
            }
        }
        Ok(pixels)
    }
}
// Interpolate camera-space positions and UV before perspective projection.
fn clip_near(vertices: &[[f32; 5]], near: f32) -> Vec<[f32; 5]> {
    let mut result = Vec::with_capacity(4);
    let mut previous = vertices[vertices.len() - 1];
    for &current in vertices {
        if (previous[2] >= near) != (current[2] >= near) {
            let t = (near - previous[2]) / (current[2] - previous[2]);
            let mut intersection =
                std::array::from_fn(|i| previous[i] + t * (current[i] - previous[i]));
            intersection[2] = near;
            result.push(intersection);
        }
        if current[2] >= near {
            result.push(current);
        }
        previous = current;
    }
    result
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| a[i] + b[i])
}
fn mul(a: [f32; 3], s: f32) -> [f32; 3] {
    a.map(|v| v * s)
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}
fn length(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn edge(a: [f32; 3], b: [f32; 3], x: f32, y: f32) -> f32 {
    (x - a[0]) * (b[1] - a[1]) - (y - a[1]) * (b[0] - a[0])
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn anchored_snapshot_roundtrip_camera_orientation_and_malformed_metadata() {
        let mut scene = Scene::new(vec![Triangle {
            points: [[-1., -1., 0.], [1., -1., 0.], [0., 1., 0.]],
            surface: 0,
        }])
        .unwrap();
        scene.starts.push(rc_package::level::PlayerStart {
            actor: "PlayerStart0".into(),
            location: [0., 0., 2.],
            rotation: [0, 16384, 0],
        });
        let bytes = scene.snapshot().unwrap();
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 3);
        let recovered = Scene::from_snapshot(&bytes).unwrap();
        assert_eq!(recovered.starts, scene.starts);
        let view = recovered.start_view(0).unwrap();
        let forward = [
            -view.yaw.cos() * view.pitch.cos(),
            -view.yaw.sin() * view.pitch.cos(),
            -view.pitch.sin(),
        ];
        assert!(
            forward[0].abs() < 1e-6 && (forward[1] - 1.).abs() < 1e-6 && forward[2].abs() < 1e-6
        );
        assert!(recovered.start_view(1).is_err());
        for len in 0..bytes.len() {
            assert!(Scene::from_snapshot(&bytes[..len]).is_err());
        }
        let mut bad = bytes.clone();
        bad[16..20].copy_from_slice(&4097u32.to_le_bytes());
        assert!(Scene::from_snapshot(&bad).is_err());
        let mut bad = bytes.clone();
        bad[88..92].copy_from_slice(&1025u32.to_le_bytes());
        assert!(Scene::from_snapshot(&bad).is_err());
        let mut bad = bytes.clone();
        bad[92] = 255;
        assert!(Scene::from_snapshot(&bad).is_err());
        let mut bad = bytes;
        let location = 92 + "PlayerStart0".len();
        bad[location..location + 4].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(Scene::from_snapshot(&bad).is_err());
        scene.starts[0].rotation = [16384, 0, 0];
        assert!(scene.start_view(0).is_err());
        scene.starts[0].rotation = [-8192, 0, 0];
        let view = scene.start_view(0).unwrap();
        assert!((view.pitch - std::f32::consts::FRAC_PI_4).abs() < 1e-6);
        scene.starts[0].rotation[2] = 4;
        assert!(scene.start_view(0).is_err());
    }
    #[test]
    fn near_clipping_preserves_uv_and_visible_portion() {
        let clipped = clip_near(
            &[
                [0., 0., 0., 0., 0.],
                [2., 0., 2., 1., 0.],
                [0., 2., 2., 0., 1.],
            ],
            1.,
        );
        assert_eq!(clipped.len(), 4);
        assert_eq!(clipped[0], [0., 1., 1., 0., 0.5]);
        assert_eq!(clipped[1], [1., 0., 1., 0.5, 0.]);
        let scene = Scene::new(vec![Triangle {
            points: [[1., 0., 0.5], [-1., -1., -1.], [-1., 1., 1.]],
            surface: 0,
        }])
        .unwrap();
        let camera = View {
            yaw: 0.,
            pitch: 0.,
            zoom: 1.,
            position: Some([0.; 3]),
        };
        assert!(scene
            .render_view(64, 64, camera)
            .unwrap()
            .iter()
            .any(|&p| p != 0xff101923));
        assert!(scene
            .render_view(
                64,
                64,
                View {
                    position: Some([f32::NAN, 0., 0.]),
                    ..camera
                }
            )
            .is_err());
        assert!(scene
            .render_view(
                64,
                64,
                View {
                    position: Some([-2., 0., 0.]),
                    ..camera
                }
            )
            .unwrap()
            .iter()
            .all(|&p| p == 0xff101923));
    }
    #[test]
    fn textured_snapshot_bounds_references_render_and_alpha() {
        let mut scene = Scene::new(vec![Triangle {
            points: [[0.0, -1.0, -1.0], [0.0, 1.0, -1.0], [0.0, 0.0, 1.0]],
            surface: 0,
        }])
        .unwrap();
        scene.textures.push(Texture {
            width: 1,
            height: 1,
            pixels: vec![0xffff0000],
        });
        scene.appearance.push(Some(FaceTexture {
            uv: [[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]],
            texture: 0,
        }));
        let bytes = scene.snapshot().unwrap();
        let mut recovered = Scene::from_snapshot(&bytes).unwrap();
        let pixels = recovered.render(64, 64, 0.0, 0.0, 1.0).unwrap();
        assert!(pixels
            .iter()
            .any(|&p| p & 0x00ff0000 > 0 && p & 0xffff == 0));
        recovered.textures[0].pixels[0] = 0;
        assert!(recovered
            .render(64, 64, 0.0, 0.0, 1.0)
            .unwrap()
            .iter()
            .all(|&p| p == 0xff101923));
        for len in 0..bytes.len() {
            assert!(Scene::from_snapshot(&bytes[..len]).is_err());
        }
        let mut bad = bytes.clone();
        bad[80..84].copy_from_slice(&1i32.to_le_bytes());
        assert!(Scene::from_snapshot(&bad).is_err());
        let mut bad = bytes;
        bad[56..60].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(Scene::from_snapshot(&bad).is_err());
    }
    #[test]
    fn snapshot_rejects_truncation_counts_and_nonfinite_coordinates() {
        let scene = Scene::new(vec![Triangle {
            points: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            surface: u32::MAX as usize,
        }])
        .unwrap();
        let bytes = scene.snapshot().unwrap();
        let recovered = Scene::from_map(&bytes).unwrap();
        assert_eq!(recovered.triangles[0].points, scene.triangles[0].points);
        // Large material identifiers must not overflow the debug rasterizer.
        recovered.render(32, 32, 0.0, 0.5, 1.0).unwrap();
        for len in 0..bytes.len() {
            assert!(Scene::from_snapshot(&bytes[..len]).is_err());
        }
        let mut bad = bytes.clone();
        bad[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Scene::from_snapshot(&bad).is_err());
        let mut bad = bytes.clone();
        bad[12..16].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(Scene::from_snapshot(&bad).is_err());
        let mut bad = bytes;
        bad.push(0);
        assert!(Scene::from_snapshot(&bad).is_err());
    }
    #[test]
    fn renders_both_windings_and_checks_parameters() {
        let t = Triangle {
            points: [[0.0, -1.0, -1.0], [0.0, 1.0, -1.0], [0.0, 0.0, 1.0]],
            surface: 0,
        };
        let s = Scene::new(vec![t.clone()]).unwrap();
        let image = s.render(64, 64, 0.0, 0.0, 1.0).unwrap();
        assert!(image.iter().any(|&p| p != 0xff101923));
        let mut reversed = t;
        reversed.points.swap(0, 2);
        assert_eq!(
            Scene::new(vec![reversed])
                .unwrap()
                .render(64, 64, 0.0, 0.0, 1.0)
                .unwrap(),
            image
        );
        assert!(s.render(2049, 64, 0.0, 0.0, 1.0).is_err());
        assert!(s.render(64, 64, f32::NAN, 0.0, 1.0).is_err());
    }
}
