use rc_game::{
    artillery::{Actor, Assets, Notify},
    bundle,
};
use rc_package::{
    mesh_query::ActorTransform,
    properties::{self, Value},
    read_package,
};
use std::{env, fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 4 {
        return Err("usage: rc-artillery-build GAME_DATA SPIRE_SCENE OUTPUT".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    fs::create_dir_all(out)?;
    let base = fs::read(&args[2])?;
    let game = bundle::decode(&base)?;
    let d = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let p = read_package(&d)?;
    let cat = rc_inspect::defaults::load(root)?;
    let value = |id: usize, name: &str| -> Result<Value, String> {
        let e = &p.exports[id - 1];
        Ok(cat
            .instance(
                &p.object_path(e.class)?,
                &properties::read(&p, &d, e)?,
                name,
                0,
                "geo_01a",
            )?
            .value)
    };
    let vector = |id, name| -> Result<[f32; 3], String> {
        match value(id, name)? {
            Value::Vector(v) => Ok(v),
            _ => Err("Expected original vector".into()),
        }
    };
    let rotation = |id| -> Result<[i32; 3], String> {
        match value(id, "Rotation")? {
            Value::Rotator(v) => Ok(v),
            _ => Err("Expected original rotation".into()),
        }
    };
    let mut actors = vec![];
    let mut notifies = vec![];
    let mut sounds = vec![];
    let mut receipts = vec![];
    let mut sound = |path: &str| -> Result<(usize, f64), Box<dyn std::error::Error>> {
        let (s, seconds) = rc_inspect::mission_assets::sound_variant(root, path, &mut receipts)?;
        let i = sounds.len();
        sounds.push(s);
        Ok((i, seconds))
    };
    for (actor, (id, name)) in [(580, "Gunship"), (31, "SpiderDroid")]
        .into_iter()
        .enumerate()
    {
        let mesh = serde_json::from_slice(&fs::read(out.join(format!("{name}-mesh.json")))?)?;
        actors.push(Actor {
            original: id as u32,
            mesh: Some(mesh),
            location: vector(id, "Location")?,
            rotation: rotation(id)?,
        });
        let notes: Vec<serde_json::Value> =
            serde_json::from_slice(&fs::read(out.join(format!("{name}-notifies.json")))?)?;
        let vd = fs::read(root.join("Animations/vehicles.ukx"))?;
        let vp = read_package(&vd)?;
        for n in notes {
            let props = properties::read(
                &vp,
                &vd,
                &vp.exports[n["original"].as_u64().unwrap() as usize - 1],
            )?;
            let property = |name| {
                props
                    .values
                    .iter()
                    .find(|v| v.name == name)
                    .map(|v| &v.value)
            };
            let role = if let Some(Value::Object { path, .. }) = property("Sound") {
                Some(sound(path)?.0)
            } else {
                None
            };
            let effect = if let Some(Value::Object { path, .. }) = property("EffectClass") {
                Some(path.clone())
            } else {
                None
            };
            let bone = if let Some(Value::Name(s)) = property("Bone") {
                Some(s.clone())
            } else {
                None
            };
            let offset = if let Some(Value::Vector(v)) = property("OffsetLocation") {
                *v
            } else {
                [0.; 3]
            };
            notifies.push(Notify {
                actor,
                time: n["time"].as_f64().unwrap() as f32,
                sound: role,
                effect,
                bone,
                offset,
            });
        }
    }
    let (warning, warning_seconds) = sound("GEO_Voice.GEO_01.CA1G1711")?;
    let (whistle, _) = sound("GEN_Sound.Impacts_Explos.exp_GEN_MedWhistle_01")?;
    let (explosion, _) = sound("GEN_Sound.Impacts_Explos.exp_GEN_sw_large_02")?;
    let tutorial_source = match value(3551, "Sound")? {
        Value::Object { path, .. } => path,
        _ => return Err("Missing original tutorial sound".into()),
    };
    let (_, tutorial_seconds) = rc_inspect::mission_assets::sound(root, &tutorial_source)?;
    let mut spawn_receipts = vec![];
    let mut spawns = vec![];
    for id in [1018, 1028, 1029] {
        let pos = vector(id, "Location")?.map(f64::from);
        let from = [pos[0], pos[1], pos[2] + 100.];
        let floor = game
            .world
            .floor_contact(from, [50., 50., 115.], 300., 0.7)?
            .ok_or("Unsupported Warrior spawn")?;
        let at = [from[0], from[1], from[2] - 300. * floor.hit.fraction + 0.5];
        if game.world.body_placement(at, [50., 50., 115.])?.state
            == rc_package::movement::PlacementState::Penetrating
        {
            return Err(format!("Warrior spawn intersects world{id}").into());
        }
        spawns.push(at);
        spawn_receipts
            .push(serde_json::json!({"original":id,"map_location":pos,"supported_center":at}));
    }
    let mut materials = vec![];
    let missile = rc_inspect::mission_assets::mesh_scene(
        root,
        "Weapons.TrandoshanWeapons.TranMissile",
        &ActorTransform {
            location: [0.; 3],
            rotation: rotation(1013)?,
            scale: [1.; 3],
            pivot: [0.; 3],
        },
        &cat,
        &mut materials,
    )?;
    let rot = rotation(1013)?;
    let pitch = rot[0] as f64 * std::f64::consts::TAU / 65536.;
    let yaw = rot[1] as f64 * std::f64::consts::TAU / 65536.;
    let dir = rc_game::direction(yaw, pitch);
    let mut resolver =
        rc_inspect::assets::Assets::new(&[&root.join("Textures"), &root.join("System")])?;
    let mut effect_texture =
        |source: &str| -> Result<rc_game::artillery::Texture, Box<dyn std::error::Error>> {
            let mut t = resolver.diffuse_at_size(source, 128)?.texture;
            for px in &mut t.pixels {
                let bright = (*px & 255).max((*px >> 8) & 255).max((*px >> 16) & 255);
                *px = (*px & 0xffffff) | if bright < 18 { 0 } else { 0xff000000 };
            }
            Ok(rc_game::artillery::Texture {
                source: source.into(),
                width: t.width,
                height: t.height,
                pixels: t.pixels,
            })
        };
    let beam_texture = effect_texture("EffectTex.Lasers.Blaster_Beam")?;
    let ribbon_texture = effect_texture("EffectTex.RibbonGradients.Ribbon_pointy")?;
    assert!(matches!(value(533, "Capacity")?, Value::Int(3)));
    assert!(matches!(value(533, "MaxAlive")?, Value::Int(1)));
    assert!(matches!(value(1017, "Capacity")?, Value::Int(4)));
    assert!(matches!(value(1017, "MaxAlive")?, Value::Int(4)));
    let assets = Assets {
        beam_texture,
        ribbon_texture,
        actors,
        warrior: Some(serde_json::from_slice(&fs::read(
            out.join("Warrior-mesh.json"),
        )?)?),
        notifies,
        sounds,
        warning,
        warning_seconds,
        tutorial_source,
        tutorial_delay: tutorial_seconds + 3. + 2.,
        spawns,
        spawn_interval: 1.25,
        warrior_health: 100,
        warrior_speed: 450.,
        missile_snapshot: missile.snapshot()?,
        projectile_starts: [1013, 1014, 1015, 1016]
            .into_iter()
            .map(|id| vector(id, "Location").map(|v| v.map(f64::from)))
            .collect::<Result<_, _>>()?,
        projectile_velocity: dir.map(|v| v * 8000.),
        projectile_interval: 0.30000001192092896,
        projectile_lifespan: 15.,
        damage: 400.,
        radius: 512.,
        whistle,
        explosion,
        provenance: serde_json::json!({"original_scripts":[23,50,32],"tutorial_actions":[3552,3553,3554,3555],"spawn_factory":533,"projectile_factory":1017,"factory_policy":"MaxAlive1/Capacity3, deterministic midpoint intervals and round-robin spawn selection","projectile_policy":"Original direction and MaxSpeed8000; original InitialSpeed10000 clamped at launch; swept constant velocity PHYS_Projectile adapter","effect_policy":"Original notify bone/offset; existing bounded sprite adapter, not full emitter simulation"}),
    };
    let metadata = serde_json::to_vec(&assets)?;
    fs::write(out.join("artillery-assets.json"), &metadata)?;
    let mut bytes = b"RCPLAY10".to_vec();
    bytes.extend((metadata.len() as u32).to_le_bytes());
    bytes.extend(metadata);
    bytes.extend(base);
    let new = bundle::decode(&bytes)?;
    fs::write(out.join("geo_01a-artillery.rcplay"), &bytes)?;
    fs::write(
        out.join("build-validation.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"bytes":bytes.len(),"sounds":new.mission.as_ref().unwrap().sounds.len(),"spawns":spawn_receipts,"materials":materials,"audio":receipts}),
        )?,
    )?;
    println!(
        "Built artillery scene {} bytes, {} sounds",
        bytes.len(),
        new.mission.as_ref().unwrap().sounds.len()
    );
    Ok(())
}
