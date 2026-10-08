//! ComputeSkinVerts palette, weighted kernels and persistent LOD command execution.
use crate::skeletal_skin_product::skin_product;
use crate::skeletal_weighted_kernels::{
    cached_rigid, cached_two, weighted_first, weighted_next, weighted_two,
};
use serde::Serialize;

/// Native global palette is resized first; prior successful bone writes survive errors.
/// Matrices are inverse-reference * pose, transposed for the vertex kernel.
pub fn build_skin_palette(
    poses: &[[u32; 16]],
    inverse: &[[u32; 16]],
    output: &mut Vec<[u32; 16]>,
) -> Result<(), String> {
    output.resize(poses.len(), [0; 16]);
    for (i, &pose) in poses.iter().enumerate() {
        let reference = *inverse
            .get(i)
            .ok_or("skin inverse-reference cache is shorter than pose")?;
        let product = skin_product(reference, pose);
        output[i] = std::array::from_fn(|j| product[(j % 4) * 4 + j / 4]);
    }
    Ok(())
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub struct BindSkinVertex {
    pub position: [u32; 3],
    pub packed_normal: u32,
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq, Default)]
pub struct SkinVertex {
    pub position: [u32; 3],
    pub normal: [u32; 3],
}

/// Each unsigned 10-bit component is biased by 511; top two bits are ignored.
/// Original rigid path does not normalize or divide these normal values by 511.
pub fn unpack_skin_normal(word: u32) -> [u32; 3] {
    std::array::from_fn(|i| (((word >> (i * 10)) & 0x3ff) as i32 - 511) as f32).map(f32::to_bits)
}
pub fn skin_rigid_vertex(
    command: u32,
    vertex: BindSkinVertex,
    palette: &[[u32; 16]],
) -> Result<SkinVertex, String> {
    if command & 0xf0000000 != 0 {
        return Err("unsupported non-rigid skin command".into());
    }
    // Unsigned quotient, including low values that are not divisible by six.
    let bone = ((command & 0xfff) / 6) as usize;
    let m = palette
        .get(bone)
        .ok_or("skin command bone is outside palette")?
        .map(f32::from_bits);
    let p = vertex.position.map(f32::from_bits);
    let n = unpack_skin_normal(vertex.packed_normal).map(f32::from_bits);
    Ok(SkinVertex {
        position: [
            ((p[1] * m[1] + p[2] * m[2]) + m[0] * p[0]) + m[3],
            ((m[6] * p[2] + m[5] * p[1]) + m[4] * p[0]) + m[7],
            ((m[10] * p[2] + m[9] * p[1]) + p[0] * m[8]) + m[11],
        ]
        .map(f32::to_bits),
        normal: [
            (n[0] * m[0] + m[2] * n[2]) + n[1] * m[1],
            (m[5] * n[1] + m[6] * n[2]) + n[0] * m[4],
            (m[9] * n[1] + m[10] * n[2]) + n[0] * m[8],
        ]
        .map(f32::to_bits),
    })
}
/// Prepared rigid stream only. Terminator is raw 0xffffffff, never a float compare.
/// Each successful command consumes one bind vertex and writes one output; tails remain.
/// Missing terminator/inputs or unsupported commands fail after preceding writes.
pub fn skin_rigid_stream(
    commands: &[u32],
    vertices: &[BindSkinVertex],
    palette: &[[u32; 16]],
    output: &mut [SkinVertex],
) -> Result<usize, String> {
    for (i, &command) in commands.iter().enumerate() {
        if command == u32::MAX {
            return Ok(i);
        }
        let vertex = *vertices.get(i).ok_or("skin stream lacks bind vertex")?;
        let target = output.get_mut(i).ok_or("skin stream output is too short")?;
        *target = skin_rigid_vertex(command, vertex, palette)?;
    }
    Err("skin stream lacks terminator".into())
}

fn bone_matrix(command: u32, palette: &[[u32; 16]]) -> Result<[f32; 16], String> {
    palette
        .get(((command & 0xfff) / 6) as usize)
        .copied()
        .map(|m| m.map(f32::from_bits))
        .ok_or_else(|| "skin command bone is outside palette".into())
}
/// Execute one non-copy command, preserving each native kernel's scalar SSE order.
/// Weight conversion includes all 28 low bits, including the bone address bits.
pub fn skin_influenced_vertex(
    commands: &[u32],
    vertex: BindSkinVertex,
    palette: &[[u32; 16]],
) -> Result<SkinVertex, String> {
    let command = *commands.first().ok_or("missing skin influence command")?;
    let kind = (command >> 28) as usize;
    if kind == 15 {
        return Err("copy/terminator is not an influence command".into());
    }
    let count = (kind & 7) + 1;
    if commands.len() != count {
        return Err("skin influence word count does not match command".into());
    }
    if kind == 0 {
        return skin_rigid_vertex(command, vertex, palette);
    }
    let p = vertex.position.map(f32::from_bits);
    let n = unpack_skin_normal(vertex.packed_normal).map(f32::from_bits);
    let m = bone_matrix(command, palette)?;
    let weight = |c: u32| (c & 0x0fffffff) as f32;
    let result = match kind {
        8 => cached_rigid(p, n, m),
        1 | 9 => {
            let m1 = bone_matrix(commands[1], palette)?;
            let weights = [weight(command), weight(commands[1])];
            if kind == 1 {
                weighted_two(p, n, m, m1, weights)
            } else {
                cached_two(p, n, m, m1, weights)
            }
        }
        _ => {
            let mut result = weighted_first(p, n, m, weight(command));
            for &c in &commands[1..] {
                result = weighted_next(p, n, bone_matrix(c, palette)?, weight(c), result);
            }
            result
        }
    };
    Ok(SkinVertex {
        position: std::array::from_fn(|i| result[i].to_bits()),
        normal: std::array::from_fn(|i| result[i + 3].to_bits()),
    })
}

#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq, Eq)]
pub struct SkinStreamVertex {
    pub vertex: SkinVertex,
    pub uv: [u32; 2],
}
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SkinStreamResult {
    pub written: usize,
    pub consumed_bind: usize,
    pub command_words: usize,
    pub cache_words: usize,
    pub kinds: [usize; 16],
}
/// Execute a native LOD command/UV stream into prepared output storage.
/// Cache is reset per call and contains raw position/normal words (six per write).
/// Prior completed records survive bounded-input errors; output tails are untouched.
/// Native allocation, profiling and post-terminator instance-buffer copies are external.
pub fn skin_lod_stream(
    commands: &[u32],
    vertices: &[BindSkinVertex],
    palette: &[[u32; 16]],
    output: &mut [SkinStreamVertex],
    cache: &mut Vec<u32>,
) -> Result<SkinStreamResult, String> {
    cache.clear();
    let (mut pc, mut bind, mut written) = (0, 0, 0);
    let mut kinds = [0; 16];
    loop {
        let command = *commands.get(pc).ok_or("skin stream lacks terminator")?;
        if command == u32::MAX {
            return Ok(SkinStreamResult {
                written,
                consumed_bind: bind,
                command_words: pc + 1,
                cache_words: cache.len(),
                kinds,
            });
        }
        let kind = (command >> 28) as usize;
        let count = if kind == 15 { 1 } else { (kind & 7) + 1 };
        let end = pc
            .checked_add(count + 2)
            .ok_or("skin command offset overflow")?;
        let record = commands
            .get(pc..end)
            .ok_or("truncated skin influences or UV words")?;
        let target = output
            .get_mut(written)
            .ok_or("skin stream output is too short")?;
        let vertex = if kind == 15 {
            // Native signed division is nonnegative after masking; quotient * 3 floats.
            let offset = ((command & 0x0fffffff) / 3) as usize * 3;
            let source = cache
                .get(offset..offset + 6)
                .ok_or("skin copy reads beyond written cache")?;
            SkinVertex {
                position: source[..3].try_into().unwrap(),
                normal: source[3..].try_into().unwrap(),
            }
        } else {
            let input = *vertices.get(bind).ok_or("skin stream lacks bind vertex")?;
            skin_influenced_vertex(&record[..count], input, palette)?
        };
        if kind != 15 {
            bind += 1;
            if kind & 8 != 0 {
                cache.extend(vertex.position);
                cache.extend(vertex.normal);
            }
        }
        *target = SkinStreamVertex {
            vertex,
            uv: [record[count], record[count + 1]],
        };
        kinds[kind] += 1;
        written += 1;
        pc = end;
    }
}
#[cfg(test)]
#[path = "skeletal_skin_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "skeletal_weighted_skin_tests.rs"]
mod weighted_tests;
