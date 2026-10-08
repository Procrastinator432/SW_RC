use super::inverse_matrix;
fn identity() -> [u32; 16] {
    let mut m = [0; 16];
    for i in [0, 5, 10, 15] {
        m[i] = 1f32.to_bits();
    }
    m
}
#[test]
fn identity_and_exact_singular_fallback() {
    assert_eq!(inverse_matrix(identity()), identity());
    assert_eq!(inverse_matrix([0; 16]), identity());
    let mut m = identity();
    m[0] = 0;
    assert_eq!(inverse_matrix(m), identity());
}
#[test]
fn translated_reflected_nonuniform_inverse() {
    let mut m = identity();
    m[0] = (-2f32).to_bits();
    m[5] = 4f32.to_bits();
    m[10] = 0.5f32.to_bits();
    m[12..15].copy_from_slice(&[10f32, 20., 30.].map(f32::to_bits));
    let inv = inverse_matrix(m).map(f32::from_bits);
    assert_eq!([inv[0], inv[5], inv[10]], [-0.5, 0.25, 2.]);
    assert_eq!(&inv[12..15], &[5., -5., -60.]);
}
#[test]
fn general_full_matrices_multiply_back_to_identity() {
    for seed in 0..32 {
        let m = std::array::from_fn::<_, 16, _>(|i| {
            let v = (((i * 7 + seed * 11) % 17) as f32 - 8.) * 0.03125;
            if i / 4 == i % 4 {
                v + 2.
            } else {
                v
            }
        });
        let inv = inverse_matrix(m.map(f32::to_bits)).map(f32::from_bits);
        for row in 0..4 {
            for col in 0..4 {
                let product: f64 = (0..4)
                    .map(|k| m[row * 4 + k] as f64 * inv[k * 4 + col] as f64)
                    .sum();
                assert!((product - if row == col { 1. } else { 0. }).abs() < 2e-6);
            }
        }
    }
}
#[test]
fn unordered_determinant_is_not_singular_identity_fallback() {
    let mut m = identity();
    m[0] = f32::NAN.to_bits();
    assert!(inverse_matrix(m)
        .iter()
        .any(|&v| f32::from_bits(v).is_nan()));
}
