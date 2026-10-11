//! Selected non-bouncing physProjectile arithmetic (104924bb..104925eb),
//! AProjectile::BoundProjectileVelocity (1048b800), and human GetFireStart.
//! Collision/callbacks are host boundaries. Portable sqrt seed, not RSQRTSS parity.
pub fn bound_with_seed(
    velocity: [f32; 3],
    max_speed: f32,
    seed: impl FnOnce(f32) -> f32,
) -> [f32; 3] {
    let [x, y, z] = velocity;
    if (x * x + y * y) + z * z <= max_speed * max_speed {
        return velocity;
    }
    let squared = (z * z + y * y) + x * x;
    let inverse = if squared != 0. {
        let r = seed(squared);
        (3. - (r * squared) * r) * (r * 0.5)
    } else {
        1.
    };
    velocity.map(|v| (v * inverse) * max_speed)
}
pub fn step(
    velocity: [f32; 3],
    acceleration: [f32; 3],
    max_speed: f32,
    dt: f32,
) -> ([f32; 3], [f32; 3]) {
    let velocity = if acceleration.iter().any(|&v| v != 0.) {
        bound_with_seed(
            std::array::from_fn(|i| velocity[i] + acceleration[i] * dt),
            max_speed,
            |s| 1. / s.sqrt(),
        )
    } else {
        velocity
    };
    (velocity, velocity.map(|v| v * dt))
}
/// Prepared eye and original GetAxes rows. No zoom in the selected playable mode.
pub fn fire_start(eye: [f32; 3], axes: [[f32; 3]; 3], offset: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| {
        ((eye[i] + axes[0][i] * offset[0]) + axes[1][i] * offset[1]) + axes[2][i] * offset[2]
    })
}

/// AProjectile::execGetTossVelocity, SSE body 104ed7e0..104ed86c.
/// A present pawn and a prepared FRotator::Vector result are caller boundaries.
/// Only 40% of the pawn velocity projected onto the aim direction is inherited.
pub fn toss_velocity(
    direction: [f32; 3],
    pawn_velocity: [f32; 3],
    speed: f32,
    toss_z: f32,
) -> [f32; 3] {
    let along = ((direction[1] * pawn_velocity[1] + direction[0] * pawn_velocity[0])
        + pawn_velocity[2] * direction[2])
        * 0.4
        + speed;
    [
        direction[0] * along,
        direction[1] * along,
        toss_z + along * direction[2],
    ]
}

/// Selected direct-trace AdjustAim branch, without PickTarget or aim error.
/// FVector -> FRotator trigonometry uses portable atan2 before the original sine table.
pub fn reticle_direction(origin: [f32; 3], target: [f32; 3], view: [f32; 3]) -> [f32; 3] {
    let mut delta = std::array::from_fn::<_, 3, _>(|i| target[i] - origin[i]);
    let length = ((delta[0] * delta[0] + delta[1] * delta[1]) + delta[2] * delta[2]).sqrt();
    if length == 0.
        || ((delta[0] * view[0] + delta[1] * view[1]) + delta[2] * view[2]) / length < 0.4
    {
        delta = view;
    }
    let yaw =
        (f64::from(delta[1]).atan2(f64::from(delta[0])) * 65536. / std::f64::consts::TAU) as i32;
    let pitch = (f64::from(delta[2]).atan2(f64::from(
        (delta[0] * delta[0] + delta[1] * delta[1]).sqrt(),
    )) * 65536.
        / std::f64::consts::TAU) as i32;
    let matrix = crate::mesh_transform::rotation_matrix([pitch, yaw, 0], [0; 3])
        .expect("finite prepared reticle rotation");
    std::array::from_fn(|i| f32::from_bits(matrix[0][i]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn laser_accelerates_from_rest_and_reaches_cap() {
        let (v, d) = step([0.; 3], [60000., 0., 0.], 10000., 1. / 60.);
        assert!((v[0] - 1000.).abs() < 0.001);
        assert!((d[0] - 1000. / 60.).abs() < 0.00001);
        let (v, _) = step([10000., 0., 0.], [60000., 0., 0.], 10000., 1. / 60.);
        assert!((v[0] - 10000.).abs() < 0.002);
    }
    #[test]
    fn zero_cap_stops_accelerated_projectile_but_zero_acceleration_skips_bound() {
        assert_eq!(step([4., 0., 0.], [1., 0., 0.], 0., 1.).0, [0.; 3]);
        assert_eq!(step([4., 0., 0.], [0.; 3], 0., 1.).0, [4., 0., 0.]);
    }
    #[test]
    fn cap_retains_three_dimensional_direction() {
        let v = bound_with_seed([300., 400., 0.], 100., |s| 1. / s.sqrt());
        assert!((v[0] - 60.).abs() < 0.00002 && (v[1] - 80.).abs() < 0.00002);
    }
    #[test]
    fn offset_follows_axes_and_unzoomed_lateral_offset() {
        assert_eq!(
            fire_start(
                [10., 20., 30.],
                [[0., 1., 0.], [-1., 0., 0.], [0., 0., 1.]],
                [5., 2., -3.]
            ),
            [8., 25., 27.]
        );
    }
}
