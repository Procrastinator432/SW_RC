//! AActor::physRootMotion 1048f2a0 and APawn continuation 1048f3a0.
//! Pose evaluation, matrix-to-rotator conversion, collision and Sphyll are hosts.
//! No complete startNewPhysics dispatcher or live player animation controller.
/// GetMoveCoords(relative=true): sampled root matrix * persistent entry anchor.
/// Preserves the original MULSS/ADDSS order; rejects nonfinite host matrices.
pub fn compose_relative(root: [u32; 16], anchor: [u32; 16]) -> Result<[u32; 16], String> {
    let root = root.map(f32::from_bits);
    let anchor = anchor.map(f32::from_bits);
    if root.iter().chain(anchor.iter()).any(|v| !v.is_finite()) {
        return Err("Nonfinite root-motion matrix".into());
    }
    let mut result = [0; 16];
    for (index, order) in crate::root_motion_orders::RELATIVE_ORDER.iter().enumerate() {
        let row = index / 4;
        let col = index % 4;
        let mut sum = root[row * 4 + order[0]] * anchor[order[0] * 4 + col];
        for k in &order[1..] {
            sum += root[row * 4 + k] * anchor[k * 4 + col];
        }
        if !sum.is_finite() {
            return Err("Root-motion matrix overflow".into());
        }
        result[index] = sum.to_bits();
    }
    Ok(result)
}

pub trait RootMotionHost {
    fn move_coords(&mut self, relative: bool) -> Result<Option<[u32; 16]>, String>;
    fn matrix_rotation(&mut self, matrix: &[u32; 16]) -> Result<[i32; 3], String>;
    fn set_rotation(&mut self, rotation: [i32; 3]) -> Result<(), String>;
    /// Read after SetRotation, matching the native order.
    fn location(&mut self) -> Result<[u32; 3], String>;
    /// The native movement return value does not determine animation availability.
    fn move_smooth(&mut self, delta: [u32; 3]) -> Result<i32, String>;
    fn update_sphyll(&mut self) -> Result<(), String>;
}

/// True consumes the physics branch even when collision prevents displacement.
/// Native DeltaTime is unused here; animation ticking happens before this call.
pub fn phys_root_motion(
    mesh_instance: bool,
    pawn: bool,
    host: &mut impl RootMotionHost,
) -> Result<bool, String> {
    if !mesh_instance {
        return Ok(false);
    }
    let Some(matrix) = host.move_coords(true)? else {
        return Ok(false);
    };
    let rotation = host.matrix_rotation(&matrix)?;
    host.set_rotation(rotation)?;
    let location = host.location()?;
    let delta = std::array::from_fn(|i| {
        (f32::from_bits(matrix[12 + i]) - f32::from_bits(location[i])).to_bits()
    });
    host.move_smooth(delta)?;
    if pawn {
        host.update_sphyll()?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Host {
        available: bool,
        events: Vec<&'static str>,
        position: [u32; 3],
        delta: Option<[u32; 3]>,
    }
    impl RootMotionHost for Host {
        fn move_coords(&mut self, relative: bool) -> Result<Option<[u32; 16]>, String> {
            assert!(relative);
            self.events.push("coords");
            let mut matrix = [0; 16];
            matrix[12..15].copy_from_slice(&[15f32, -7., 600.].map(f32::to_bits));
            Ok(self.available.then_some(matrix))
        }
        fn matrix_rotation(&mut self, _: &[u32; 16]) -> Result<[i32; 3], String> {
            self.events.push("rotation");
            Ok([10, 20, 30])
        }
        fn set_rotation(&mut self, r: [i32; 3]) -> Result<(), String> {
            assert_eq!(r, [10, 20, 30]);
            self.events.push("set_rotation");
            // Make the order observable: native reads location after this call.
            self.position = [3f32, -2., 1000.].map(f32::to_bits);
            Ok(())
        }
        fn location(&mut self) -> Result<[u32; 3], String> {
            self.events.push("location");
            Ok(self.position)
        }
        fn move_smooth(&mut self, delta: [u32; 3]) -> Result<i32, String> {
            self.events.push("move");
            self.delta = Some(delta);
            Ok(0) // blocked; caller must still consume the root-motion branch.
        }
        fn update_sphyll(&mut self) -> Result<(), String> {
            self.events.push("sphyll");
            Ok(())
        }
    }
    fn host(available: bool) -> Host {
        Host {
            available,
            events: vec![],
            position: [0; 3],
            delta: None,
        }
    }
    #[test]
    fn absent_mesh_or_root_allows_normal_physics_without_side_effects() {
        let mut h = host(true);
        assert!(!phys_root_motion(false, true, &mut h).unwrap());
        assert!(h.events.is_empty());
        let mut h = host(false);
        assert!(!phys_root_motion(true, true, &mut h).unwrap());
        assert_eq!(h.events, ["coords"]);
    }
    #[test]
    fn blocked_pawn_move_still_consumes_branch_and_updates_sphyll() {
        let mut h = host(true);
        assert!(phys_root_motion(true, true, &mut h).unwrap());
        assert_eq!(h.delta.unwrap().map(f32::from_bits), [12., -5., -400.]);
        assert_eq!(
            h.events,
            [
                "coords",
                "rotation",
                "set_rotation",
                "location",
                "move",
                "sphyll"
            ]
        );
    }
    #[test]
    fn ordinary_actor_has_no_pawn_continuation() {
        let mut h = host(true);
        assert!(phys_root_motion(true, false, &mut h).unwrap());
        assert_eq!(h.events.last(), Some(&"move"));
    }
    #[test]
    fn relative_composition_preserves_native_cancellation_order() {
        let mut root = [0.; 16];
        root[..4].copy_from_slice(&[1e20, -1e20, 3., 4.]);
        let result = compose_relative(root.map(f32::to_bits), [1f32.to_bits(); 16]).unwrap();
        assert_eq!(&result[..4], &[0, 3f32.to_bits(), 3f32.to_bits(), 0]);
        // A conventional order would yield 7 in the first component.
        let standard = ((root[0] + root[1]) + root[2]) + root[3];
        assert_eq!(standard, 7.);
    }
    #[test]
    fn relative_composition_applies_entry_rotation_and_translation() {
        let mut root: [u32; 16] = crate::mesh_transform::rotation_matrix([0; 3], [0; 3])
            .unwrap()
            .concat()
            .try_into()
            .unwrap();
        root[12..15].copy_from_slice(&[10f32, 0., -600.].map(f32::to_bits));
        let anchor = crate::mesh_transform::rotation_matrix(
            [0, 16384, 0],
            [100f32, 200., 300.].map(f32::to_bits),
        )
        .unwrap();
        let world = compose_relative(root, anchor.concat().try_into().unwrap()).unwrap();
        assert_eq!(
            world[12..15]
                .iter()
                .map(|v| f32::from_bits(*v))
                .collect::<Vec<_>>(),
            [100., 210., -300.]
        );
        root[0] = f32::NAN.to_bits();
        assert!(compose_relative(root, [0; 16]).is_err());
    }
}
