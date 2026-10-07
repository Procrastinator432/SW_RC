//! Original UMeshAnimation/FSkelAnimSeq/AnalogTrack archive format, SWRC 151..159.
use crate::{properties, skeletal_track::AnimationTrack, Export, Package, Reader, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AnimationName {
    pub index: i32,
    pub name: String,
}
#[derive(Debug, Serialize)]
pub struct AnimationReferenceBone {
    pub name: AnimationName,
    pub word_4: u32,
    pub word_8: u32,
}
#[derive(Debug, Serialize)]
pub struct AnimationNotify {
    pub time_bits: u32,
    pub name: AnimationName,
    pub object_index: i32,
}
#[derive(Debug, Serialize)]
pub struct StoredSequence {
    pub payload_offset: usize,
    pub end_offset: usize,
    pub word_2c: u32,
    pub name: AnimationName,
    pub groups: Vec<AnimationName>,
    pub first_frame: i32,
    pub frames: i32,
    pub notifies: Vec<AnimationNotify>,
    pub rate_bits: u32,
    pub word_18: u32,
    pub minimum_blend_bits: u32,
    pub randomize_start: u8,
    pub word_30: u32,
    pub word_50: u32,
    pub flags: u32,
    pub word_58: u32,
    pub tracks: Vec<AnimationTrack>,
}
#[derive(Debug, Serialize)]
pub struct MeshAnimation {
    pub word_28: u32,
    pub reference_bones: Vec<AnimationReferenceBone>,
    pub sequences: Vec<StoredSequence>,
    pub native_offset: usize,
    pub end_offset: usize,
    pub unparsed_tail_bytes: usize,
}
fn name(pkg: &Package, r: &mut Reader<'_>) -> Result<AnimationName> {
    let index = r.index()?;
    Ok(AnimationName {
        index,
        name: pkg.name(index)?.to_owned(),
    })
}
fn word(r: &mut Reader<'_>) -> Result<u16> {
    Ok(u16::from_le_bytes(r.take(2)?.try_into().unwrap()))
}
fn track(r: &mut Reader<'_>) -> Result<AnimationTrack> {
    let position_scale_bits = r.u32()?;
    let n = r.count(6)?;
    let positions = (0..n)
        .map(|_| Ok([word(r)? as i16, word(r)? as i16, word(r)? as i16]))
        .collect::<Result<Vec<_>>>()?;
    let n = r.count(6)?;
    let rotations = (0..n)
        .map(|_| Ok([word(r)?, word(r)?, word(r)?]))
        .collect::<Result<Vec<_>>>()?;
    let n = r.count(1)?;
    let durations = r.take(n)?.to_vec();
    Ok(AnimationTrack {
        rotation_count_word: rotations.len() as u32,
        position_count_word: positions.len() as u32,
        duration_count_word: durations.len() as u32,
        position_scale_bits,
        rotations,
        positions,
        durations,
    })
}
fn sequence(pkg: &Package, r: &mut Reader<'_>) -> Result<StoredSequence> {
    let payload_offset = r.pos;
    let word_2c = r.u32()?;
    let name = name(pkg, r)?;
    let n = r.count(1)?;
    let groups = (0..n)
        .map(|_| self::name(pkg, r))
        .collect::<Result<Vec<_>>>()?;
    let first_frame = r.i32()?;
    let frames = r.i32()?;
    let n = r.count(6)?;
    let notifies = (0..n)
        .map(|_| {
            let time_bits = r.u32()?;
            let name = self::name(pkg, r)?;
            let object_index = r.index()?;
            pkg.object_path(object_index)?;
            Ok(AnimationNotify {
                time_bits,
                name,
                object_index,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let rate_bits = r.u32()?;
    let word_18 = r.u32()?;
    let minimum_blend_bits = r.u32()?;
    let randomize_start = r.byte()?;
    let word_30 = r.u32()?;
    let word_50 = r.u32()?;
    let flags = r.u32()?;
    let word_58 = r.u32()?;
    let n = r.count(7)?;
    let tracks = (0..n).map(|_| track(r)).collect::<Result<Vec<_>>>()?;
    Ok(StoredSequence {
        payload_offset,
        end_offset: r.pos,
        word_2c,
        name,
        groups,
        first_frame,
        frames,
        notifies,
        rate_bits,
        word_18,
        minimum_blend_bits,
        randomize_start,
        word_30,
        word_50,
        flags,
        word_58,
        tracks,
    })
}
pub fn read_mesh_animation(pkg: &Package, data: &[u8], export: &Export) -> Result<MeshAnimation> {
    if !(151..=159).contains(&pkg.summary.version) || pkg.summary.licensee_version != 1 {
        return Err("MeshAnimation supports SWRC 151–159 / licensee 1; earlier rotation conversion is unresolved".into());
    }
    if pkg.object_path(export.class)? != "Engine.MeshAnimation" {
        return Err("expected Engine.MeshAnimation export".into());
    }
    let payload = pkg.payload(data, export)?;
    let native_offset = properties::read(pkg, data, export)?.native_offset;
    let mut r = Reader::at(payload, native_offset)?;
    let word_28 = r.u32()?;
    let n = r.count(9)?;
    let reference_bones = (0..n)
        .map(|_| {
            Ok(AnimationReferenceBone {
                name: name(pkg, &mut r)?,
                word_4: r.u32()?,
                word_8: r.u32()?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let n = r.count(45)?;
    let sequences = (0..n)
        .map(|_| sequence(pkg, &mut r))
        .collect::<Result<Vec<_>>>()?;
    Ok(MeshAnimation {
        word_28,
        reference_bones,
        sequences,
        native_offset,
        end_offset: r.pos,
        unparsed_tail_bytes: payload.len() - r.pos,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn package() -> Package {
        Package {
            summary: crate::Summary {
                version: 159,
                licensee_version: 1,
                flags: 0,
                name_count: 4,
                name_offset: 0,
                export_count: 0,
                export_offset: 0,
                import_count: 0,
                import_offset: 0,
            },
            names: vec!["None".into(), "Clip".into(), "Bone".into(), "Notify".into()],
            imports: vec![],
            exports: vec![],
        }
    }
    fn sequence_bytes() -> Vec<u8> {
        let mut b = 7u32.to_le_bytes().to_vec();
        b.extend([1, 1, 2]);
        b.extend(0i32.to_le_bytes());
        b.extend(10i32.to_le_bytes());
        b.push(1);
        b.extend(0.25f32.to_le_bytes());
        b.extend([3, 0]);
        for v in [30f32.to_bits(), 0, 0.1f32.to_bits()] {
            b.extend(v.to_le_bytes());
        }
        b.push(1);
        for v in [1f32.to_bits(), 123, 1, 456] {
            b.extend(v.to_le_bytes());
        }
        b.push(0);
        b
    }
    #[test]
    fn sequence_names_metadata_notifies_and_all_truncations() {
        let pkg = package();
        let b = sequence_bytes();
        let s = sequence(&pkg, &mut Reader::at(&b, 0).unwrap()).unwrap();
        assert_eq!(s.name.name, "Clip");
        assert_eq!(s.groups[0].name, "Bone");
        assert_eq!(s.frames, 10);
        assert_eq!(s.notifies[0].name.name, "Notify");
        assert_eq!(s.notifies[0].object_index, 0);
        assert_eq!((s.word_50, s.flags, s.word_58), (123, 1, 456));
        assert_eq!(s.end_offset, b.len());
        for end in 0..b.len() {
            assert!(
                sequence(&pkg, &mut Reader::at(&b[..end], 0).unwrap()).is_err(),
                "{end}"
            );
        }
        let mut bad = b.clone();
        bad[4] = 63;
        assert!(sequence(&pkg, &mut Reader::at(&bad, 0).unwrap()).is_err());
        bad = b;
        bad[21] = 1;
        assert!(sequence(&pkg, &mut Reader::at(&bad, 0).unwrap()).is_err());
    }
    #[test]
    fn track_archive_positions_precede_rotations_and_preserve_bits() {
        let mut b = 0x80000000u32.to_le_bytes().to_vec();
        b.push(1);
        for v in [-32768i16, 32767, -1] {
            b.extend(v.to_le_bytes());
        }
        b.push(1);
        for v in [65535u16, 1, 2] {
            b.extend(v.to_le_bytes());
        }
        b.extend([2, 0, 255]);
        let mut r = Reader::at(&b, 0).unwrap();
        let t = track(&mut r).unwrap();
        assert_eq!(r.pos, b.len());
        assert_eq!(t.positions, [[-32768, 32767, -1]]);
        assert_eq!(t.rotations, [[65535, 1, 2]]);
        assert_eq!(t.position_scale_bits, 0x80000000);
        assert_eq!(t.durations, [0, 255]);
    }
    #[test]
    fn track_all_truncations_and_negative_counts_fail() {
        let mut b = 1f32.to_le_bytes().to_vec();
        b.push(1);
        b.extend([0; 6]);
        b.push(1);
        b.extend([0; 6]);
        b.extend([2, 2, 3]);
        for end in 0..b.len() {
            assert!(
                track(&mut Reader::at(&b[..end], 0).unwrap()).is_err(),
                "{end}"
            );
        }
        let mut b = vec![0; 4];
        b.push(0x81);
        assert!(track(&mut Reader::at(&b, 0).unwrap()).is_err());
    }
    #[test]
    fn empty_track_arrays_are_parsed_without_invented_keys() {
        let b = [0, 0, 0, 0, 0, 0, 0];
        let t = track(&mut Reader::at(&b, 0).unwrap()).unwrap();
        assert!(t.rotations.is_empty() && t.positions.is_empty() && t.durations.is_empty());
    }
}
