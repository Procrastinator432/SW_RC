//! SWRC StaticMesh render streams, reconstructed from local engine serializers.
//! Collision prefix is read separately by mesh_collision; editor/lighting tails remain opaque.
use super::{geometry::Triangle, properties, Export, Package, Reader, Result};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Section {
    pub flags: u32,
    pub first_index: u16,
    pub first_vertex: u16,
    pub last_vertex: u16,
    pub triangle_count: u16,
    pub face_count: u16,
}
#[derive(Debug, Serialize)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}
#[derive(Debug, Serialize)]
pub struct StaticMesh {
    pub materials: Vec<Option<String>>,
    pub bounds: [[f32; 3]; 2],
    pub sections: Vec<Section>,
    pub vertices: Vec<Vertex>,
    pub uv_streams: Vec<Vec<[f32; 2]>>,
    pub indices: Vec<u16>,
    pub wire_indices: Vec<u16>,
    pub geometry_bytes: usize,
    pub unparsed_tail_bytes: usize,
}
fn word(r: &mut Reader<'_>) -> Result<u16> {
    Ok(u16::from_le_bytes(r.take(2)?.try_into().unwrap()))
}
fn bounds(r: &mut Reader<'_>) -> Result<[[f32; 3]; 2]> {
    let result = [r.vector()?, r.vector()?];
    r.byte()?;
    Ok(result)
}
fn indices(r: &mut Reader<'_>) -> Result<Vec<u16>> {
    let count = r.count(2)?;
    let result = (0..count).map(|_| word(r)).collect::<Result<_>>()?;
    r.u32()?; // revision
    Ok(result)
}
pub fn read_static_mesh(pkg: &Package, data: &[u8], e: &Export) -> Result<StaticMesh> {
    if !(133..=159).contains(&pkg.summary.version) || pkg.summary.licensee_version > 1 {
        return Err("StaticMesh supports SWRC 133–159 / licensee 0–1 only".into());
    }
    let payload = pkg.payload(data, e)?;
    let props = properties::read(pkg, data, e)?;
    let mut materials = Vec::new();
    if let Some(p) = props.values.iter().find(|p| p.name == "Materials") {
        let structs = properties::struct_array(
            pkg,
            payload
                .get(p.payload_offset..p.payload_offset + p.bytes)
                .ok_or("Materials outside payload")?,
        )?;
        for s in structs {
            materials.push(s.values.into_iter().find_map(|p| {
                if p.name == "Material" {
                    if let properties::Value::Object { path, index } = p.value {
                        if index != 0 {
                            Some(path)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }));
        }
    }
    let mut r = Reader::at(payload, props.native_offset)?;
    bounds(&mut r)?;
    r.vector()?;
    r.f32()?; // UPrimitive sphere
    let count = r.count(14)?;
    let mut sections = Vec::with_capacity(count);
    for _ in 0..count {
        sections.push(Section {
            flags: r.u32()?,
            first_index: word(&mut r)?,
            first_vertex: word(&mut r)?,
            last_vertex: word(&mut r)?,
            triangle_count: word(&mut r)?,
            face_count: word(&mut r)?,
        });
    }
    let bounds = bounds(&mut r)?;
    let count = r.count(24)?;
    let vertices = (0..count)
        .map(|_| {
            Ok(Vertex {
                position: r.vector()?,
                normal: r.vector()?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    r.u32()?; // vertex revision
    if pkg.summary.version >= 155 {
        r.u32()?;
    }
    if pkg.summary.version >= 149 {
        r.u32()?;
    } // vertex/color flag
    for _ in 0..2 {
        let count = r.count(4)?;
        r.take(count * 4)?;
        r.u32()?; // color stream revision
    }
    let count = r.count(9)?;
    let mut uv_streams = Vec::with_capacity(count);
    for _ in 0..count {
        let count = r.count(8)?;
        uv_streams.push(
            (0..count)
                .map(|_| Ok([r.f32()?, r.f32()?]))
                .collect::<Result<Vec<_>>>()?,
        );
        r.u32()?;
        r.u32()?;
    }
    let mesh = StaticMesh {
        materials,
        bounds,
        sections,
        vertices,
        uv_streams,
        indices: indices(&mut r)?,
        wire_indices: indices(&mut r)?,
        geometry_bytes: r.pos,
        unparsed_tail_bytes: payload.len() - r.pos,
    };
    mesh.triangles()?;
    Ok(mesh)
}
impl StaticMesh {
    pub fn triangles(&self) -> Result<Vec<Triangle>> {
        let mut result = Vec::new();
        for (surface, section) in self.sections.iter().enumerate() {
            if section.flags != 0 {
                return Err("Unsupported StaticMesh section topology".into());
            }
            let first = section.first_index as usize;
            let end = first + section.face_count as usize * 3;
            let range = self
                .indices
                .get(first..end)
                .ok_or("Mesh section outside index stream")?;
            for face in range.chunks_exact(3) {
                let mut points = [[0.0; 3]; 3];
                for (j, &index) in face.iter().enumerate() {
                    points[j] = self
                        .vertices
                        .get(index as usize)
                        .ok_or("Mesh index outside vertex stream")?
                        .position;
                }
                result.push(Triangle { points, surface });
            }
        }
        Ok(result)
    }
}

/// Actor::LocalToWorld: subtract PrePivot, scale, rotate, then translate.
/// Rotation uses the original 16-bit turn / 14-bit lookup-table quantization.
pub fn transform_point(
    point: [f32; 3],
    location: [f32; 3],
    rotation: [i32; 3],
    scale: [f32; 3],
    pivot: [f32; 3],
) -> [f32; 3] {
    let sine = |angle: i32| (((angle >> 2) & 16383) as f32 * std::f32::consts::TAU / 16384.0).sin();
    let cosine = |angle: i32| sine(angle.wrapping_add(16384));
    let [p, y, r] = rotation;
    let (sp, cp, sy, cy, sr, cr) = (sine(p), cosine(p), sine(y), cosine(y), sine(r), cosine(r));
    let axes = [
        [cp * cy, cp * sy, sp],
        [sr * sp * cy - cr * sy, sr * sp * sy + cr * cy, -sr * cp],
        [-(cr * sp * cy + sr * sy), sr * cy - cr * sp * sy, cr * cp],
    ];
    std::array::from_fn(|i| {
        location[i]
            + (0..3)
                .map(|j| (point[j] - pivot[j]) * scale[j] * axes[j][i])
                .sum::<f32>()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn serialized_mesh_versions_and_every_truncation() {
        for version in [133, 149, 155, 159] {
            let pkg = Package {
                summary: super::super::Summary {
                    version,
                    licensee_version: 1,
                    flags: 0,
                    name_count: 1,
                    name_offset: 0,
                    export_count: 0,
                    export_offset: 0,
                    import_count: 0,
                    import_offset: 0,
                },
                names: vec!["None".into()],
                imports: vec![],
                exports: vec![],
            };
            let mut data = vec![0; 42]; // None property + primitive box/sphere
            data.push(1); // section count
            data.extend_from_slice(&0u32.to_le_bytes());
            for n in [0u16, 0, 2, 1, 1] {
                data.extend_from_slice(&n.to_le_bytes());
            }
            data.extend_from_slice(&[0; 25]); // second box
            data.push(3); // vertices
            for position in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
                for f in position.into_iter().chain([0.0, 0.0, 1.0]) {
                    data.extend_from_slice(&f.to_le_bytes());
                }
            }
            data.extend_from_slice(&[0; 4]);
            if version >= 155 {
                data.extend_from_slice(&[0; 4]);
            }
            if version >= 149 {
                data.extend_from_slice(&[0; 4]);
            }
            for _ in 0..2 {
                data.extend_from_slice(&[0; 5]);
            } // empty color stream + revision
            data.extend_from_slice(&[1, 3]); // one UV stream, three vertices
            for f in [0.0f32, 0.0, 1.0, 0.0, 0.0, 1.0] {
                data.extend_from_slice(&f.to_le_bytes());
            }
            data.extend_from_slice(&[0; 8]);
            data.push(3);
            for n in [0u16, 1, 2] {
                data.extend_from_slice(&n.to_le_bytes());
            }
            data.extend_from_slice(&[0; 9]); // index revision, empty wire indices, revision
            let mut export = Export {
                class: 0,
                super_class: 0,
                outer: 0,
                extra: None,
                name: "test".into(),
                flags: 0,
                serial_size: data.len() as u32,
                serial_offset: 0,
            };
            let mesh = read_static_mesh(&pkg, &data, &export).unwrap();
            assert_eq!(mesh.triangles().unwrap().len(), 1);
            assert_eq!(mesh.uv_streams[0][2], [0.0, 1.0]);
            assert_eq!(mesh.unparsed_tail_bytes, 0);
            for len in 0..data.len() {
                export.serial_size = len as u32;
                assert!(
                    read_static_mesh(&pkg, &data[..len], &export).is_err(),
                    "version {version}, length {len}"
                );
            }
        }
    }
    #[test]
    fn actor_transform_uses_unreal_turns_and_pivot_before_scale() {
        let actual = transform_point(
            [2.0, 0.0, 0.0],
            [10.0, 20.0, 30.0],
            [0, 16384, 0],
            [3.0, 1.0, 1.0],
            [1.0, 0.0, 0.0],
        );
        for (a, b) in actual.into_iter().zip([10.0, 23.0, 30.0]) {
            assert!((a - b).abs() < 0.0001);
        }
        let pitch = transform_point([1.0, 0.0, 0.0], [0.0; 3], [16384, 0, 0], [1.0; 3], [0.0; 3]);
        assert!((pitch[2] - 1.0).abs() < 0.0001);
    }
    #[test]
    fn section_ranges_and_vertex_references_are_checked() {
        let mut mesh = StaticMesh {
            materials: vec![],
            bounds: [[0.0; 3]; 2],
            sections: vec![Section {
                flags: 0,
                first_index: 0,
                first_vertex: 0,
                last_vertex: 2,
                triangle_count: 1,
                face_count: 1,
            }],
            vertices: vec![Vertex {
                position: [0.0; 3],
                normal: [0.0; 3],
            }],
            uv_streams: vec![],
            indices: vec![0, 0, 0],
            wire_indices: vec![],
            geometry_bytes: 0,
            unparsed_tail_bytes: 0,
        };
        assert_eq!(mesh.triangles().unwrap().len(), 1);
        mesh.indices[2] = 1;
        assert!(mesh.triangles().is_err());
        mesh.indices.pop();
        assert!(mesh.triangles().is_err());
    }
}
