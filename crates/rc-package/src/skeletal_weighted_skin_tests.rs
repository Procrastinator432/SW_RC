use super::*;
fn identity() -> [u32; 16] {
    std::array::from_fn(|i| if i / 4 == i % 4 { 1f32.to_bits() } else { 0 })
}
fn vertex() -> BindSkinVertex {
    BindSkinVertex {
        position: [1f32, 2., 3.].map(f32::to_bits),
        packed_normal: 511 | (511 << 10) | (1022 << 20),
    }
}
fn cancellation() -> (BindSkinVertex, [u32; 16]) {
    let mut m = identity();
    m[..3].copy_from_slice(&[1e20f32, 3., -1e20].map(f32::to_bits));
    let mut v = vertex();
    v.position = [1f32.to_bits(); 3];
    (v, m)
}
#[test]
fn two_weights_keep_native_scale_without_normalizing_sum() {
    let out = skin_influenced_vertex(&[0x18000000, 0x08000000], vertex(), &[identity()]).unwrap();
    assert_eq!(out.position[0], 1.000_015_3f32.to_bits());
    assert_ne!(out.position, vertex().position);
}
#[test]
fn weight_includes_low_bone_address_bits() {
    let out = skin_influenced_vertex(&[0x10000000, 6], vertex(), &[identity(); 2]).unwrap();
    assert_eq!(
        out.position[0],
        (6f32 * f32::from_bits(0x31800080)).to_bits()
    );
    assert_ne!(out.position[0], 0);
}
#[test]
fn every_influence_count_accepts_zero_weight_without_normalization() {
    for kind in (1u32..8).chain(9..15) {
        let mut commands = vec![0; ((kind & 7) + 1) as usize];
        commands[0] = kind << 28;
        let out = skin_influenced_vertex(&commands, vertex(), &[identity()]).unwrap();
        assert_eq!(out, SkinVertex::default(), "kind {kind}");
    }
}
#[test]
fn influence_word_count_is_exact() {
    for commands in [
        &[][..],
        &[0, 0][..],
        &[0x20000000, 0][..],
        &[0x90000000][..],
    ] {
        assert!(skin_influenced_vertex(commands, vertex(), &[identity()]).is_err());
    }
}
#[test]
fn cached_rigid_retains_its_distinct_cancellation_order() {
    let (v, m) = cancellation();
    assert_eq!(skin_rigid_vertex(0, v, &[m]).unwrap().position[0], 0);
    assert_eq!(
        skin_influenced_vertex(&[0x80000000], v, &[m])
            .unwrap()
            .position[0],
        3f32.to_bits()
    );
}
#[test]
fn cached_two_retains_its_distinct_cancellation_order() {
    let (v, m) = cancellation();
    assert_eq!(
        skin_influenced_vertex(&[0x18000000, 0], v, &[m])
            .unwrap()
            .position[0],
        0
    );
    assert_eq!(
        skin_influenced_vertex(&[0x98000000, 0], v, &[m])
            .unwrap()
            .position[0],
        (3f32 * (134217728f32 * f32::from_bits(0x31800080))).to_bits()
    );
}
#[test]
fn influence_rejects_copy_terminator_and_missing_palette_bone() {
    for command in [0xf0000000, u32::MAX, 0x80000fff] {
        assert!(skin_influenced_vertex(&[command], vertex(), &[identity()]).is_err());
    }
}
#[test]
fn terminator_resets_cache_and_preserves_output_tail() {
    let mut cache = vec![77; 12];
    let sentinel = SkinStreamVertex {
        vertex: SkinVertex {
            position: [88; 3],
            normal: [99; 3],
        },
        uv: [123; 2],
    };
    let mut out = [sentinel];
    let result = skin_lod_stream(&[u32::MAX], &[], &[], &mut out, &mut cache).unwrap();
    assert_eq!(result.written, 0);
    assert_eq!(result.command_words, 1);
    assert!(cache.is_empty());
    assert_eq!(out, [sentinel]);
}
#[test]
fn copy_preserves_cached_raw_words_but_uses_its_own_uv() {
    let mut out = [SkinStreamVertex::default(); 2];
    let mut cache = vec![];
    let result = skin_lod_stream(
        &[
            0x80000000,
            0x80000000,
            u32::MAX,
            0xf0000002,
            17,
            18,
            u32::MAX,
        ],
        &[vertex()],
        &[identity()],
        &mut out,
        &mut cache,
    )
    .unwrap();
    assert_eq!(result.consumed_bind, 1);
    assert_eq!(result.cache_words, 6);
    assert_eq!(out[0].vertex, out[1].vertex);
    assert_eq!(out[1].uv, [17, 18]);
    assert_eq!(out[0].uv, [0x80000000, u32::MAX]);
}
#[test]
fn copy_quotient_can_span_adjacent_cache_entries() {
    let mut second = vertex();
    second.position = [4f32, 5., 6.].map(f32::to_bits);
    let mut out = [SkinStreamVertex::default(); 3];
    let mut cache = vec![];
    skin_lod_stream(
        &[
            0x80000000,
            0,
            0,
            0x80000000,
            0,
            0,
            0xf0000003,
            0,
            0,
            u32::MAX,
        ],
        &[vertex(), second],
        &[identity()],
        &mut out,
        &mut cache,
    )
    .unwrap();
    assert_eq!(out[2].vertex.position, out[0].vertex.normal);
    assert_eq!(out[2].vertex.normal, out[1].vertex.position);
    assert_eq!(cache.len(), 12);
}
#[test]
fn copy_cannot_read_future_or_unwritten_cache_words() {
    let mut out = [SkinStreamVertex::default(); 2];
    let mut cache = vec![0; 600];
    assert!(skin_lod_stream(
        &[0xf0000000, 0, 0, u32::MAX],
        &[],
        &[],
        &mut out,
        &mut cache
    )
    .is_err());
    assert!(cache.is_empty());
    assert!(skin_lod_stream(
        &[0x80000000, 0, 0, 0xf0000006, 0, 0, u32::MAX],
        &[vertex()],
        &[identity()],
        &mut out,
        &mut cache
    )
    .is_err());
    assert_eq!(cache.len(), 6);
}
#[test]
fn truncated_uv_retains_only_prior_completed_records() {
    let mut out = [SkinStreamVertex::default(); 2];
    let mut cache = vec![];
    assert!(skin_lod_stream(
        &[0x80000000, 1, 2, 0x80000000, 3],
        &[vertex(); 2],
        &[identity()],
        &mut out,
        &mut cache
    )
    .is_err());
    assert_eq!(out[0].uv, [1, 2]);
    assert_eq!(out[1], SkinStreamVertex::default());
    assert_eq!(cache.len(), 6);
}
#[test]
fn bind_shortage_retains_prior_output_and_cache() {
    let mut out = [SkinStreamVertex::default(); 2];
    let mut cache = vec![];
    assert!(skin_lod_stream(
        &[0x80000000, 1, 2, 0, 3, 4, u32::MAX],
        &[vertex()],
        &[identity()],
        &mut out,
        &mut cache
    )
    .is_err());
    assert_eq!(out[0].uv, [1, 2]);
    assert_eq!(out[1], SkinStreamVertex::default());
    assert_eq!(cache.len(), 6);
}
#[test]
fn bad_bone_retains_prior_output_and_cache() {
    let mut out = [SkinStreamVertex::default(); 2];
    let mut cache = vec![];
    assert!(skin_lod_stream(
        &[0x80000000, 1, 2, 6, 3, 4, u32::MAX],
        &[vertex(); 2],
        &[identity()],
        &mut out,
        &mut cache
    )
    .is_err());
    assert_eq!(out[0].uv, [1, 2]);
    assert_eq!(out[1], SkinStreamVertex::default());
    assert_eq!(cache.len(), 6);
}
#[test]
fn short_output_retains_prior_output_and_cache() {
    let mut out = [SkinStreamVertex::default()];
    let mut cache = vec![];
    assert!(skin_lod_stream(
        &[0x80000000, 1, 2, 0, 3, 4, u32::MAX],
        &[vertex(); 2],
        &[identity()],
        &mut out,
        &mut cache
    )
    .is_err());
    assert_eq!(out[0].uv, [1, 2]);
    assert_eq!(cache.len(), 6);
}
#[test]
fn missing_terminator_fails_after_prior_completed_record() {
    let mut out = [SkinStreamVertex::default()];
    let mut cache = vec![];
    assert!(skin_lod_stream(
        &[0x80000000, 1, 2],
        &[vertex()],
        &[identity()],
        &mut out,
        &mut cache
    )
    .unwrap_err()
    .contains("terminator"));
    assert_eq!(out[0].uv, [1, 2]);
    assert_eq!(cache.len(), 6);
}
