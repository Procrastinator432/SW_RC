//! Selected UKMeshProps serializer (engine.dll 10386d10 / 10386ad0).
//! Deliberately accepts only the one identity convex box used by Blockade03.
use crate::{properties, Export, Package, Reader};
#[derive(Debug, serde::Serialize)]
pub struct BoxProperties {
    pub inertia: [f64; 6],
    pub center: [f64; 3],
    pub bounds: [[f64; 3]; 2],
}
pub fn read_box(pkg: &Package, data: &[u8], export: &Export) -> Result<BoxProperties, String> {
    if !(157..=159).contains(&pkg.summary.version) || pkg.summary.licensee_version != 1 {
        return Err("Unsupported selected KMeshProps version".into());
    }
    let props = properties::read(pkg, data, export)?;
    parse(&pkg.payload(data, export)?[props.native_offset..])
}
fn parse(bytes: &[u8]) -> Result<BoxProperties, String> {
    let mut r = Reader::at(bytes, 0)?;
    let mut inertia = [0.; 6];
    for v in &mut inertia {
        *v = f64::from(r.f32()?);
    }
    let center = r.vector()?.map(f64::from);
    for _ in 0..3 {
        if r.index()? != 0 {
            return Err("Selected KMeshProps primitive is not convex-only".into());
        }
    }
    if r.index()? != 1 {
        return Err("Selected KMeshProps needs one convex element".into());
    }
    for i in 0..16 {
        if r.f32()? != if i % 5 == 0 { 1. } else { 0. } {
            return Err("Selected convex transform is not identity".into());
        }
    }
    if r.index()? != 8 {
        return Err("Selected convex is not an eight-vertex box".into());
    }
    let mut vertices = vec![];
    for _ in 0..8 {
        vertices.push(r.vector()?.map(f64::from));
    }
    if r.index()? != 0 || r.pos != bytes.len() {
        return Err("Unsupported selected KMeshProps tail".into());
    }
    let bounds: [[f64; 3]; 2] = std::array::from_fn(|side| {
        std::array::from_fn(|i| {
            vertices.iter().map(|v| v[i]).fold(
                if side == 0 {
                    f64::INFINITY
                } else {
                    f64::NEG_INFINITY
                },
                |a, b| if side == 0 { a.min(b) } else { a.max(b) },
            )
        })
    });
    let mut corners = std::collections::BTreeSet::new();
    for v in vertices {
        let mut mask = 0;
        for i in 0..3 {
            if v[i] == bounds[1][i] {
                mask |= 1 << i;
            } else if v[i] != bounds[0][i] {
                return Err("Convex vertex is not a box corner".into());
            }
        }
        corners.insert(mask);
    }
    if corners.len() != 8
        || inertia
            .iter()
            .chain(center.iter())
            .chain(bounds.iter().flatten())
            .any(|v| !v.is_finite())
        || [0, 3, 5].iter().any(|i| inertia[*i] <= 0.)
        || (0..3).any(|i| bounds[0][i] >= bounds[1][i])
    {
        return Err("Invalid convex mass properties".into());
    }
    Ok(BoxProperties {
        inertia,
        center,
        bounds,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut b = vec![];
        for v in [2_f32, 0., 0., 3., 0., 4., 0.1, 0.2, 0.3] {
            b.extend(v.to_le_bytes());
        }
        b.extend([0, 0, 0, 1]);
        for i in 0..16 {
            b.extend((if i % 5 == 0 { 1_f32 } else { 0. }).to_le_bytes());
        }
        b.push(8);
        for m in 0..8 {
            for i in 0..3 {
                b.extend((if m & (1 << i) == 0 { -1_f32 } else { 1. }).to_le_bytes());
            }
        }
        b.push(0);
        b
    }
    #[test]
    fn mass_shape_layout_truncation_and_unsupported_geometry() {
        let mut b = fixture();
        let p = parse(&b).unwrap();
        assert_eq!(p.inertia, [2., 0., 0., 3., 0., 4.]);
        assert_eq!(p.bounds, [[-1.; 3], [1.; 3]]);
        for end in 0..b.len() {
            assert!(parse(&b[..end]).is_err(), "{end}");
        }
        b[36] = 1;
        assert!(parse(&b).is_err());
        b = fixture();
        b[104] = 7;
        assert!(parse(&b).is_err());
        b = fixture();
        b.extend([0]);
        assert!(parse(&b).is_err());
    }
}
