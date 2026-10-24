//! Selected port visibility mask for original arm/hand skin geometry.
//! All influencing bones must belong to an arm; head/torso are never retained.
use crate::visual::Mesh;
pub fn triangle_mask(mesh: &Mesh) -> Result<Vec<bool>, String> {
    let h = rc_package::skeletal_hierarchy::prepare_hierarchy(&mesh.bones)?;
    let roots = ["shoulder_L", "shoulder_R"].map(|name| {
        mesh.bones
            .iter()
            .position(|b| b.name.name.eq_ignore_ascii_case(name))
            .ok_or("Missing arm root")
    });
    let roots = [roots[0]?, roots[1]?];
    let selected: Vec<_> = (0..mesh.bones.len())
        .map(|mut i| {
            for _ in 0..mesh.bones.len() {
                if roots.contains(&i) {
                    return true;
                }
                let p = h.parents[i];
                if p < 0 || p as usize == i {
                    break;
                }
                i = p as usize;
            }
            false
        })
        .collect();
    let outputs = skin_mask(&mesh.lod.commands, &selected)?;
    let mut mask = vec![];
    for section in rc_package::skeletal_draw::draw_sections(&mesh.lod) {
        let end = section.first_index + section.triangle_count * 3;
        let indices = mesh.lod.indices[section.bank]
            .get(section.first_index..end)
            .ok_or("Arm mask section outside indices")?;
        for tri in indices.chunks_exact(3) {
            let keep = if let Some(bone) = section.bone {
                *selected
                    .get(bone as usize)
                    .ok_or("Arm rigid bone outside skeleton")?
            } else {
                tri.iter().try_fold(true, |yes, i| {
                    outputs
                        .get(*i as usize)
                        .map(|v| yes && *v)
                        .ok_or("Arm soft index outside stream")
                })?
            };
            mask.push(keep);
        }
    }
    if !mask.iter().any(|v| *v) || mask.iter().all(|v| *v) {
        return Err("Arm mask does not isolate partial geometry".into());
    }
    Ok(mask)
}

fn skin_mask(commands: &[u32], selected: &[bool]) -> Result<Vec<bool>, String> {
    let mut outputs = vec![];
    let mut cache = vec![];
    let mut pc = 0;
    loop {
        let command = *commands.get(pc).ok_or("Arm mask lacks skin terminator")?;
        if command == u32::MAX {
            break;
        }
        let kind = (command >> 28) as usize;
        let count = if kind == 15 { 1 } else { (kind & 7) + 1 };
        let words = commands
            .get(pc..pc + count + 2)
            .ok_or("Truncated arm mask skin record")?;
        let visible = if kind == 15 {
            let offset = ((command & 0x0fffffff) / 3) as usize * 3;
            if !offset.is_multiple_of(6) {
                return Err("Unaligned arm mask copy".into());
            }
            *cache.get(offset / 6).ok_or("Arm mask copy outside cache")?
        } else {
            let mut all = true;
            for word in &words[..count] {
                all &= *selected
                    .get(((word & 0xfff) / 6) as usize)
                    .ok_or("Arm mask bone outside skeleton")?;
            }
            all
        };
        if kind != 15 && kind & 8 != 0 {
            cache.push(visible);
        }
        outputs.push(visible);
        pc += count + 2;
    }
    Ok(outputs)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixed_head_arm_influences_never_leak_into_visible_geometry() {
        assert_eq!(
            skin_mask(&[0x10000000, 6, 0, 0, u32::MAX], &[true, false]).unwrap(),
            [false]
        );
        assert_eq!(
            skin_mask(&[0x10000000, 6, 0, 0, u32::MAX], &[true, true]).unwrap(),
            [true]
        );
    }
    #[test]
    fn cached_vertex_copies_keep_the_same_visibility() {
        let commands = [0x80000000, 0, 0, 0xf0000000, 0, 0, u32::MAX];
        assert_eq!(skin_mask(&commands, &[true]).unwrap(), [true, true]);
        assert_eq!(skin_mask(&commands, &[false]).unwrap(), [false, false]);
    }
    #[test]
    fn malformed_records_copies_and_bones_are_rejected() {
        for commands in [
            &[][..],
            &[0][..],
            &[0xf0000000, 0, 0, u32::MAX][..],
            &[6, 0, 0, u32::MAX][..],
        ] {
            assert!(skin_mask(commands, &[true]).is_err());
        }
    }
}
