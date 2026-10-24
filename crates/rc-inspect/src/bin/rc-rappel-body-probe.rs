//! Inspect original player skeleton and draw sections for a bounded arms adapter.
use std::{env, fs};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = env::args().collect();
    if !(3..=4).contains(&a.len()) {
        return Err("usage: rc-rappel-body-probe SCENE OUT.json [GAME_DATA]".into());
    }
    let g = rc_game::bundle::decode(&fs::read(&a[1])?)?;
    let player = &g
        .opening
        .as_ref()
        .ok_or("Opening")?
        .rappel
        .as_ref()
        .ok_or("Rappel")?
        .player;
    let mesh = &player.mesh;
    let mut samples = vec![];
    let clip = player.clip("EntranceRope600")?;
    for frame in [0.2, 0.4, 0.6, 0.8] {
        let channel = player.channel(clip, frame)?;
        let joints = player.joint_positions(&channel, false)?;
        let root = player.root_matrix(clip, frame)?.map(f32::from_bits);
        let points = mesh
            .bones
            .iter()
            .zip(joints)
            .filter(|(b, _)| ["Head", "wrist_L", "wrist_R"].contains(&b.name.name.as_str()))
            .map(|(b, p)| {
                (
                    b.name.name.clone(),
                    std::array::from_fn::<_, 3, _>(|i| p[i] - root[12 + i]),
                )
            })
            .collect::<Vec<_>>();
        samples.push(serde_json::json!({"frame":frame,"root":root,"joints_relative_root":points}));
    }
    let mut defaults = vec![];
    let mut textures = vec![];
    if a.len() == 4 {
        let bytes = fs::read(std::path::Path::new(&a[3]).join("Textures/CloneTextures.utx"))?;
        let pkg = rc_package::read_package(&bytes)?;
        for (i, e) in pkg.exports.iter().enumerate() {
            if e.name.to_lowercase().contains("commando") {
                textures.push(serde_json::json!({"path":pkg.object_path(i as i32+1)?,"class":pkg.object_path(e.class)?}));
            }
        }
        for file in ["Properties/properties.u", "System/ctcharacters.u"] {
            let bytes = fs::read(std::path::Path::new(&a[3]).join(file))?;
            let pkg = rc_package::read_package(&bytes)?;
            for e in &pkg.exports {
                if e.class == 0 && e.name.to_lowercase().contains("commando") {
                    let props = rc_package::classes::read(&pkg, &bytes, e)?.properties;
                    let raw = pkg.payload(&bytes, e)?;
                    let skins = props
                        .values
                        .iter()
                        .filter(|p| p.name == "Skins")
                        .map(|p| {
                            rc_package::properties::object_array(
                                &pkg,
                                &raw[p.payload_offset..p.payload_offset + p.bytes],
                            )
                        })
                        .collect::<Vec<_>>();
                    defaults.push(serde_json::json!({"file":file,"class":e.name,"properties":props,"skins":skins}));
                }
            }
        }
    }
    fs::write(
        &a[2],
        serde_json::to_vec_pretty(
            &serde_json::json!({"bones":mesh.bones,"sections":rc_package::skeletal_draw::draw_sections(&mesh.lod),"program":rc_package::skeletal_lod::inspect_skin_program(&mesh.lod.commands,mesh.lod.bind_vertices.len())?,"materials":mesh.materials.iter().map(|m|&m.source).collect::<Vec<_>>(),"defaults":defaults,"textures":textures,"samples":samples}),
        )?,
    )?;
    Ok(())
}
