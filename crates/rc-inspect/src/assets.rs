use rc_package::{
    classes,
    material_uv::{TexPanner2D, UvTransform},
    properties::{self, Value},
    read_package, texture, Package,
};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};
pub struct Assets {
    files: HashMap<String, PathBuf>,
    loaded: HashMap<String, (Package, Vec<u8>)>,
}
pub struct Diffuse {
    pub uv_transform: Option<UvTransform>,
    pub panner: Option<TexPanner2D>,
    pub source: String,
    pub texture: rc_render::Texture,
    pub scale: f32,
    pub chain: Vec<String>,
    pub format: u8,
    pub source_mip: [usize; 2],
}
impl Assets {
    pub fn qualify(&self, package: &str, path: &str) -> String {
        if path
            .split_once('.')
            .is_some_and(|(p, _)| self.files.contains_key(&p.to_lowercase()))
        {
            path.to_owned()
        } else {
            format!("{package}.{path}")
        }
    }
    pub fn new(directories: &[&Path]) -> Result<Self, String> {
        let mut files = HashMap::new();
        for directory in directories {
            for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
                let path = entry.map_err(|e| e.to_string())?.path();
                if path.extension().is_some_and(|e| {
                    ["utx", "usx", "ukx", "u"]
                        .iter()
                        .any(|ext| e.eq_ignore_ascii_case(ext))
                }) {
                    if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                        files.insert(name.to_lowercase(), path);
                    }
                }
            }
        }
        Ok(Self {
            files,
            loaded: HashMap::new(),
        })
    }
    pub fn diffuse(&mut self, path: &str) -> Result<Diffuse, String> {
        self.diffuse_at_size(path, 64)
    }
    pub fn diffuse_at_size(&mut self, path: &str, limit: usize) -> Result<Diffuse, String> {
        self.resolve_diffuse(path, limit, None)
    }
    /// Explicit time-sampled path. Legacy scalar-only consumers keep rejecting panners.
    pub fn diffuse_at_time(
        &mut self,
        path: &str,
        limit: usize,
        time: f32,
    ) -> Result<Diffuse, String> {
        if !time.is_finite() {
            return Err("Nonfinite material time".into());
        }
        if !self.loaded.contains_key("engine") {
            let data = fs::read(
                self.files
                    .get("engine")
                    .ok_or("Missing Engine package defaults")?,
            )
            .map_err(|e| e.to_string())?;
            self.loaded
                .insert("engine".into(), (read_package(&data)?, data));
        }
        let (pkg, data) = &self.loaded["engine"];
        let class = pkg
            .exports
            .iter()
            .find(|e| e.name == "TexPanner2D" && e.class == 0)
            .ok_or("Missing TexPanner2D class")?;
        let defaults = classes::read(pkg, data, class)?.properties;
        self.resolve_diffuse(path, limit, Some((time, defaults)))
    }
    fn resolve_diffuse(
        &mut self,
        path: &str,
        limit: usize,
        timed: Option<(f32, properties::Properties)>,
    ) -> Result<Diffuse, String> {
        if limit == 0 || limit > 1024 {
            return Err("Diffuse preview size outside 1..1024".into());
        }
        let mut path = path.to_owned();
        let mut visited = HashSet::new();
        let mut scale = 1.0;
        let mut chain = Vec::new();
        let mut panner = None;
        for _ in 0..16 {
            if !visited.insert(path.to_lowercase()) {
                return Err("Material cycle".into());
            }
            let (package, object) = path.split_once('.').ok_or("Incomplete material path")?;
            let key = package.to_lowercase();
            if !self.loaded.contains_key(&key) {
                let file = self
                    .files
                    .get(&key)
                    .ok_or_else(|| format!("Missing material package {package}"))?;
                let data = fs::read(file).map_err(|e| e.to_string())?;
                let pkg = read_package(&data)?;
                self.loaded.insert(key.clone(), (pkg, data));
            }
            let (pkg, data) = &self.loaded[&key];
            let e = pkg
                .exports
                .iter()
                .enumerate()
                .find_map(|(i, e)| {
                    pkg.object_path(i as i32 + 1)
                        .ok()?
                        .eq_ignore_ascii_case(object)
                        .then_some(e)
                })
                .ok_or_else(|| format!("Missing material {path}"))?;
            let class = pkg.object_path(e.class)?;
            chain.push(class.clone());
            if class == "Engine.Texture" {
                let t = texture::read_texture(pkg, data, e)?;
                let palette = if t.format == 0 {
                    let index = t.palette.ok_or("Missing palette reference")?;
                    if index <= 0 {
                        return Err("External P8 palette not supported yet".into());
                    }
                    Some(texture::read_palette(
                        pkg,
                        data,
                        pkg.exports
                            .get(index as usize - 1)
                            .ok_or("Invalid palette reference")?,
                    )?)
                } else {
                    None
                };
                let mip = t
                    .mips
                    .iter()
                    .filter(|m| !m.data.is_empty())
                    .min_by_key(|m| m.width.max(m.height).abs_diff(limit))
                    .ok_or("No stored texture mip")?;
                let pixels = texture::decode(t.format, mip, palette.as_deref())?;
                // Bound snapshot size; use original mips when available, nearest sampling otherwise.
                let width = mip.width.min(limit);
                let height = mip.height.min(limit);
                let resized = (0..height)
                    .flat_map(|y| {
                        (0..width).map(move |x| {
                            (y * mip.height / height) * mip.width + x * mip.width / width
                        })
                    })
                    .map(|i| pixels[i])
                    .collect();
                return Ok(Diffuse {
                    uv_transform: panner
                        .map(|p: TexPanner2D| p.at(timed.as_ref().unwrap().0))
                        .transpose()?,
                    panner,
                    source: path,
                    scale,
                    chain,
                    format: t.format,
                    source_mip: [mip.width, mip.height],
                    texture: rc_render::Texture {
                        width,
                        height,
                        pixels: resized,
                    },
                });
            }
            let props = properties::read(pkg, data, e)?;
            let number = |name: &str| {
                props.values.iter().find_map(|p| {
                    if p.name == name {
                        match p.value {
                            Value::Float(v) => Some(v),
                            Value::Int(v) => Some(v as f32),
                            Value::Byte(v) => Some(v as f32),
                            _ => None,
                        }
                    } else {
                        None
                    }
                })
            };
            let field = match class.as_str() {
                "Engine.TexPanner2D" if timed.is_some() => {
                    if panner.is_some() || scale != 1. {
                        return Err("Multiple/composed UV modifiers not supported yet".into());
                    }
                    let parameters =
                        TexPanner2D::from_properties(&timed.as_ref().unwrap().1, &props)?;
                    parameters.at(timed.as_ref().unwrap().0)?;
                    panner = Some(parameters);
                    "Material"
                }
                "Engine.Shader" => "Diffuse",
                "Engine.FinalBlend" => "Material",
                "Engine.HsBumpDiff"
                | "Engine.HsBumpDiffSpec"
                | "Engine.HsBumpDiffSpecMask"
                | "Engine.HsBumpIllumSpecMask"
                | "Engine.HsBumpDiffBlend"
                | "Engine.HsBumpDiffBlendMask"
                | "Engine.HsBumpDiffBlendMaskIllum" => {
                    scale *= number("DiffUVScale").unwrap_or(1.0);
                    "DiffuseTexture"
                }
                // Diagnostic base-layer preview; combine effects remain explicitly out of scope.
                "Engine.Combiner" => match number("CombineOperation").unwrap_or(0.0) as u8 {
                    1 => "Material2",
                    7 => "Mask",
                    _ => "Material1",
                },
                "Engine.TexCoordSource" if number("SourceChannel").unwrap_or(0.0) == 0.0 => {
                    "Material"
                }
                _ => return Err(format!("Unsupported material class {class}")),
            };
            if !scale.is_finite() {
                return Err("Nonfinite material UV scale".into());
            }
            if panner.is_some() && scale != 1. {
                return Err("Multiple/composed UV modifiers not supported yet".into());
            }
            let (index, next) = props
                .values
                .into_iter()
                .find_map(|p| {
                    if p.name == field {
                        if let Value::Object { index, path } = p.value {
                            Some((index, path))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .ok_or_else(|| format!("No {field} in {path}"))?;
            if index == 0 {
                return Err("Empty diffuse reference".into());
            }
            path = if index > 0 {
                format!("{package}.{next}")
            } else {
                next
            };
        }
        Err("Material chain exceeds limit".into())
    }
}
