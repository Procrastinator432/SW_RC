use rc_package::{
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
    pub source: String,
    pub texture: rc_render::Texture,
    pub scale: f32,
    pub chain: Vec<String>,
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
                if path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("utx") || e.eq_ignore_ascii_case("usx"))
                {
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
        let mut path = path.to_owned();
        let mut visited = HashSet::new();
        let mut scale = 1.0;
        let mut chain = Vec::new();
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
                    .min_by_key(|m| m.width.max(m.height).abs_diff(64))
                    .ok_or("No stored texture mip")?;
                let pixels = texture::decode(t.format, mip, palette.as_deref())?;
                // Bound snapshot size; use original mips when available, nearest sampling otherwise.
                let width = mip.width.min(64);
                let height = mip.height.min(64);
                let resized = (0..height)
                    .flat_map(|y| {
                        (0..width).map(move |x| {
                            (y * mip.height / height) * mip.width + x * mip.width / width
                        })
                    })
                    .map(|i| pixels[i])
                    .collect();
                return Ok(Diffuse {
                    source: path,
                    scale,
                    chain,
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
