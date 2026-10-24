//! Original map/actor audio import; no guessed triangle or portal connectivity.
use rc_package::{
    audio_geometry, audio_zones,
    defaults::Catalog,
    properties::{self, Value},
    Package,
};
pub fn read(
    pkg: &Package,
    data: &[u8],
    catalog: &Catalog,
    owners: &[i32],
    map: &str,
) -> Result<(rc_game::ambient_geometry::Assets, serde_json::Value), String> {
    let binding = rc_package::level::world_binding(pkg, data)?;
    let model = &pkg.exports[binding.model_export];
    let bsp = audio_geometry::read_model(pkg, data, model)?;
    let connectivity = audio_zones::read_connectivity(pkg, data, model)?;
    let distances = audio_zones::Distances::build(&connectivity)?;
    let level_info = pkg
        .exports
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            let class = pkg.object_path(e.class).ok()?;
            catalog
                .derives_from(&class, "Engine.LevelInfo")
                .ok()
                .filter(|v| *v)
                .map(|_| i as i32 + 1)
        })
        .ok_or("Missing original LevelInfo for audio region fallback")?;
    let mut actors = vec![];
    let mut receipts = vec![];
    for &owner in owners {
        let export = owner
            .checked_sub(1)
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|i| pkg.exports.get(i))
            .ok_or("Invalid audio actor owner")?;
        let class = pkg.object_path(export.class)?;
        let props = properties::read(pkg, data, export)?;
        let mode = catalog.instance(&class, &props, "SoundOcclusion", 0, map)?;
        let Value::Byte(mode_value) = mode.value else {
            return Err("Invalid original SoundOcclusion byte".into());
        };
        let mover = catalog.derives_from(&class, "Engine.Mover")?;
        actors.push(rc_game::ambient_geometry::Actor {
            owner,
            mode: mode_value,
            mover,
        });
        receipts.push(
            serde_json::json!({"owner":owner,"class":class,"sound_occlusion":mode,"mover":mover}),
        );
    }
    let assets = rc_game::ambient_geometry::Assets {
        bsp,
        connectivity,
        distances,
        level_info,
        actors,
    };
    assets.validate()?;
    let receipt = serde_json::json!({"model":binding.model_path,"level_info":level_info,"actors":receipts,
        "connectivity":"FZoneProperties.Connectivity (not Visibility, not ZoneActor object refs)",
        "table":"LoadMap 1041f33e..1041f4bc, directed relation squared each iteration",
        "trace_boundary":"StaticMeshes mode rejects import until original mask-0x286 general trace host is available"});
    Ok((assets, receipt))
}
