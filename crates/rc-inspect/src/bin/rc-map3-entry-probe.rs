use std::fs;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut g = rc_game::bundle::decode(&fs::read(
        "analysis/mission19-map3/delivery/geo_01c.rcplay",
    )?)?;
    g.restore_handoff(&serde_json::from_slice(&fs::read(
        "analysis/mission19-map3/delivery/playthrough/map2-handoff.json",
    )?)?)?;
    for i in 0..360 {
        g.advance(rc_game::STEP, rc_game::Input::default())?;
        if i % 30 == 0 {
            println!(
                "{i}: {:?} grounded {} health {}",
                g.state.pawn.body.position, g.state.pawn.body.grounded, g.state.health
            );
        }
    }
    Ok(())
}
