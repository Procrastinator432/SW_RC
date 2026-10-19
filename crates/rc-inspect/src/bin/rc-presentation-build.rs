//! Bind selected original GEO_01A factories/prop transitions to visible effects.
use rc_package::{
    properties::{self, Value},
    read_package,
};
use std::{env, fs, path::Path};
#[derive(serde::Serialize)]
struct Pixels {
    #[serde(with = "rc_game::pixel_words")]
    pixels: Vec<u32>,
}
fn compact(value: &mut serde_json::Value) -> Result<usize, String> {
    let mut count = 0;
    if let Some(object) = value.as_object_mut() {
        if object.contains_key("uv_scale")
            && object
                .get("source")
                .is_some_and(serde_json::Value::is_string)
        {
            if let Some(pixels) = object.get("pixels").and_then(serde_json::Value::as_array) {
                let words = pixels
                    .iter()
                    .map(|p| {
                        p.as_u64()
                            .filter(|p| *p <= u64::from(u32::MAX))
                            .map(|p| p as u32)
                            .ok_or("Invalid material pixel")
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let encoded =
                    serde_json::to_value(Pixels { pixels: words }).map_err(|e| e.to_string())?;
                object.insert("pixels".into(), encoded["pixels"].clone());
                count += 1;
            }
        }
        for value in object.values_mut() {
            count += compact(value)?;
        }
    } else if let Some(values) = value.as_array_mut() {
        for value in values {
            count += compact(value)?;
        }
    }
    Ok(count)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 4 {
        return Err("usage: rc-presentation-build GAME_DATA SCENE OUT".into());
    }
    let root = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    let raw = fs::read(&args[2])?;
    if !raw.starts_with(b"RCPLAY13") {
        return Err("Expected Opening scene".into());
    }
    let game = rc_game::bundle::decode(&raw)?;
    let n = u32::from_le_bytes(raw[8..12].try_into()?) as usize;
    let mut opening: rc_game::opening::Assets = serde_json::from_slice(&raw[12..12 + n])?;
    if !opening.world_effects.is_empty() {
        return Err("Presentation effects already bound".into());
    }
    let cat = rc_inspect::defaults::load(root)?;
    let data = fs::read(root.join("Maps/geo_01a.ctm"))?;
    let map = read_package(&data)?;
    let station = &map.exports[735];
    let station_class = map.object_path(station.class)?;
    let station_props = properties::read(&map, &data, station)?;
    let station_value = |field| {
        cat.instance(&station_class, &station_props, field, 0, "geo_01a")
            .map(|v| v.value)
    };
    let Value::Vector(center) = station_value("Location")? else {
        return Err("Station center".into());
    };
    let Value::Float(radius) = station_value("CollisionRadius")? else {
        return Err("Station radius".into());
    };
    let Value::Float(height) = station_value("CollisionHeight")? else {
        return Err("Station height".into());
    };
    for field in ["bCollideActors", "bBlockActors", "bBlockPlayers"] {
        if !matches!(station_value(field)?, Value::Bool(true)) {
            return Err(format!("Station {field} differs").into());
        }
    }
    let collision = rc_package::cylinder::Cylinder {
        center: center.map(f64::from),
        radius: f64::from(radius),
        height: f64::from(height),
    };
    collision.validate()?;
    opening.bacta.as_mut().ok_or("Missing station")?.collision = Some(collision);
    opening.provenance["bacta"]["collision"] = serde_json::to_value(collision)?;
    opening.provenance["bacta"]["transform"] = "Legacy bottom-aligned visual; original actor Location/CollisionRadius/CollisionHeight cylinder".into();
    opening.provenance["bacta"]["limits"] = "Selected SpriteEmitter host, local RNG and idle body pose policy; not full native particle/animation parity. Station recharge/reuse after objective remains outside this tutorial host".into();
    let mesh_bytes = fs::read(root.join("StaticMeshes/Arena_HR.usx"))?;
    let mesh_pkg = read_package(&mesh_bytes)?;
    let mass_export = mesh_pkg
        .exports
        .iter()
        .enumerate()
        .find(|(i, _)| {
            mesh_pkg
                .object_path(*i as i32 + 1)
                .is_ok_and(|p| p == "ArenaWalls.Blockade03.KMeshProps1280")
        })
        .ok_or("Missing original barrier mass properties")?
        .1;
    let mass = rc_package::karma_mesh::read_box(&mesh_pkg, &mesh_bytes, mass_export)?;
    let mut rigid_receipts = vec![];
    let momentum = |class: &str| -> Result<(f32, f32), String> {
        let Value::Object { path, .. } = cat.resolve(class, "MyDamageType", 0)?.value else {
            return Err("Missing projectile damage type".into());
        };
        let Value::Float(impulse) = cat.resolve(&path, "KDamageImpulse", 0)?.value else {
            return Err("Missing KDamageImpulse".into());
        };
        let Value::Float(up) = cat.resolve(&path, "KDeathUpKick", 0)?.value else {
            return Err("Missing KDeathUpKick".into());
        };
        Ok((impulse, up))
    };
    let Value::Float(pawn_mass) = cat.resolve("CTGame.CTPawn", "Mass", 0)?.value else {
        return Err("Missing original pawn mass".into());
    };
    let (shot_momentum, _) = momentum("CTInventory.DC15Proj")?;
    let (blast_momentum, blast_up_kick) = momentum("CTInventory.ThermalDetonatorProj")?;
    let mut interactions = rc_game::rigid_interaction::Assets {
        props: vec![],
        shot_momentum,
        blast_momentum,
        blast_up_kick,
        pawn_mass: f64::from(pawn_mass),
    };
    for id in [909_u32, 910, 912, 913] {
        let e = &map.exports[id as usize - 1];
        let class = map.object_path(e.class)?;
        let props = properties::read(&map, &data, e)?;
        let value = |field| {
            cat.instance(&class, &props, field, 0, "geo_01a")
                .map(|p| p.value)
        };
        let Value::Object { index, .. } = value("KParams")? else {
            return Err("Original KParams reference".into());
        };
        if index <= 0 {
            return Err("Missing original KParams".into());
        }
        let ke = &map.exports[index as usize - 1];
        let kc = map.object_path(ke.class)?;
        let kp = properties::read(&map, &data, ke)?;
        let param = |field| cat.instance(&kc, &kp, field, 0, "geo_01a").map(|p| p.value);
        let number = |field| -> Result<f64, String> {
            match param(field)? {
                Value::Float(v) => Ok(f64::from(v)),
                _ => Err(format!("KParams {field}")),
            }
        };
        let Value::Vector(location) = value("Location")? else {
            return Err("Barrier location".into());
        };
        let Value::Rotator(rot) = value("Rotation")? else {
            return Err("Barrier rotation".into());
        };
        let Value::Float(scale) = value("DrawScale")? else {
            return Err("Barrier scale".into());
        };
        let Value::Vector(scale3) = value("DrawScale3D")? else {
            return Err("Barrier scale3".into());
        };
        let Value::Vector(pivot) = value("PrePivot")? else {
            return Err("Barrier pivot".into());
        };
        let scale3 = scale3.map(|v| v * scale);
        if scale3[0] != scale3[1]
            || scale3[0] != scale3[2]
            || scale3[0] <= 0.
            || !matches!(param("bKStayUpright")?, Value::Bool(false))
        {
            return Err("Unsupported selected Karma scale/constraint".into());
        }
        let columns: [[f64; 3]; 3] = std::array::from_fn(|i| {
            let mut axis = [0.; 3];
            axis[i] = 1.;
            rc_package::mesh::transform_point(axis, [0.; 3], rot, [1.; 3], [0.; 3]).map(f64::from)
        });
        // Preserve the local box axes before adding the large world origin.
        // Rounding each world corner to f32 made narrow boxes non-orthogonal.
        let transform = |p: [f64; 3]| {
            std::array::from_fn(|i| {
                f64::from(location[i])
                    + (0..3)
                        .map(|j| {
                            (p[j] * rc_package::karma_impulse::WORLD_UNITS - f64::from(pivot[j]))
                                * f64::from(scale3[j])
                                * columns[j][i]
                        })
                        .sum::<f64>()
            })
        };
        let center = transform(mass.center);
        let Value::Bool(initial_take_shot) = value("bKTakeShot")? else {
            return Err("Missing bKTakeShot".into());
        };
        let Value::Int(damage_threshold) = value("DamageThreshold")? else {
            return Err("Missing DamageThreshold".into());
        };
        let Value::Float(collision_radius) = value("CollisionRadius")? else {
            return Err("Missing CollisionRadius".into());
        };
        if !matches!(value("Physics")?, Value::Byte(0))
            || !matches!(value("LifeTime")?,Value::Float(v) if v==0.)
            || !matches!(value("Event")?,Value::Name(ref n) if n=="None")
        {
            return Err("Unsupported selected Karma initial physics/lifetime/event".into());
        }
        interactions.props.push(rc_game::rigid_interaction::Prop {
            original: id,
            location: location.map(f64::from),
            collision_radius,
            initial_take_shot,
            damage_threshold,
        });
        // Native convex order: x-max front four, x-min back four.
        let vertices = [5, 7, 3, 1, 4, 6, 0, 2]
            .map(|m| transform(std::array::from_fn(|i| mass.bounds[(m >> i) & 1][i])));
        let visual = game
            .dynamic
            .as_ref()
            .and_then(|p| p.objects.iter().find(|o| o.original == id))
            .ok_or("Missing original barrier mesh")?;
        let visual_bounds: [[f64; 3]; 2] = std::array::from_fn(|side| {
            std::array::from_fn(|axis| {
                visual
                    .triangles
                    .iter()
                    .flat_map(|t| t.points)
                    .map(|v| f64::from(v[axis]))
                    .fold(
                        if side == 0 {
                            f64::INFINITY
                        } else {
                            f64::NEG_INFINITY
                        },
                        if side == 0 { f64::min } else { f64::max },
                    )
            })
        });
        let physical_bounds: [[f64; 3]; 2] = std::array::from_fn(|side| {
            std::array::from_fn(|axis| {
                vertices.iter().map(|v| v[axis]).fold(
                    if side == 0 {
                        f64::INFINITY
                    } else {
                        f64::NEG_INFINITY
                    },
                    if side == 0 { f64::min } else { f64::max },
                )
            })
        });
        let ratios: [f64; 3] = std::array::from_fn(|i| {
            (physical_bounds[1][i] - physical_bounds[0][i])
                / (visual_bounds[1][i] - visual_bounds[0][i])
        });
        if ratios
            .iter()
            .any(|r| !r.is_finite() || !(0.9..=1.25).contains(r))
        {
            return Err(format!("Selected Karma/visual scale mismatch: {id} {ratios:?}").into());
        }
        let rows: [[f64; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|j| columns[j][i]));
        let [xx, xy, xz, yy, yz, zz] = mass.inertia;
        let tensor = [[xx, xy, xz], [xy, yy, yz], [xz, yz, zz]];
        let weight = number("KMass")?;
        let factor =
            weight * (f64::from(scale3[0]) * rc_package::karma_impulse::WORLD_UNITS).powi(2);
        let inertia = std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                (0..3)
                    .flat_map(|k| (0..3).map(move |l| rows[i][k] * tensor[k][l] * rows[j][l]))
                    .sum::<f64>()
                    * factor
            })
        });
        let Value::Vector(velocity) = param("KStartLinVel")? else {
            return Err("KStartLinVel".into());
        };
        let Value::Vector(angular) = param("KStartAngVel")? else {
            return Err("KStartAngVel".into());
        };
        if velocity != [0.; 3] || angular != [0.; 3] {
            return Err("Selected barriers have unexpected initial velocities".into());
        }
        let volume = game.volumes.select(center.map(|v| v as f32))?;
        let gravity_scale = number("KActorGravScale")?;
        let binding = rc_game::rigid::Binding {
            original: id,
            center,
            vertices,
            inertia,
            mass: weight,
            friction: number("KFriction")?,
            restitution: number("KRestitution")?,
            linear_damping: number("KLinearDamping")?,
            angular_damping: number("KAngularDamping")?,
            gravity: volume
                .settings
                .gravity
                .map(|v| f64::from(v) * gravity_scale),
            initial_velocity: [0.; 3],
            initial_angular_velocity: [0.; 3],
        };
        binding.validate()?;
        rigid_receipts.push(serde_json::json!({"original":id,"KParams":index,"binding":binding,"visual_bounds":visual_bounds,"physical_bounds":physical_bounds,"bounds_ratio":ratios,"world_units_per_karma_unit":rc_package::karma_impulse::WORLD_UNITS}));
        opening.rigid.push(binding);
    }
    interactions.validate(&opening.rigid)?;
    opening.rigid_interactions = Some(interactions);
    opening.provenance["rigid"] = serde_json::json!({"mesh":"Arena_HR.ArenaWalls.Blockade03.KMeshProps1280","serializer":"engine.dll 10386d10 / 10386ad0","mass_properties":mass,"actors":rigid_receipts,"solver":"Portable 480 Hz free rigid body integration, original COM/full inertia/convex box/mass/friction/restitution/gravity; triangle terrain and pairwise box contacts; saved pose and velocities","boundary":"Selected four barriers; not native MathEngine contact ordering or general Karma, projectile and blast impulses; finite-mass Pawn contact adapter, not native Pawn/Karma contact parity"});
    let teleport = &map.exports[2395 - 1];
    let teleport_props = properties::read(&map, &data, teleport)?;
    for (field, expected) in [
        ("ActorToTeleportTag", "PlayerExplosionFactory"),
        ("DestinationTag", "PlayerCommando"),
    ] {
        if !teleport_props
            .values
            .iter()
            .any(|p| p.name == field && matches!(&p.value, Value::Name(name) if name == expected))
        {
            return Err("Original injury teleport target differs".into());
        }
    }
    let program = opening
        .programs
        .iter_mut()
        .find(|p| p.original == 497)
        .ok_or("Missing injury program")?;
    let event = program
        .actions
        .iter()
        .position(|a| a.original == 2396)
        .ok_or("Missing injury effect event")?;
    if !program.actions.iter().any(|a| a.original == 2395) {
        program.actions.insert(
            event,
            rc_game::opening::Action {
                original: 2395,
                op: rc_game::opening::Op::TeleportEffectToPlayer { original: 823 },
            },
        );
    }
    if let Some(receipts) = opening.provenance["scripts"].as_array_mut() {
        for receipt in receipts.iter_mut().filter(|r| r["original"] == 2395) {
            receipt["operation"] =
                serde_json::to_value(rc_game::opening::Op::TeleportEffectToPlayer {
                    original: 823,
                })?;
            receipt["omitted_reason"] = serde_json::Value::Null;
            receipt["bound_by"] = "map-a-presentation".into();
        }
    }
    let mut door_motion = vec![];
    for id in [34, 35] {
        let e = &map.exports[id - 1];
        let class = map.object_path(e.class)?;
        let props = properties::read(&map, &data, e)?;
        let get = |field, index| {
            cat.instance(&class, &props, field, index, "geo_01a")
                .map(|v| v.value)
        };
        let Value::Float(delay) = get("DelayTime", 0)? else {
            return Err("Door delay".into());
        };
        let Value::Float(seconds) = get("MoveTimes", 0)? else {
            return Err("Door move time".into());
        };
        let Value::Vector(offset) = get("KeyPos", 1)? else {
            return Err("Door key position".into());
        };
        let Value::Rotator(rotation) = get("KeyRot", 1)? else {
            return Err("Door key rotation".into());
        };
        let Value::Byte(glide) = get("MoverGlideType", 0)? else {
            return Err("Door glide type".into());
        };
        if rotation != [0; 3] || glide > 1 {
            return Err("Unsupported original rotating door".into());
        }
        door_motion.push(rc_game::spire_runtime::DoorMotion {
            original: id as u32,
            delay: f64::from(delay),
            seconds: f64::from(seconds),
            glide: glide == 1,
            offset,
        });
    }
    let mut resolver =
        rc_inspect::assets::Assets::new(&[&root.join("Textures"), &root.join("System")])?;
    let mut sounds = game
        .mission
        .as_ref()
        .ok_or("Mission bank")?
        .sounds
        .iter()
        .map(|s| s.source.clone())
        .collect::<Vec<_>>();
    let mut receipts = vec![];
    let mut bindings = vec![];
    for (id, disabled) in [
        (211, None),
        (823, None),
        (822, None),
        (617, Some(617)),
        (442, None),
        (950, None),
        (951, None),
        (1135, None),
        (1136, None),
        (1137, None),
    ] {
        let e = &map.exports[id as usize - 1];
        let class = map.object_path(e.class)?;
        let props = properties::read(&map, &data, e)?;
        let get = |field| {
            cat.instance(&class, &props, field, 0, "geo_01a")
                .map(|v| v.value)
        };
        let Value::Vector(location) = get("Location")? else {
            return Err("Factory location".into());
        };
        let Value::Rotator(rotation) = get("Rotation")? else {
            return Err("Factory rotation".into());
        };
        let source = if id == 617 {
            let field = props
                .values
                .iter()
                .find(|p| p.name == "DestroyedState")
                .ok_or("DestroyedState")?;
            let payload = map.payload(&data, e)?;
            let transition = properties::tagged_struct(
                &map,
                &payload[field.payload_offset..field.payload_offset + field.bytes],
            )?;
            let Value::Object { path, .. } = &transition
                .values
                .iter()
                .find(|p| p.name == "TransitionEffect")
                .ok_or("Transition effect")?
                .value
            else {
                return Err("Transition effect type".into());
            };
            path.clone()
        } else {
            let Value::Object { path, index } = get("Prototype")? else {
                return Err("Factory effect prototype".into());
            };
            if index == 0 {
                return Err("Null factory prototype".into());
            }
            path
        };
        let event = if disabled.is_none() {
            let Value::Name(tag) = get("Tag")? else {
                return Err("Factory event tag".into());
            };
            Some(tag)
        } else {
            None
        };
        let mut role = |field: &str| -> Result<Option<usize>, String> {
            let Value::Object { index, path } = cat.resolve(&source, field, 0)?.value else {
                return Err("Effect sound field".into());
            };
            if index == 0 {
                return Ok(None);
            }
            if let Some(i) = sounds.iter().position(|s| s == &path) {
                return Ok(Some(i));
            }
            let (sound, _) = rc_inspect::mission_assets::sound_variant(root, &path, &mut receipts)
                .map_err(|e| e.to_string())?;
            let i = sounds.len();
            sounds.push(path.clone());
            opening.sound_bindings.push(rc_game::opening::SoundBinding {
                role: i,
                source: path,
                pcm: rc_game::replay::fingerprint(&sound.wave),
            });
            opening.sounds.push(sound);
            Ok(Some(i))
        };
        let sound = role("SpawnSound")?;
        let ambient = role("AmbientSound")?;
        let templates = rc_inspect::particle_assets::effect(root, &mut resolver, &source)?;
        receipts.push(serde_json::json!({"original":id,"class":class,"effect_class":source,"location":location,"rotation":rotation,"event":event,"disabled":disabled,"spawn_sound":sound,"ambient":ambient,"templates":templates.iter().map(|t|t.original).collect::<Vec<_>>()}));
        bindings.push(rc_game::world_effects::Binding {
            teleport_action: (id == 823).then_some(2395),
            original: id,
            source,
            event,
            disabled,
            location,
            rotation,
            templates,
            sound,
            ambient,
        });
    }
    opening.world_effects = bindings;
    opening.provenance["world_effects"] = serde_json::json!({"bindings":receipts,"injury":"175 scripted damage remains separate from original PlayerExplosionFactory823; original ACTION_TeleportToPoint2395 captures PlayerCommando location and pawn yaw before triggering factory823; fixed transform persists through checkpoint","barrier617":"PropDestroyed: original transition effect, no replacement mesh/material and no explosion damage => immediate actor removal","boundary":"Selected original SpriteEmitter assets, portable fixed-tick/range renderer; no general Karma, projector, global RNG or native driver parity"});
    let meta = serde_json::to_vec(&opening)?;
    let mut result = b"RCPLAY13".to_vec();
    result.extend((meta.len() as u32).to_le_bytes());
    result.extend(meta);
    let mut at = 12 + n;
    let mut compact_materials = 0;
    for _ in 0..16 {
        let header = raw.get(at..at + 12).ok_or("Truncated scene layer")?;
        if !header.starts_with(b"RCPLAY") {
            return Err("Invalid scene layer".into());
        }
        let size = u32::from_le_bytes(header[8..12].try_into()?) as usize;
        let mut value: serde_json::Value =
            serde_json::from_slice(raw.get(at + 12..at + 12 + size).ok_or("Layer metadata")?)?;
        if &header[..8] == b"RCPLAY09" {
            value["door_motion"] = serde_json::to_value(&door_motion)?;
            value["provenance"]["door_motion_source"] = serde_json::json!({"function":"AMover::physMovingBrush 104edfb0","curve":"MV_MoveByTime: t; MV_GlideByTime: 3*t*t - 2*t*t*t","geometry_and_collision":"same original KeyPos[1] displacement after DelayTime over MoveTimes[0]","limits":"Selected translation-only exit doors; encroachment/pushing/general mover keys are outside this host"});
        }
        compact_materials += compact(&mut value)?;
        let bytes = serde_json::to_vec(&value)?;
        result.extend_from_slice(&header[..8]);
        result.extend((bytes.len() as u32).to_le_bytes());
        result.extend(bytes);
        at += 12 + size;
        if &header[..8] == b"RCPLAY01" {
            result.extend_from_slice(&raw[at..]);
            break;
        }
    }
    if result.len() > 112 * 1024 * 1024 {
        return Err("Corrected scene exceeds native/Android payload budget".into());
    }
    rc_game::bundle::decode(&result)?;
    fs::create_dir_all(out)?;
    fs::write(out.join("geo_01a.rcplay"), &result)?;
    fs::write(
        out.join("presentation.json"),
        serde_json::to_vec_pretty(&opening.provenance["world_effects"])?,
    )?;
    fs::write(
        out.join("physics.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"bacta_collision":opening.provenance["bacta"]["collision"],"rigid":opening.provenance["rigid"]}),
        )?,
    )?;
    println!(
        "Bound {} original presentation effects, {} bytes; compacted {compact_materials} material arrays losslessly",
        opening.world_effects.len(),
        result.len()
    );
    Ok(())
}
