//! Isolated original landing-curve/world probe; does not modify a gameplay replay.
use std::{env, fs, path::Path};
struct Sample {
    matrix: [u32; 16],
    flags: u8,
}
impl rc_package::move_coords::MoveCoordsHost for Sample {
    fn sequence_flags(
        &mut self,
        _: usize,
        _: &mut [rc_package::mesh_animation::AnimationChannel],
    ) -> Result<Option<u8>, String> {
        Ok(Some(self.flags))
    }
    fn evaluate_move_matrix(
        &mut self,
        _: usize,
        _: &mut [rc_package::mesh_animation::AnimationChannel],
    ) -> Result<[u32; 16], String> {
        Ok(self.matrix)
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args_os().collect();
    if a.len() != 4 {
        return Err("usage: rc-rappel-path-check SCENE ASSET_DIR OUT".into());
    }
    let g = rc_game::bundle::decode(&fs::read(&a[1])?)?;
    let dir = Path::new(&a[2]);
    let actors: serde_json::Value = serde_json::from_slice(&fs::read(dir.join("actors.json"))?)?;
    let spawn = actors["actors"]
        .as_array()
        .ok_or("Missing actors")?
        .iter()
        .find(|v| v["original"] == 10)
        .ok_or("Missing original PlayerStart")?;
    let location: [f64; 3] =
        serde_json::from_value(spawn["resolved"]["Location"]["value"]["value"].clone())?;
    let rotation: [i32; 3] =
        serde_json::from_value(spawn["resolved"]["Rotation"]["value"]["value"].clone())?;
    let mesh: rc_game::visual::Mesh =
        serde_json::from_slice(&fs::read(dir.join("player-mesh.json"))?)?;
    let p = mesh.prepare_reference()?;
    let c = p.clip("EntranceRope600")?;
    let s = &p.mesh.clips[c].sequence;
    let extent = g.profile.physics.extent;
    let anchor: [u32; 16] = rc_package::mesh_transform::rotation_matrix(
        rotation,
        location.map(|v| (v as f32).to_bits()),
    )?
    .concat()
    .try_into()
    .unwrap();
    let identity: [u32; 16] = rc_package::mesh_transform::rotation_matrix([0; 3], [0; 3])?
        .concat()
        .try_into()
        .unwrap();
    // EntranceRope600 begins at identity, so PostInit's inverse(first)*actor
    // is the actor matrix. General nonidentity entry composition stays open.
    assert_eq!(
        p.root_matrix(c, 0.)?.map(f32::from_bits),
        identity.map(f32::from_bits)
    );
    let mut from = location;
    let mut rows = vec![];
    let mut inputs = vec![];
    let mut expected = vec![];
    for i in 0..s.frames {
        let frame = i as f32 / s.frames as f32;
        let mut channel = [p.channel(c, frame)?];
        let mut world = [0; 16];
        let mut host = Sample {
            matrix: p.root_matrix(c, frame)?,
            flags: s.flags as u8,
        };
        assert!(rc_package::move_coords::get_move_coords_relative(
            &mut channel,
            0,
            Some(&mut world),
            anchor,
            &mut host
        )?);
        for word in host.matrix.into_iter().chain(anchor) {
            inputs.extend(word.to_le_bytes());
        }
        for word in world {
            expected.extend(word.to_le_bytes());
        }
        let to = std::array::from_fn(|j| f32::from_bits(world[12 + j]) as f64);
        let sweep = g.world.sweep_motion(from, to, extent)?;
        if !sweep.complete {
            return Err("Incomplete world sweep".into());
        }
        let eye = [to[0], to[1], to[2] + 60.];
        rows.push(serde_json::json!({"frame":i,"from":from,"target":to,"root_world_matrix":world,"script_eye_without_bob_shake":eye,"sweep":sweep}));
        from = to;
    }
    fs::write(dir.join("matrix-input.bin"), inputs)?;
    fs::write(dir.join("matrix-rust.bin"), expected)?;
    fs::write(
        &a[3],
        serde_json::to_vec_pretty(
            &serde_json::json!({"isolated_original_curve_probe":true,"native_relative_matrix":true,"native_move_smooth_parity":false,"start":location,"rotation":rotation,"extent":extent,"accepted_scene_spawn":g.state.pawn.body.position,"endpoint":from,"start_placement":g.world.body_placement(location,extent)?,"endpoint_placement":g.world.body_placement(from,extent)?,"endpoint_floor":g.world.floor_contact(from,extent,32.,0.7)?,"rows":rows}),
        )?,
    )?;
    println!(
        "Original rappel start {location:?}, endpoint {from:?}, accepted scene {:?}",
        g.state.pawn.body.position
    );
    Ok(())
}
