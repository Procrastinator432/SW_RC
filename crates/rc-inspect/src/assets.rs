use rc_package::{
    classes,
    material_jitter::{JitterState, TexJitter},
    material_panner::TexPanner,
    material_uv::{TexPanner2D, UvTransform},
    properties::{self, Value},
    read_package, texture, Package,
};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};
#[derive(Clone, Debug, serde::Serialize)]
pub struct ShaderConstant {
    pub slot: usize,
    pub kind: u8,
    pub value: [f32; 4],
}
#[derive(Clone, Debug, serde::Serialize)]
pub struct ShaderTexture {
    pub slot: u32,
    pub material: Option<String>,
}
#[derive(Clone, Debug, serde::Serialize)]
pub struct HardwareShaderInputs {
    pub vertex_source: String,
    pub pixel_source: String,
    pub vertex_constants: Vec<ShaderConstant>,
    pub pixel_constants: Vec<ShaderConstant>,
    pub textures: Vec<ShaderTexture>,
}
pub struct Assets {
    files: HashMap<String, PathBuf>,
    loaded: HashMap<String, (Package, Vec<u8>)>,
}
#[derive(Default)]
pub struct MaterialState {
    pub oscillators: HashMap<String, JitterState>,
}
type JitterHost<'a> = Option<(
    &'a mut MaterialState,
    &'a mut dyn FnMut() -> Result<u16, String>,
)>;
pub struct Diffuse {
    pub jitter: Option<TexJitter>,
    pub jitter_state: Option<JitterState>,
    pub directional_panner: Option<TexPanner>,
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
    /// Explicit serialized inputs only; no wrapper overrides or sampler/state defaults.
    pub fn hardware_shader_inputs(&mut self, path: &str) -> Result<HardwareShaderInputs, String> {
        let (class, props) = self.material_properties(path)?;
        if class != "Engine.HardwareShader" {
            return Err("Expected HardwareShader".into());
        }
        let package = path.split_once('.').ok_or("Incomplete shader path")?.0;
        let text = |name: &str| -> Result<String, String> {
            let matches: Vec<_> = props
                .values
                .iter()
                .filter(|p| p.name == name && p.array_index == 0)
                .collect();
            match matches.as_slice() {
                [p] => match &p.value {
                    Value::String(s) => Ok(s.clone()),
                    _ => Err(format!("Invalid {name}")),
                },
                _ => Err(format!("Missing or duplicate {name}")),
            }
        };
        let vertex_source = text("VertexShaderText")?;
        let pixel_source = text("PixelShaderText")?;
        let mut textures = Vec::new();
        let mut seen = HashSet::new();
        for property in &props.values {
            if property.name != "Textures" {
                continue;
            }
            if !seen.insert(property.array_index) {
                return Err("Duplicate shader texture slot".into());
            }
            let material = match &property.value {
                Value::Object { index, path } => {
                    if *index == 0 {
                        None
                    } else {
                        Some(self.qualify(package, path))
                    }
                }
                _ => return Err("Invalid shader texture reference".into()),
            };
            textures.push(ShaderTexture {
                slot: property.array_index,
                material,
            });
        }
        Ok(HardwareShaderInputs {
            vertex_source,
            pixel_source,
            vertex_constants: self.shader_constant_bindings(path, "VSConstants")?,
            pixel_constants: self.shader_constant_bindings(path, "PSConstants")?,
            textures,
        })
    }
    /// Material-defined pixel constants only; dynamic engine constants need a host.
    pub fn pixel_constants(&mut self, path: &str) -> Result<[[f32; 4]; 8], String> {
        let mut values = [[0.; 4]; 8];
        for c in self.shader_constant_bindings(path, "PSConstants")? {
            match c.kind {
                0 => {}
                1 => values[c.slot] = c.value,
                _ => return Err("Dynamic pixel constant requires engine host".into()),
            }
        }
        Ok(values)
    }
    /// Serialized bindings, retaining dynamic kinds instead of inventing host values.
    pub fn shader_constant_bindings(
        &mut self,
        path: &str,
        field: &str,
    ) -> Result<Vec<ShaderConstant>, String> {
        let limit = match field {
            "PSConstants" => 8,
            "VSConstants" => 96,
            _ => return Err("Unknown shader constant array".into()),
        };
        let (class, props) = self.material_properties(path)?;
        if class != "Engine.HardwareShader" {
            return Err("Expected HardwareShader".into());
        }
        let (package, object) = path.split_once('.').ok_or("Incomplete shader path")?;
        let (pkg, data) = &self.loaded[&package.to_lowercase()];
        let export = pkg
            .exports
            .iter()
            .enumerate()
            .find_map(|(i, e)| {
                pkg.object_path(i as i32 + 1)
                    .ok()?
                    .eq_ignore_ascii_case(object)
                    .then_some(e)
            })
            .ok_or("Missing shader export")?;
        let payload = pkg.payload(data, export)?;
        let slice = |offset: usize, count: usize| payload.get(offset..offset.checked_add(count)?);
        let mut constants = vec![];
        let mut seen = vec![false; limit];
        for p in props.values.iter().filter(|p| p.name == field) {
            let slot = p.array_index as usize;
            if slot >= limit || seen[slot] || p.struct_name.as_deref() != Some("SConstantsInfo") {
                return Err("Invalid pixel constant slot/struct".into());
            }
            seen[slot] = true;
            let raw = slice(p.payload_offset, p.bytes).ok_or("Pixel constant payload range")?;
            let nested = properties::tagged_struct(pkg, raw)?;
            let kind = nested
                .values
                .iter()
                .find(|p| p.name == "Type")
                .map(|p| &p.value);
            let kind = match kind {
                None => 0,
                Some(Value::Byte(v)) => *v,
                _ => return Err("Expected byte shader constant kind".into()),
            };
            let mut components = [0.; 4];
            if let Some(value) = nested.values.iter().find(|p| p.name == "Value") {
                if value.struct_name.as_deref() != Some("Plane") {
                    return Err("Expected pixel constant Plane".into());
                }
                let plane = raw
                    .get(
                        value.payload_offset
                            ..value
                                .payload_offset
                                .checked_add(value.bytes)
                                .ok_or("Plane overflow")?,
                    )
                    .ok_or("Plane payload range")?;
                let plane = properties::tagged_struct(pkg, plane)?;
                for (i, name) in ["X", "Y", "Z", "W"].iter().enumerate() {
                    if let Some(p) = plane.values.iter().find(|p| p.name == *name) {
                        let Value::Float(v) = p.value else {
                            return Err("Expected float plane component".into());
                        };
                        if !v.is_finite() {
                            return Err("Nonfinite pixel constant".into());
                        }
                        components[i] = v;
                    }
                }
            }
            constants.push(ShaderConstant {
                slot,
                kind,
                value: components,
            });
        }
        Ok(constants)
    }
    /// Read original tagged material inputs without claiming shader evaluation.
    pub fn material_properties(
        &mut self,
        path: &str,
    ) -> Result<(String, properties::Properties), String> {
        let (package, object) = path.split_once('.').ok_or("Incomplete material path")?;
        let key = package.to_lowercase();
        if !self.loaded.contains_key(&key) {
            let data = fs::read(self.files.get(&key).ok_or("Missing material package")?)
                .map_err(|e| e.to_string())?;
            self.loaded
                .insert(key.clone(), (read_package(&data)?, data));
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
            .ok_or("Missing material export")?;
        Ok((pkg.object_path(e.class)?, properties::read(pkg, data, e)?))
    }
    pub fn class_properties(&mut self, path: &str) -> Result<properties::Properties, String> {
        let (package, object) = path.split_once('.').ok_or("Incomplete class path")?;
        let key = package.to_lowercase();
        if !self.loaded.contains_key(&key) {
            let data = fs::read(self.files.get(&key).ok_or("Missing class package")?)
                .map_err(|e| e.to_string())?;
            self.loaded
                .insert(key.clone(), (read_package(&data)?, data));
        }
        let (pkg, data) = &self.loaded[&key];
        let e = pkg
            .exports
            .iter()
            .find(|e| e.class == 0 && e.name.eq_ignore_ascii_case(object))
            .ok_or("Missing class export")?;
        Ok(classes::read(pkg, data, e)?.properties)
    }
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
        self.resolve_diffuse(path, limit, None, None)
    }
    /// Explicit time-sampled path. Legacy scalar-only consumers keep rejecting panners.
    pub fn diffuse_at_time(
        &mut self,
        path: &str,
        limit: usize,
        time: f32,
    ) -> Result<Diffuse, String> {
        self.sampled_diffuse(path, limit, time, None)
    }
    /// Retain oscillator state by qualified material object; host owns shared rand order.
    pub fn diffuse_with_jitter(
        &mut self,
        path: &str,
        limit: usize,
        time: f32,
        state: &mut MaterialState,
        random: &mut dyn FnMut() -> Result<u16, String>,
    ) -> Result<Diffuse, String> {
        self.sampled_diffuse(path, limit, time, Some((state, random)))
    }
    fn sampled_diffuse(
        &mut self,
        path: &str,
        limit: usize,
        time: f32,
        jitter: JitterHost<'_>,
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
        let class = pkg
            .exports
            .iter()
            .find(|e| e.name == "TexPanner" && e.class == 0)
            .ok_or("Missing TexPanner class")?;
        let directional_defaults = classes::read(pkg, data, class)?.properties;
        let class = pkg
            .exports
            .iter()
            .find(|e| e.name == "TexOscillator" && e.class == 0)
            .ok_or("Missing TexOscillator class")?;
        let oscillator_defaults = classes::read(pkg, data, class)?.properties;
        self.resolve_diffuse(
            path,
            limit,
            Some((time, defaults, directional_defaults, oscillator_defaults)),
            jitter,
        )
    }
    fn resolve_diffuse(
        &mut self,
        path: &str,
        limit: usize,
        timed: Option<(
            f32,
            properties::Properties,
            properties::Properties,
            properties::Properties,
        )>,
        mut jitter_host: JitterHost<'_>,
    ) -> Result<Diffuse, String> {
        if limit == 0 || limit > 1024 {
            return Err("Diffuse preview size outside 1..1024".into());
        }
        let mut path = path.to_owned();
        let mut visited = HashSet::new();
        let mut scale = 1.0;
        let mut chain = Vec::new();
        let mut panner = None;
        let mut directional_panner = None;
        let mut uv_transform = None;
        let mut jitter = None;
        let mut jitter_state = None;
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
                    jitter,
                    jitter_state,
                    uv_transform,
                    directional_panner,
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
                "Engine.TexOscillator" if jitter_host.is_some() => {
                    if uv_transform.is_some() || scale != 1. {
                        return Err("Multiple/composed UV modifiers not supported yet".into());
                    }
                    let timed = timed.as_ref().ok_or("Jitter needs material time")?;
                    let (parameters, initial) = TexJitter::from_properties(&timed.3, &props)?;
                    let (runtime, random) = jitter_host.as_mut().unwrap();
                    let state = runtime
                        .oscillators
                        .entry(path.to_lowercase())
                        .or_insert(initial);
                    uv_transform = Some(parameters.step(timed.0, state, *random)?);
                    jitter = Some(parameters);
                    jitter_state = Some(*state);
                    "Material"
                }
                "Engine.TexPanner" if timed.is_some() => {
                    if uv_transform.is_some() || scale != 1. {
                        return Err("Multiple/composed UV modifiers not supported yet".into());
                    }
                    let parameters =
                        TexPanner::from_properties(&timed.as_ref().unwrap().2, &props)?;
                    uv_transform = Some(parameters.at(timed.as_ref().unwrap().0)?);
                    directional_panner = Some(parameters);
                    "Material"
                }
                "Engine.TexPanner2D" if timed.is_some() => {
                    if uv_transform.is_some() || scale != 1. {
                        return Err("Multiple/composed UV modifiers not supported yet".into());
                    }
                    let parameters =
                        TexPanner2D::from_properties(&timed.as_ref().unwrap().1, &props)?;
                    uv_transform = Some(parameters.at(timed.as_ref().unwrap().0)?);
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
            if uv_transform.is_some() && scale != 1. {
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
