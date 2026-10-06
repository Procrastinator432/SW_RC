//! BSP geometry prefix, reconstructed from the original engine's serializers.
//! Separate collision/hull readers interpret the collision tail; lighting and visibility remain opaque.
use super::{properties, Export, Package, Reader, Result};
use serde::Serialize;
#[derive(Debug, Serialize)]
pub struct Node {
    pub plane: [f32; 4],
    pub vertex_start: i32,
    pub surface: i32,
    pub back: i32,
    pub front: i32,
    pub coplanar: i32,
    pub collision_bound: i32,
    pub vertex_count: u8,
    pub flags: u8,
}
#[derive(Debug, Serialize)]
pub struct Surface {
    pub material: i32,
    pub material_path: String,
    pub flags: u32,
    pub base: i32,
    pub normal: i32,
    pub texture_u: i32,
    pub texture_v: i32,
}
#[derive(Debug, Serialize)]
pub struct Bsp {
    pub vectors: Vec<[f32; 3]>,
    pub points: Vec<[f32; 3]>,
    pub nodes: Vec<Node>,
    pub surfaces: Vec<Surface>,
    pub vertices: Vec<[i32; 2]>,
    pub geometry_bytes: usize,
    pub unparsed_tail_bytes: usize,
}
#[derive(Clone, Debug, Serialize)]
pub struct Triangle {
    pub points: [[f32; 3]; 3],
    pub surface: usize,
}
impl Bsp {
    pub fn triangles(&self) -> Result<Vec<Triangle>> {
        let mut triangles = Vec::new();
        for n in &self.nodes {
            if n.vertex_count < 3 {
                continue;
            }
            let start =
                usize::try_from(n.vertex_start).map_err(|_| "negative vertex pool reference")?;
            let end = start
                .checked_add(n.vertex_count as usize)
                .ok_or("vertex range overflow")?;
            let vertices = self
                .vertices
                .get(start..end)
                .ok_or("node outside vertex pool")?;
            let surface = usize::try_from(n.surface).map_err(|_| "negative surface index")?;
            if surface >= self.surfaces.len() {
                return Err("node outside surface table".into());
            }
            let point = |i: i32| -> Result<[f32; 3]> {
                usize::try_from(i)
                    .ok()
                    .and_then(|j| self.points.get(j))
                    .copied()
                    .ok_or_else(|| "vertex outside point table".into())
            };
            let first = point(vertices[0][0])?;
            for j in 1..vertices.len() - 1 {
                triangles.push(Triangle {
                    points: [first, point(vertices[j][0])?, point(vertices[j + 1][0])?],
                    surface,
                });
            }
        }
        Ok(triangles)
    }
}
fn vectors(r: &mut Reader<'_>) -> Result<Vec<[f32; 3]>> {
    let n = r.count(12)?;
    (0..n).map(|_| r.vector()).collect()
}
fn plane(r: &mut Reader<'_>) -> Result<[f32; 4]> {
    Ok([r.f32()?, r.f32()?, r.f32()?, r.f32()?])
}
pub fn read_bsp(pkg: &Package, data: &[u8], e: &Export) -> Result<Bsp> {
    if pkg.summary.version < 151 {
        return Err("BSP supported for SWRC version 151+ only".into());
    }
    let payload = pkg.payload(data, e)?;
    let props = properties::read(pkg, data, e)?;
    let mut r = Reader::at(payload, props.native_offset)?;
    r.vector()?;
    r.vector()?;
    r.byte()?;
    r.vector()?;
    r.f32()?; // UPrimitive bounds
    let vectors = vectors(&mut r)?;
    let points = self::vectors(&mut r)?;
    let count = r.count(71)?;
    let mut nodes = Vec::new();
    for _ in 0..count {
        let plane = plane(&mut r)?;
        r.take(8)?;
        let flags = r.byte()?;
        let vertex_start = r.index()?;
        let surface = r.index()?;
        let back = r.index()?;
        let front = r.index()?;
        let coplanar = r.index()?;
        let collision_bound = r.index()?;
        r.index()?;
        r.vector()?;
        r.f32()?;
        r.take(2)?;
        let vertex_count = r.byte()?;
        r.take(20)?; // two leaf indices, section, first vertex, lightmap index
        nodes.push(Node {
            plane,
            vertex_start,
            surface,
            back,
            front,
            coplanar,
            collision_bound,
            vertex_count,
            flags,
        });
    }
    let count = r.count(32)?;
    let mut surfaces = Vec::new();
    for _ in 0..count {
        let material = r.index()?;
        let flags = r.u32()?;
        let base = r.index()?;
        let normal = r.index()?;
        let texture_u = r.index()?;
        let texture_v = r.index()?;
        r.index()?;
        r.index()?;
        plane(&mut r)?;
        r.f32()?;
        surfaces.push(Surface {
            material,
            material_path: pkg.object_path(material)?,
            flags,
            base,
            normal,
            texture_u,
            texture_v,
        });
    }
    let count = r.count(2)?;
    let mut vertices = Vec::new();
    for _ in 0..count {
        vertices.push([r.index()?, r.index()?]);
    }
    let bsp = Bsp {
        vectors,
        points,
        nodes,
        surfaces,
        vertices,
        geometry_bytes: r.pos,
        unparsed_tail_bytes: payload.len() - r.pos,
    };
    bsp.triangles()?;
    Ok(bsp)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_triangle_and_every_payload_truncation() {
        let pkg = Package {
            summary: super::super::Summary {
                version: 159,
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
        let mut data = vec![0; 42];
        data.extend_from_slice(&[0, 3]);
        for f in [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
            data.extend_from_slice(&f.to_le_bytes());
        }
        data.push(1);
        data.extend_from_slice(&[0; 25]);
        data.extend_from_slice(&[0, 0, 0x81, 0x81, 0x81, 0x81, 0x81]);
        data.extend_from_slice(&[0; 18]);
        data.push(3);
        data.extend_from_slice(&[0; 20]);
        data.push(1);
        data.extend_from_slice(&[0; 31]);
        data.extend_from_slice(&[3, 0, 0x81, 1, 0x81, 2, 0x81]);
        let mut e = Export {
            class: 0,
            super_class: 0,
            outer: 0,
            extra: None,
            name: "Model".into(),
            flags: 0,
            serial_size: data.len() as u32,
            serial_offset: 0,
        };
        let bsp = read_bsp(&pkg, &data, &e).unwrap();
        assert_eq!(bsp.unparsed_tail_bytes, 0);
        assert_eq!(bsp.triangles().unwrap().len(), 1);
        for n in 0..data.len() {
            e.serial_size = n as u32;
            assert!(
                read_bsp(&pkg, &data[..n], &e).is_err(),
                "accepted truncation {n}"
            );
        }
    }
}
