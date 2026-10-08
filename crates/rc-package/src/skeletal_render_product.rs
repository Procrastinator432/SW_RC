//! Native rigid Render product, 10510647..10510a12; generated, before actor transform.
pub fn rigid_render_product(inverse: [u32; 16], pose: [u32; 16]) -> [u32; 16] {
    let inverse = inverse.map(f32::from_bits);
    let pose = pose.map(f32::from_bits);
    [
        ((((pose[0] * inverse[0]) + (inverse[2] * pose[8])) + (inverse[1] * pose[4]))
            + (inverse[3] * pose[12])),
        ((((pose[13] * inverse[3]) + (inverse[0] * pose[1])) + (pose[5] * inverse[1]))
            + (pose[9] * inverse[2])),
        ((((pose[10] * inverse[2]) + (pose[2] * inverse[0])) + (inverse[3] * pose[14]))
            + (pose[6] * inverse[1])),
        ((((pose[7] * inverse[1]) + (inverse[3] * pose[15])) + (pose[3] * inverse[0]))
            + (inverse[2] * pose[11])),
        ((((inverse[6] * pose[8]) + (inverse[7] * pose[12])) + (pose[0] * inverse[4]))
            + (pose[4] * inverse[5])),
        ((((pose[5] * inverse[5]) + (inverse[7] * pose[13])) + (inverse[6] * pose[9]))
            + (inverse[4] * pose[1])),
        ((((pose[6] * inverse[5]) + (inverse[7] * pose[14])) + (inverse[6] * pose[10]))
            + (pose[2] * inverse[4])),
        ((((inverse[7] * pose[15]) + (inverse[6] * pose[11])) + (pose[7] * inverse[5]))
            + (pose[3] * inverse[4])),
        ((((inverse[10] * pose[8]) + (inverse[11] * pose[12])) + (pose[0] * inverse[8]))
            + (pose[4] * inverse[9])),
        ((((pose[5] * inverse[9]) + (inverse[11] * pose[13])) + (inverse[10] * pose[9]))
            + (inverse[8] * pose[1])),
        ((((pose[6] * inverse[9]) + (inverse[11] * pose[14])) + (inverse[10] * pose[10]))
            + (pose[2] * inverse[8])),
        ((((inverse[11] * pose[15]) + (inverse[10] * pose[11])) + (pose[7] * inverse[9]))
            + (pose[3] * inverse[8])),
        ((((inverse[14] * pose[8]) + (inverse[15] * pose[12])) + (pose[0] * inverse[12]))
            + (pose[4] * inverse[13])),
        ((((pose[5] * inverse[13]) + (inverse[15] * pose[13])) + (inverse[14] * pose[9]))
            + (inverse[12] * pose[1])),
        ((((pose[6] * inverse[13]) + (inverse[15] * pose[14])) + (inverse[14] * pose[10]))
            + (pose[2] * inverse[12])),
        ((((inverse[15] * pose[15]) + (inverse[14] * pose[11])) + (pose[7] * inverse[13]))
            + (pose[3] * inverse[12])),
    ]
    .map(f32::to_bits)
}
