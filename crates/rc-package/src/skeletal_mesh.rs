//! SkeletalMesh archive prefix through reference bones and stored animation linkups.
//! LOD geometry is bounded/skipped; remaining render/skin data stays explicit.
use crate::{properties, skeletal_animation::AnimationName, Export, Package, Reader, Result};
use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize)]
pub struct StoredBone {
    pub name: AnimationName,
    pub flags: u32,
    pub rotation: [u32; 4],
    pub position: [u32; 3],
    pub word_24: u32,
    pub word_28_first: u32,
    pub word_28: u32,
    pub word_30: u32,
    pub word_38: i32,
    pub word_34: i32,
}
#[derive(Debug, Serialize)]
pub struct StoredLinkup {
    pub word_0: u32,
    pub animation_index: i32,
    pub mapping: Vec<i32>,
}
#[derive(Debug, Serialize)]
pub struct SkeletalMeshPrefix {
    pub native_offset: usize,
    pub bones_offset: usize,
    pub linkups_offset: usize,
    pub end_offset: usize,
    pub remaining_bytes: usize,
    pub lod_version: i32,
    pub materials_offset: usize,
    pub materials: Vec<i32>,
    pub points_offset: usize,
    pub points: Vec<[u32; 3]>,
    pub bones: Vec<StoredBone>,
    pub linkups: Vec<StoredLinkup>,
}
fn array(r: &mut Reader<'_>, stride: usize) -> Result<()> {
    let n = r.count(stride)?;
    r.take(n * stride)?;
    Ok(())
}
fn reference(pkg: &Package, r: &mut Reader<'_>) -> Result<i32> {
    let index = r.index()?;
    pkg.object_path(index)?;
    Ok(index)
}
fn points(r: &mut Reader<'_>) -> Result<Vec<[u32; 3]>> {
    let n = r.count(12)?;
    (0..n).map(|_| Ok([r.u32()?, r.u32()?, r.u32()?])).collect()
}
fn bone(pkg: &Package, r: &mut Reader<'_>) -> Result<StoredBone> {
    let index = r.index()?;
    let name = AnimationName {
        index,
        name: pkg.name(index)?.to_owned(),
    };
    let flags = r.u32()?;
    let rotation = [r.u32()?, r.u32()?, r.u32()?, r.u32()?];
    let position = [r.u32()?, r.u32()?, r.u32()?];
    Ok(StoredBone {
        name,
        flags,
        rotation,
        position,
        word_24: r.u32()?,
        word_28_first: r.u32()?,
        word_28: r.u32()?,
        word_30: r.u32()?,
        word_38: r.i32()?,
        word_34: r.i32()?,
    })
}
pub fn read_skeletal_mesh_prefix(
    pkg: &Package,
    data: &[u8],
    e: &Export,
) -> Result<SkeletalMeshPrefix> {
    if !(151..=159).contains(&pkg.summary.version) || pkg.summary.licensee_version != 1 {
        return Err("SkeletalMesh prefix supports SWRC 151–159/licensee1".into());
    }
    if pkg.object_path(e.class)? != "Engine.SkeletalMesh" {
        return Err("expected Engine.SkeletalMesh".into());
    }
    let payload = pkg.payload(data, e)?;
    let native_offset = properties::read(pkg, data, e)?.native_offset;
    let mut r = Reader::at(payload, native_offset)?;
    r.take(25 + 16)?; // UPrimitive: box (two vectors+byte), sphere (vector+radius)
                      // Persistent package archives omit transient UMesh+54 (ArIsPersistent at +15).
    let lod_version = r.i32()?;
    if !(5..=8).contains(&lod_version) {
        return Err(format!("unsupported LOD archive version {lod_version}"));
    }
    r.u32()?;
    array(&mut r, 4)?; // +5c and packed vertices
    let materials_offset = r.pos;
    let n = r.count(1)?;
    let materials = (0..n)
        .map(|_| reference(pkg, &mut r))
        .collect::<Result<Vec<_>>>()?;
    r.take(36)?; // scale/origin/rotator
    for stride in [2, 8, 2, 10, 8] {
        array(&mut r, stride)?;
    }
    r.take(24)?;
    r.u32()?;
    reference(pkg, &mut r)?;
    r.take(52)?; // six words, +e4 and attachment
    r.u32()?; // LOD version>3: +120
    if lod_version > 6 {
        r.take(16)?;
    }
    let points_offset = r.pos;
    let points = points(&mut r)?;
    let bones_offset = r.pos;
    let n = r.count(57)?;
    let bones = (0..n)
        .map(|_| bone(pkg, &mut r))
        .collect::<Result<Vec<_>>>()?;
    let linkups_offset = r.pos;
    let n = r.count(if pkg.summary.version >= 152 { 5 } else { 1 })?;
    let mut linkups = Vec::with_capacity(n);
    for _ in 0..n {
        let word_0 = if pkg.summary.version >= 152 {
            r.u32()?
        } else {
            1
        };
        let animation_index = reference(pkg, &mut r)?;
        // Persistent archives omit the transient cached bone-to-track array.
        let mapping = Vec::new();
        linkups.push(StoredLinkup {
            word_0,
            animation_index,
            mapping,
        });
    }
    Ok(SkeletalMeshPrefix {
        native_offset,
        bones_offset,
        linkups_offset,
        end_offset: r.pos,
        remaining_bytes: payload.len() - r.pos,
        lod_version,
        materials_offset,
        materials,
        points_offset,
        points,
        bones,
        linkups,
    })
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum LinkupRefresh {
    SameLength,
    MissingAnimation,
    Rebuilt { matched: usize, warning: bool },
}
/// Native 10509430: cache validity tests ONLY length; missing names map to -1.
pub fn refresh_linkup<T: Eq>(
    mapping: &mut Vec<i32>,
    mesh_names: &[T],
    animation_names: Option<&[T]>,
) -> LinkupRefresh {
    if mapping.len() == mesh_names.len() {
        return LinkupRefresh::SameLength;
    }
    let Some(animation_names) = animation_names else {
        return LinkupRefresh::MissingAnimation;
    };
    *mapping = mesh_names
        .iter()
        .map(|name| {
            animation_names
                .iter()
                .position(|n| n == name)
                .map_or(-1, |i| i as i32)
        })
        .collect();
    let matched = mapping.iter().filter(|&&i| i >= 0).count();
    LinkupRefresh::Rebuilt {
        matched,
        warning: matched == 0,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn points_preserve_raw_float_words_and_stop_at_array_end() {
        let mut bytes = vec![2];
        for word in [
            0u32, 0x80000000, 0x7fc12345, 0x3f800000, 0x7f800000, 0xff800000,
        ] {
            bytes.extend(word.to_le_bytes());
        }
        bytes.push(99);
        let mut r = Reader::at(&bytes, 0).unwrap();
        assert_eq!(
            points(&mut r).unwrap(),
            [
                [0, 0x80000000, 0x7fc12345],
                [0x3f800000, 0x7f800000, 0xff800000]
            ]
        );
        assert_eq!(r.pos, 25);
    }
    #[test]
    fn points_truncations_and_negative_count_fail() {
        let bytes = [1u8; 13];
        for end in 0..13 {
            assert!(points(&mut Reader::at(&bytes[..end], 0).unwrap()).is_err());
        }
        assert!(points(&mut Reader::at(&[0x81], 0).unwrap()).is_err());
    }
    #[test]
    fn empty_points_do_not_consume_following_data() {
        let mut r = Reader::at(&[0, 99], 0).unwrap();
        assert!(points(&mut r).unwrap().is_empty());
        assert_eq!(r.pos, 1);
    }
    fn fixture() -> (Package, Vec<u8>) {
        let pkg = Package {
            summary: crate::Summary {
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
            names: vec!["Root".into()],
            imports: vec![],
            exports: vec![],
        };
        let mut b = vec![0];
        for word in 1u32..=14 {
            b.extend(word.to_le_bytes());
        }
        (pkg, b)
    }
    #[test]
    fn bone_archive_repeated_field_and_tail_order() {
        let (pkg, b) = fixture();
        let mut r = Reader::at(&b, 0).unwrap();
        let v = bone(&pkg, &mut r).unwrap();
        assert_eq!(r.pos, 57);
        assert_eq!(v.rotation, [2, 3, 4, 5]);
        assert_eq!(v.position, [6, 7, 8]);
        assert_eq!(
            (v.word_24, v.word_28_first, v.word_28, v.word_30),
            (9, 10, 11, 12)
        );
        assert_eq!((v.word_38, v.word_34), (13, 14));
    }
    #[test]
    fn bone_archive_truncations_and_bad_name_fail() {
        let (pkg, b) = fixture();
        for end in 0..b.len() {
            assert!(bone(&pkg, &mut Reader::at(&b[..end], 0).unwrap()).is_err());
        }
        let mut b = b;
        b[0] = 1;
        assert!(bone(&pkg, &mut Reader::at(&b, 0).unwrap()).is_err());
    }
    #[test]
    fn refresh_first_match_missing_names_and_empty_warning() {
        let mut m = vec![];
        assert_eq!(
            refresh_linkup(&mut m, &[3, 1, 9], Some(&[1, 3, 3])),
            LinkupRefresh::Rebuilt {
                matched: 2,
                warning: false
            }
        );
        assert_eq!(m, [1, 0, -1]);
        assert_eq!(
            refresh_linkup(&mut m, &[] as &[i32], Some(&[])),
            LinkupRefresh::Rebuilt {
                matched: 0,
                warning: true
            }
        );
        assert!(m.is_empty());
    }
    #[test]
    fn same_length_does_not_revalidate_names_or_animation() {
        let mut m = vec![99, -8];
        assert_eq!(
            refresh_linkup(&mut m, &[1, 2], Some(&[7, 8])),
            LinkupRefresh::SameLength
        );
        assert_eq!(m, [99, -8]);
        assert_eq!(
            refresh_linkup(&mut m, &[1, 2], None),
            LinkupRefresh::SameLength
        );
        assert_eq!(
            refresh_linkup(&mut m, &[1, 2, 3], None),
            LinkupRefresh::MissingAnimation
        );
        assert_eq!(m, [99, -8]);
    }
    #[test]
    fn no_matches_preserves_minus_one_map_and_warns() {
        let mut m = vec![];
        assert_eq!(
            refresh_linkup(&mut m, &[0, 1], Some(&[2, 3])),
            LinkupRefresh::Rebuilt {
                matched: 0,
                warning: true
            }
        );
        assert_eq!(m, [-1, -1]);
    }
}
