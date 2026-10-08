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
#[test]
fn palette_identity_and_transpose() {
    let mut pose = identity();
    pose[12..15].copy_from_slice(&[4f32, 5., 6.].map(f32::to_bits));
    let mut out = vec![];
    build_skin_palette(&[pose], &[identity()], &mut out).unwrap();
    assert_eq!(
        [out[0][3], out[0][7], out[0][11]],
        [4f32, 5., 6.].map(f32::to_bits)
    );
    assert_eq!(out[0][12..15], [0; 3]);
}
#[test]
fn palette_product_order_is_inverse_then_pose() {
    let mut inverse = identity();
    inverse[12] = 2f32.to_bits();
    let mut pose = identity();
    pose[0] = 3f32.to_bits();
    let mut out = vec![];
    build_skin_palette(&[pose], &[inverse], &mut out).unwrap();
    assert_eq!(out[0][3], 6f32.to_bits());
}
#[test]
fn palette_resize_and_partial_writes_on_short_inverse() {
    let mut out = vec![[77; 16]; 3];
    assert!(build_skin_palette(&[identity(); 2], &[identity()], &mut out).is_err());
    assert_eq!(out.len(), 2);
    assert_eq!(out[0], identity());
    assert_eq!(out[1], [77; 16]);
    build_skin_palette(&[], &[], &mut out).unwrap();
    assert!(out.is_empty());
}
#[test]
fn normal_components_are_biased_unsigned_ten_bit_values() {
    assert_eq!(unpack_skin_normal(0), [-511f32; 3].map(f32::to_bits));
    assert_eq!(unpack_skin_normal(u32::MAX), [512f32; 3].map(f32::to_bits));
    assert_eq!(unpack_skin_normal(511 | (511 << 10) | (511 << 20)), [0; 3]);
}
#[test]
fn identity_skin_does_not_normalize_normal() {
    let v = skin_rigid_vertex(0, vertex(), &[identity()]).unwrap();
    assert_eq!(v.position, vertex().position);
    assert_eq!(v.normal, [0, 0, 511f32.to_bits()]);
}
#[test]
fn translated_point_leaves_normal_unchanged() {
    let mut palette = identity();
    palette[3] = 5f32.to_bits();
    palette[7] = (-2f32).to_bits();
    let v = skin_rigid_vertex(0, vertex(), &[palette]).unwrap();
    assert_eq!(v.position, [6f32, 0., 3.].map(f32::to_bits));
    assert_eq!(v.normal, [0, 0, 511f32.to_bits()]);
}
#[test]
fn rigid_command_uses_unsigned_low_twelve_quotient_and_ignores_middle_bits() {
    let mut second = identity();
    second[3] = 8f32.to_bits();
    let palette = [identity(), second];
    assert_eq!(
        skin_rigid_vertex(0x0abcd00b, vertex(), &palette)
            .unwrap()
            .position[0],
        9f32.to_bits()
    );
}
#[test]
fn nonrigid_and_out_of_range_commands_are_explicit_errors() {
    for command in [0x10000000, 0x80000000, 0xf0000000, u32::MAX, 0xfff] {
        assert!(skin_rigid_vertex(command, vertex(), &[identity()]).is_err());
    }
}
#[test]
fn stream_terminator_and_untouched_output_tail() {
    let mut out = [SkinVertex {
        position: [77; 3],
        normal: [88; 3],
    }; 2];
    assert_eq!(
        skin_rigid_stream(
            &[0, u32::MAX, 0x80000000],
            &[vertex()],
            &[identity()],
            &mut out
        )
        .unwrap(),
        1
    );
    assert_eq!(out[1].position, [77; 3]);
    assert_eq!(
        skin_rigid_stream(&[u32::MAX], &[], &[], &mut []).unwrap(),
        0
    );
}
#[test]
fn stream_failure_keeps_prior_successful_vertices() {
    for commands in [&[0, 0x10000000][..], &[0, 6][..], &[0][..]] {
        let mut out = [SkinVertex::default(); 2];
        assert!(skin_rigid_stream(commands, &[vertex(); 2], &[identity()], &mut out).is_err());
        assert_eq!(out[0].position, vertex().position);
        assert_eq!(out[1], SkinVertex::default());
    }
}
#[test]
fn missing_input_and_output_fail_without_writes() {
    let mut out = [SkinVertex::default()];
    assert!(skin_rigid_stream(&[0, u32::MAX], &[], &[identity()], &mut out).is_err());
    assert_eq!(out[0], SkinVertex::default());
    assert!(skin_rigid_stream(&[0, u32::MAX], &[vertex()], &[identity()], &mut []).is_err());
}
