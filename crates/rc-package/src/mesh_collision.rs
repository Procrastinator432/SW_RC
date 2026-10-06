//! Original StaticMesh collision prefix: optional Model and triangle BSP tree.
//! This does not implement UStaticMesh::LineCheck, extent sweeps or pawn physics.
use crate::{mesh::StaticMesh, Export, Package, Reader};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct CollisionTriangle {
    pub vertices: [u16; 3],
    pub material: i32,
}
#[derive(Clone, Debug, Serialize)]
pub struct CollisionNode {
    pub triangle: u16,
    /// Coplanar, front, back; 65535 is the absent-child sentinel.
    pub children: [u16; 3],
    pub compressed_bounds: [[i16; 3]; 2],
}
#[derive(Debug, Serialize)]
pub struct MeshCollision {
    pub simple_model: i32,
    pub simple_model_path: String,
    pub serialized_shadow_only: Option<u32>,
    pub use_simple_line: bool,
    pub use_simple_box: bool,
    pub use_simple_karma: bool,
    pub triangles: Vec<CollisionTriangle>,
    pub nodes: Vec<CollisionNode>,
    pub bounds_scale: [f32; 3],
    pub decoded_bytes: usize,
    pub remaining_bytes: usize,
}

fn word(r: &mut Reader<'_>) -> Result<u16, String> {
    Ok(u16::from_le_bytes(r.take(2)?.try_into().unwrap()))
}
pub fn read(
    pkg: &Package,
    data: &[u8],
    export: &Export,
    mesh: &StaticMesh,
) -> Result<MeshCollision, String> {
    if !(133..=159).contains(&pkg.summary.version) || pkg.summary.licensee_version > 1 {
        return Err("unsupported StaticMesh collision variant".into());
    }
    let payload = pkg.payload(data, export)?;
    let mut r = Reader::at(payload, mesh.geometry_bytes)?;
    let simple_model = r.index()?;
    let simple_model_path = pkg.object_path(simple_model)?;
    if simple_model > 0 {
        let model = &pkg.exports[simple_model as usize - 1];
        if !pkg
            .object_path(model.class)?
            .eq_ignore_ascii_case("Engine.Model")
        {
            return Err("StaticMesh simple collision is not a Model".into());
        }
    } else if simple_model < 0
        && !pkg.imports[simple_model.unsigned_abs() as usize - 1]
            .class_name
            .eq_ignore_ascii_case("Model")
    {
        return Err("StaticMesh imported simple collision is not a Model".into());
    }
    let serialized_shadow_only = if pkg.summary.version > 150 {
        Some(r.u32()?)
    } else {
        None
    };
    let properties = crate::properties::read(pkg, data, export)?;
    // UStaticMesh::StaticConstructor sets these native defaults to true.
    let simple_flag = |name: &str| -> Result<bool, String> {
        match properties
            .values
            .iter()
            .rev()
            .find(|p| p.name == name)
            .map(|p| &p.value)
        {
            Some(crate::properties::Value::Bool(value)) => Ok(*value),
            Some(_) => Err(format!("{name} is not a Bool property")),
            None => Ok(true),
        }
    };
    let count = r.count(7)?;
    let mut triangles = Vec::with_capacity(count);
    for _ in 0..count {
        let vertices = [word(&mut r)?, word(&mut r)?, word(&mut r)?];
        let material = r.index()?;
        if vertices.iter().any(|&v| v as usize >= mesh.vertices.len())
            || material < 0
            || material as usize >= mesh.materials.len()
        {
            return Err("invalid collision triangle vertex/material reference".into());
        }
        triangles.push(CollisionTriangle { vertices, material });
    }
    let count = r.count(20)?;
    if count > 65535 {
        return Err("collision tree exceeds 16-bit node range".into());
    }
    let mut nodes = Vec::with_capacity(count);
    for _ in 0..count {
        let triangle = word(&mut r)?;
        let children = [word(&mut r)?, word(&mut r)?, word(&mut r)?];
        let compressed_bounds = [
            [
                word(&mut r)? as i16,
                word(&mut r)? as i16,
                word(&mut r)? as i16,
            ],
            [
                word(&mut r)? as i16,
                word(&mut r)? as i16,
                word(&mut r)? as i16,
            ],
        ];
        nodes.push(CollisionNode {
            triangle,
            children,
            compressed_bounds,
        });
    }
    let bounds_scale = r.vector()?;
    let result = MeshCollision {
        simple_model,
        simple_model_path,
        serialized_shadow_only,
        use_simple_line: simple_flag("UseSimpleLineCollision")?,
        use_simple_box: simple_flag("UseSimpleBoxCollision")?,
        use_simple_karma: simple_flag("UseSimpleKarmaCollision")?,
        triangles,
        nodes,
        bounds_scale,
        decoded_bytes: r.pos,
        remaining_bytes: payload.len() - r.pos,
    };
    result.validate()?;
    Ok(result)
}
impl MeshCollision {
    pub fn expanded_bounds(&self, node: usize) -> Result<[[f64; 3]; 2], String> {
        let bounds = self
            .nodes
            .get(node)
            .ok_or("invalid collision node")?
            .compressed_bounds;
        Ok(bounds
            .map(|v| std::array::from_fn(|i| f64::from(v[i]) * f64::from(self.bounds_scale[i]))))
    }
    fn validate(&self) -> Result<(), String> {
        if self.bounds_scale.iter().any(|v| !v.is_finite() || *v < 0.0) {
            return Err("invalid collision bounds scale".into());
        }
        for node in &self.nodes {
            if node
                .compressed_bounds
                .iter()
                .any(|v| (0..3).any(|i| !(f32::from(v[i]) * self.bounds_scale[i]).is_finite()))
            {
                return Err("collision bounds expansion overflows".into());
            }
            if node.triangle as usize >= self.triangles.len()
                || node
                    .children
                    .iter()
                    .any(|&c| c != u16::MAX && c as usize >= self.nodes.len())
            {
                return Err("invalid collision node triangle/child reference".into());
            }
            if (0..3).any(|i| node.compressed_bounds[0][i] > node.compressed_bounds[1][i]) {
                return Err("inverted collision bounds".into());
            }
        }
        let mut colors = vec![0; self.nodes.len()];
        for root in 0..self.nodes.len() {
            let mut stack = vec![(root, false)];
            while let Some((index, leaving)) = stack.pop() {
                if leaving {
                    colors[index] = 2;
                    continue;
                }
                if colors[index] == 1 {
                    return Err("collision tree cycle".into());
                }
                if colors[index] == 2 {
                    continue;
                }
                colors[index] = 1;
                stack.push((index, true));
                for child in self.nodes[index].children {
                    if child != u16::MAX {
                        stack.push((child as usize, false));
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::Vertex;
    fn fixture(version: u16) -> (Package, Export, StaticMesh, Vec<u8>) {
        let pkg = Package {
            summary: crate::Summary {
                version,
                licensee_version: 1,
                flags: 0,
                name_count: 1,
                name_offset: 0,
                import_count: 0,
                import_offset: 0,
                export_count: 0,
                export_offset: 0,
            },
            names: vec!["None".into()],
            imports: vec![],
            exports: vec![],
        };
        let mesh = StaticMesh {
            materials: vec![None],
            bounds: [[0.0; 3]; 2],
            sections: vec![],
            vertices: vec![
                Vertex {
                    position: [0.0; 3],
                    normal: [0.0; 3],
                },
                Vertex {
                    position: [1.0, 0.0, 0.0],
                    normal: [0.0; 3],
                },
                Vertex {
                    position: [0.0, 1.0, 0.0],
                    normal: [0.0; 3],
                },
            ],
            uv_streams: vec![],
            indices: vec![],
            wire_indices: vec![],
            geometry_bytes: 1,
            unparsed_tail_bytes: 0,
        };
        let mut data = vec![0, 0]; // property None, optional Model None
        if version > 150 {
            data.extend_from_slice(&1u32.to_le_bytes());
        }
        data.push(1);
        for v in [0u16, 1, 2] {
            data.extend_from_slice(&v.to_le_bytes());
        }
        data.push(0); // material CompactIndex
        data.push(1);
        for v in [0u16, 65535, 65535, 65535] {
            data.extend_from_slice(&v.to_le_bytes());
        }
        for v in [-2i16, -2, -2, 2, 2, 2] {
            data.extend_from_slice(&v.to_le_bytes());
        }
        for v in [0.5f32, 0.25, 1.0] {
            data.extend_from_slice(&v.to_le_bytes());
        }
        let export = Export {
            class: 0,
            super_class: 0,
            outer: 0,
            extra: None,
            name: "Mesh".into(),
            flags: 0,
            serial_offset: 0,
            serial_size: data.len() as u32,
        };
        (pkg, export, mesh, data)
    }
    #[test]
    fn native_collision_prefix_versions_bounds_and_every_truncation() {
        for version in [133, 150, 151, 159] {
            let (pkg, mut export, mesh, mut data) = fixture(version);
            let collision = read(&pkg, &data, &export, &mesh).unwrap();
            assert_eq!(collision.triangles.len(), 1);
            assert_eq!(
                collision.expanded_bounds(0).unwrap(),
                [[-1.0, -0.5, -2.0], [1.0, 0.5, 2.0]]
            );
            assert!(
                collision.use_simple_line && collision.use_simple_box && collision.use_simple_karma
            );
            for end in 0..data.len() {
                export.serial_size = end as u32;
                assert!(
                    read(&pkg, &data[..end], &export, &mesh).is_err(),
                    "version {version} prefix {end}"
                );
            }
            data.push(42);
            export.serial_size = data.len() as u32;
            assert_eq!(
                read(&pkg, &data, &export, &mesh).unwrap().remaining_bytes,
                1
            );
        }
    }
    #[test]
    fn invalid_triangle_material_children_cycles_and_bounds_are_rejected() {
        let (pkg, export, mesh, data) = fixture(159);
        for (offset, value) in [(7, 3), (13, 1), (17, 1)] {
            let mut bad = data.clone();
            bad[offset] = value;
            assert!(
                read(&pkg, &bad, &export, &mesh).is_err(),
                "bad byte {offset}={value}"
            );
        }
        for (offset, value) in [(15, 1u16), (17, 0), (23, 3)] {
            let mut bad = data.clone();
            bad[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
            assert!(
                read(&pkg, &bad, &export, &mesh).is_err(),
                "bad word {offset}={value}"
            );
        }
        let mut bad = data.clone();
        bad[1] = 1;
        assert!(read(&pkg, &bad, &export, &mesh).is_err());
        let mut bad = data.clone();
        bad[35..39].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(read(&pkg, &bad, &export, &mesh).is_err());
        let mut bad = data.clone();
        bad[35..39].copy_from_slice(&(-0.5f32).to_le_bytes());
        assert!(read(&pkg, &bad, &export, &mesh).is_err());
        let mut bad = data.clone();
        bad[35..39].copy_from_slice(&f32::MAX.to_le_bytes());
        assert!(read(&pkg, &bad, &export, &mesh).is_err());
    }
    #[test]
    fn serialized_simple_mode_overrides_native_true_default() {
        let (mut pkg, mut export, mut mesh, mut data) = fixture(159);
        pkg.names.push("UseSimpleLineCollision".into());
        data.splice(0..1, [1, 0x03, 0]); // explicit False then property terminator
        mesh.geometry_bytes += 2;
        export.serial_size = data.len() as u32;
        let collision = read(&pkg, &data, &export, &mesh).unwrap();
        assert!(!collision.use_simple_line && collision.use_simple_box);
    }
}
