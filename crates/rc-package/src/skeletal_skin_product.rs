//! Generated from ComputeSkinVerts 1050bd96..1050c14f, before transpose.
pub fn skin_product(inverse: [u32; 16], pose: [u32; 16]) -> [u32; 16] {
    let inverse = inverse.map(f32::from_bits);
    let pose = pose.map(f32::from_bits);
    [
        ((((inverse[3] * pose[12]) + (inverse[0] * pose[0])) + (inverse[2] * pose[8]))
            + (inverse[1] * pose[4])),
        ((((pose[9] * inverse[2]) + (inverse[1] * pose[5])) + (inverse[0] * pose[1]))
            + (pose[13] * inverse[3])),
        ((((pose[6] * inverse[1]) + (inverse[0] * pose[2])) + (inverse[3] * pose[14]))
            + (pose[10] * inverse[2])),
        ((((pose[7] * inverse[1]) + (inverse[3] * pose[15])) + (pose[3] * inverse[0]))
            + (inverse[2] * pose[11])),
        ((((pose[4] * inverse[5]) + (pose[0] * inverse[4])) + (pose[12] * inverse[7]))
            + (pose[8] * inverse[6])),
        ((((pose[13] * inverse[7]) + (pose[9] * inverse[6])) + (inverse[5] * pose[5]))
            + (inverse[4] * pose[1])),
        ((((pose[14] * inverse[7]) + (pose[10] * inverse[6])) + (inverse[4] * pose[2]))
            + (pose[6] * inverse[5])),
        ((((pose[3] * inverse[4]) + (pose[15] * inverse[7])) + (pose[11] * inverse[6]))
            + (pose[7] * inverse[5])),
        ((((pose[4] * inverse[9]) + (pose[0] * inverse[8])) + (pose[12] * inverse[11]))
            + (pose[8] * inverse[10])),
        ((((pose[13] * inverse[11]) + (pose[9] * inverse[10])) + (inverse[9] * pose[5]))
            + (inverse[8] * pose[1])),
        ((((pose[14] * inverse[11]) + (pose[10] * inverse[10])) + (inverse[8] * pose[2]))
            + (pose[6] * inverse[9])),
        ((((pose[3] * inverse[8]) + (pose[15] * inverse[11])) + (pose[11] * inverse[10]))
            + (pose[7] * inverse[9])),
        ((((pose[4] * inverse[13]) + (pose[0] * inverse[12])) + (pose[12] * inverse[15]))
            + (pose[8] * inverse[14])),
        ((((pose[13] * inverse[15]) + (pose[9] * inverse[14])) + (inverse[13] * pose[5]))
            + (inverse[12] * pose[1])),
        ((((pose[14] * inverse[15]) + (pose[10] * inverse[14])) + (inverse[12] * pose[2]))
            + (pose[6] * inverse[13])),
        ((((pose[3] * inverse[12]) + (pose[15] * inverse[15])) + (pose[11] * inverse[14]))
            + (pose[7] * inverse[13])),
    ]
    .map(f32::to_bits)
}
