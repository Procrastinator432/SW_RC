//! Persistent SWRC skeletal LOD archives (package 151..159/licensee1, mesh LOD 8).
//! Raw words remain raw when their render semantics have not yet been recovered.
use crate::{
    skeletal_mesh::read_skeletal_mesh_prefix, skeletal_skin::BindSkinVertex, Export, Package,
    Reader,
};
use serde::Serialize;
#[derive(Debug, Serialize)]
pub struct StoredAlias {
    pub names: [i32; 2],
    pub matrix: [u32; 16],
}
#[derive(Debug, Serialize)]
pub struct StoredInfluence {
    /// Raw IEEE-754 weight; no conversion or normalization during archive loading.
    pub weight: u32,
    pub vertex_index: u16,
    pub bone_index: u16,
}
#[derive(Debug, Serialize)]
pub struct StoredLod {
    pub offset: usize,
    pub end_offset: usize,
    pub version: u32,
    pub commands: Vec<u32>,
    pub bind_vertices: Vec<BindSkinVertex>,
    pub word_14: u32,
    pub sections: [Vec<[u16; 9]>; 2],
    pub indices: [Vec<u16>; 2],
    pub index_words: [u32; 2],
    pub stream_words: [u32; 3],
    pub stream_vertices: Vec<[u32; 8]>,
    pub lazy_offsets: [usize; 4],
    pub lazy_ends: [u32; 4],
    pub influences: Vec<StoredInfluence>,
    pub wedges: Vec<[u8; 10]>,
    pub faces: Vec<[u8; 8]>,
    pub points: Vec<[u32; 3]>,
    pub tail_words: [u32; 6],
}
#[derive(Debug, Serialize)]
pub struct SkeletalLods {
    pub start_offset: usize,
    pub end_offset: usize,
    pub remaining_bytes: usize,
    pub word_1bc: u32,
    pub material_links: Vec<(Vec<u16>, u32)>,
    pub word_pairs: Vec<[u16; 2]>,
    pub aliases: Vec<StoredAlias>,
    pub word_1c4: u32,
    pub lods: Vec<StoredLod>,
}
fn shorts<const N: usize>(r: &mut Reader<'_>) -> Result<[u16; N], String> {
    let mut out = [0; N];
    for word in &mut out {
        *word = u16::from_le_bytes(r.take(2)?.try_into().unwrap());
    }
    Ok(out)
}
fn words<const N: usize>(r: &mut Reader<'_>) -> Result<[u32; N], String> {
    let mut out = [0; N];
    for word in &mut out {
        *word = r.u32()?;
    }
    Ok(out)
}
fn array<T>(
    r: &mut Reader<'_>,
    stride: usize,
    mut read: impl FnMut(&mut Reader<'_>) -> Result<T, String>,
) -> Result<Vec<T>, String> {
    let n = r.count(stride)?;
    (0..n).map(|_| read(r)).collect()
}
fn lazy<const N: usize>(r: &mut Reader<'_>, base: usize) -> Result<(u32, Vec<[u8; N]>), String> {
    let end = r.u32()?;
    let entries = array(r, N, |r| Ok(r.take(N)?.try_into().unwrap()))?;
    if base.checked_add(r.pos) != Some(end as usize) {
        return Err("LOD lazy array end offset does not match decoded data".into());
    }
    Ok((end, entries))
}
fn lod(r: &mut Reader<'_>, base: usize) -> Result<StoredLod, String> {
    let offset = r.pos;
    let version = r.u32()?;
    if version != 1 {
        return Err("skeletal LOD model reader currently requires version 1".into());
    }
    let commands = array(r, 4, |r| r.u32())?;
    let bind_vertices = array(r, 16, |r| {
        Ok(BindSkinVertex {
            position: words(r)?,
            packed_normal: r.u32()?,
        })
    })?;
    let word_14 = r.u32()?;
    let sections = [array(r, 18, shorts)?, array(r, 18, shorts)?];
    // Persistent FRenderResource emits no raw payload, FSkinVertexStream omits UObject pointer.
    let i0 = array(r, 2, |r| Ok(shorts::<1>(r)?[0]))?;
    let w0 = r.u32()?;
    let i1 = array(r, 2, |r| Ok(shorts::<1>(r)?[0]))?;
    let w1 = r.u32()?;
    let stream_words = words(r)?;
    let stream_vertices = array(r, 32, words)?;
    let mut lazy_offsets = [0; 4];
    lazy_offsets[0] = r.pos;
    let (e0, inf) = lazy::<8>(r, base)?;
    let influences = inf
        .into_iter()
        .map(|a| StoredInfluence {
            weight: u32::from_le_bytes(a[..4].try_into().unwrap()),
            vertex_index: u16::from_le_bytes(a[4..6].try_into().unwrap()),
            bone_index: u16::from_le_bytes(a[6..8].try_into().unwrap()),
        })
        .collect();
    lazy_offsets[1] = r.pos;
    let (e1, wedges) = lazy::<10>(r, base)?;
    lazy_offsets[2] = r.pos;
    let (e2, faces) = lazy::<8>(r, base)?;
    lazy_offsets[3] = r.pos;
    let (e3, pts) = lazy::<12>(r, base)?;
    let points = pts
        .into_iter()
        .map(|a| {
            std::array::from_fn(|i| u32::from_le_bytes(a[i * 4..i * 4 + 4].try_into().unwrap()))
        })
        .collect();
    let tail_words = words(r)?;
    Ok(StoredLod {
        offset,
        end_offset: r.pos,
        version,
        commands,
        bind_vertices,
        word_14,
        sections,
        indices: [i0, i1],
        index_words: [w0, w1],
        stream_words,
        stream_vertices,
        lazy_offsets,
        lazy_ends: [e0, e1, e2, e3],
        influences,
        wedges,
        faces,
        points,
        tail_words,
    })
}
pub fn read_skeletal_lods(
    pkg: &Package,
    data: &[u8],
    export: &Export,
) -> Result<SkeletalLods, String> {
    let prefix = read_skeletal_mesh_prefix(pkg, data, export)?;
    if prefix.lod_version != 8 {
        return Err("skeletal LOD reader currently requires mesh archive version 8".into());
    }
    let payload = pkg.payload(data, export)?;
    let start_offset = prefix.end_offset;
    let mut r = Reader::at(payload, start_offset)?;
    let word_1bc = r.u32()?;
    let material_links = array(&mut r, 5, |r| {
        Ok((array(r, 2, |r| Ok(shorts::<1>(r)?[0]))?, r.u32()?))
    })?;
    let word_pairs = array(&mut r, 4, shorts)?;
    let aliases = array(&mut r, 66, |r| {
        let names = [r.index()?, r.index()?];
        for n in names {
            pkg.name(n)?;
        }
        Ok(StoredAlias {
            names,
            matrix: words(r)?,
        })
    })?;
    let word_1c4 = r.u32()?;
    let lods = array(&mut r, 1, |r| lod(r, export.serial_offset as usize))?;
    Ok(SkeletalLods {
        start_offset,
        end_offset: r.pos,
        remaining_bytes: payload.len() - r.pos,
        word_1bc,
        material_links,
        word_pairs,
        aliases,
        word_1c4,
        lods,
    })
}

/// Locate top-nibble-zero commands without pretending weighted commands are rigid.
/// Each output command is followed by two UV words. Copy commands consume no bind vertex.
#[derive(Debug, Serialize)]
pub struct RigidLodEntry {
    pub command_index: usize,
    pub bind_index: usize,
    pub output_index: usize,
}
#[derive(Debug, Serialize)]
pub struct LodProgram {
    pub rigid: Vec<RigidLodEntry>,
    pub consumed_bind: usize,
    pub outputs: usize,
    pub command_words: usize,
    pub kinds: [usize; 16],
}
pub fn inspect_skin_program(commands: &[u32], bind_count: usize) -> Result<LodProgram, String> {
    let (mut pc, mut bind, mut output) = (0, 0, 0);
    let mut rigid = vec![];
    let mut kinds = [0; 16];
    loop {
        let command = *commands
            .get(pc)
            .ok_or("LOD skin program has no terminator")?;
        if command == u32::MAX {
            return Ok(LodProgram {
                rigid,
                consumed_bind: bind,
                outputs: output,
                command_words: pc + 1,
                kinds,
            });
        }
        let kind = (command >> 28) as usize;
        kinds[kind] += 1;
        let count = if kind == 15 { 1 } else { (kind & 7) + 1 };
        let next = pc
            .checked_add(count + 2)
            .ok_or("skin command offset overflow")?;
        if next > commands.len() {
            return Err("truncated skin influences or UV words".into());
        }
        if kind != 15 {
            if bind >= bind_count {
                return Err("skin program exceeds bind vertex count".into());
            }
            if kind == 0 {
                rigid.push(RigidLodEntry {
                    command_index: pc,
                    bind_index: bind,
                    output_index: output,
                });
            }
            bind += 1;
        }
        pc = next;
        output += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut b = 1u32.to_le_bytes().to_vec();
        b.push(1); // one command word
        b.extend(u32::MAX.to_le_bytes());
        b.push(1); // one bind vertex, preserving non-finite raw bits
        for w in [0x80000000u32, 0x7fc01234, 0x7f800000, 0xfedcba98] {
            b.extend(w.to_le_bytes());
        }
        b.extend(14u32.to_le_bytes());
        for _ in 0..2 {
            b.extend([1]);
            for w in 0..9u16 {
                b.extend(w.to_le_bytes());
            }
        }
        for w in [28u32, 40] {
            b.push(1);
            b.extend(17u16.to_le_bytes());
            b.extend(w.to_le_bytes());
        }
        for w in [58u32, 59, 60] {
            b.extend(w.to_le_bytes());
        }
        b.push(1);
        for w in 0..8u32 {
            b.extend(w.to_le_bytes());
        }
        for n in [8usize, 10, 8, 12] {
            let end = 1000 + b.len() + 5 + n;
            b.extend((end as u32).to_le_bytes());
            b.push(1);
            b.extend((0..n).map(|i| i as u8));
        }
        for w in 0..6u32 {
            b.extend(w.to_le_bytes());
        }
        b
    }
    #[test]
    fn archive_compact_counts_serialized_strides_and_absolute_lazy_offsets() {
        let b = fixture();
        let v = lod(&mut Reader::at(&b, 0).unwrap(), 1000).unwrap();
        assert_eq!(v.end_offset, b.len());
        assert_eq!(
            v.bind_vertices[0].position,
            [0x80000000, 0x7fc01234, 0x7f800000]
        );
        assert_eq!(v.bind_vertices[0].packed_normal, 0xfedcba98);
        assert_eq!(v.sections[1][0], [0, 1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(v.indices, [vec![17], vec![17]]);
        assert_eq!(v.index_words, [28, 40]);
        assert_eq!(v.stream_words, [58, 59, 60]);
        assert_eq!(v.stream_vertices[0], [0, 1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(v.influences[0].weight, 0x03020100);
        assert_eq!(v.influences[0].vertex_index, 0x504);
        assert_eq!(v.influences[0].bone_index, 0x706);
        assert_eq!(v.wedges[0], [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
        assert_eq!(v.faces[0], [0, 1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(v.points[0], [0x03020100, 0x07060504, 0x0b0a0908]);
        assert_eq!(v.tail_words, [0, 1, 2, 3, 4, 5]);
    }
    #[test]
    fn archive_every_truncation_fails() {
        let b = fixture();
        for end in 0..b.len() {
            assert!(
                lod(&mut Reader::at(&b[..end], 0).unwrap(), 1000).is_err(),
                "{end}"
            );
        }
    }
    #[test]
    fn archive_wrong_absolute_base_and_lazy_end_fail() {
        let mut b = fixture();
        let v = lod(&mut Reader::at(&b, 0).unwrap(), 1000).unwrap();
        assert!(lod(&mut Reader::at(&b, 0).unwrap(), 999).is_err());
        for offset in v.lazy_offsets {
            let saved = b[offset];
            b[offset] ^= 1;
            assert!(lod(&mut Reader::at(&b, 0).unwrap(), 1000).is_err());
            b[offset] = saved;
        }
    }
    #[test]
    fn archive_unknown_model_version_rejected() {
        let mut b = fixture();
        b[0] = 2;
        assert!(lod(&mut Reader::at(&b, 0).unwrap(), 1000)
            .unwrap_err()
            .contains("version 1"));
    }
    #[test]
    fn archive_negative_compact_array_count_rejected() {
        let mut b = fixture();
        b[4] = 0x81;
        assert!(lod(&mut Reader::at(&b, 0).unwrap(), 1000).is_err());
    }
    #[test]
    fn program_uv_words_are_not_commands() {
        let p = inspect_skin_program(&[6, u32::MAX, 0xf0000000, u32::MAX], 1).unwrap();
        assert_eq!((p.consumed_bind, p.outputs, p.command_words), (1, 1, 4));
        assert_eq!(
            (
                p.rigid[0].command_index,
                p.rigid[0].bind_index,
                p.rigid[0].output_index
            ),
            (0, 0, 0)
        );
    }
    #[test]
    fn program_all_nibbles_influence_counts_and_copy_bind_consumption() {
        let mut commands = vec![];
        for kind in 0..16u32 {
            let count = if kind == 15 {
                1
            } else {
                (kind as usize & 7) + 1
            };
            commands.push(kind << 28);
            commands.extend(std::iter::repeat_n(0xffffffff, count - 1));
            commands.extend([0xffffffff, 0xffffffff]);
        }
        commands.push(u32::MAX);
        let p = inspect_skin_program(&commands, 15).unwrap();
        assert_eq!(p.kinds, [1; 16]);
        assert_eq!(
            (p.consumed_bind, p.outputs, p.command_words),
            (15, 16, commands.len())
        );
        assert_eq!(p.rigid.len(), 1);
    }
    #[test]
    fn program_copy_then_rigid_has_distinct_output_and_bind_index() {
        let p = inspect_skin_program(&[0xf0000000, 0, 0, 12, 0, 0, u32::MAX], 1).unwrap();
        assert_eq!(
            (
                p.rigid[0].command_index,
                p.rigid[0].bind_index,
                p.rigid[0].output_index
            ),
            (3, 0, 1)
        );
        assert_eq!((p.consumed_bind, p.outputs), (1, 2));
    }
    #[test]
    fn program_terminator_stops_before_trailing_data() {
        let p = inspect_skin_program(&[u32::MAX, 0, 0, 0], 0).unwrap();
        assert_eq!((p.command_words, p.outputs, p.consumed_bind), (1, 0, 0));
    }
    #[test]
    fn program_missing_terminator_and_empty_fail() {
        for commands in [&[][..], &[0, 0, 0][..]] {
            assert!(inspect_skin_program(commands, 1)
                .unwrap_err()
                .contains("no terminator"));
        }
    }
    #[test]
    fn program_truncated_influences_or_uv_fail() {
        for commands in [&[0][..], &[0, 0][..], &[0x70000000, 0, 0, 0, 0][..]] {
            assert!(inspect_skin_program(commands, 1)
                .unwrap_err()
                .contains("truncated"));
        }
    }
    #[test]
    fn program_bind_overrun_fails_but_cached_rigid_is_counted() {
        assert!(inspect_skin_program(&[0, 0, 0, u32::MAX], 0).is_err());
        let p = inspect_skin_program(&[0x80000000, 0, 0, u32::MAX], 1).unwrap();
        assert_eq!(p.kinds[8], 1);
        assert!(p.rigid.is_empty());
        assert_eq!(p.consumed_bind, 1);
    }
}
