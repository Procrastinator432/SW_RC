use std::{env, fs};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().collect();
    if args.len() != 2 {
        return Err("usage: rc-grenade-world-check SCENE".into());
    }
    let g = rc_game::bundle::decode(&fs::read(&args[1])?)?;
    for p in [[-2185.94, -6129.42, -2363.5], [-2098.5, -8209.39, -2363.5]] {
        println!(
            "{}",
            serde_json::to_string(&g.world.body_placement(p, g.profile.physics.extent)?)?
        );
    }
    Ok(())
}
