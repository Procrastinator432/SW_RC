use super::*;
use serde_json::Value;
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../fixtures/music-decisions-native.json"
    ))
    .unwrap()
}
#[test]
fn original_random_seed_and_512_draws_survive_cold_state() {
    let data: Value =
        serde_json::from_str(include_str!("../../../fixtures/music-random-native.json")).unwrap();
    for row in data["sequences"].as_array().unwrap() {
        let mut rng = Random {
            seed: row["initial"].as_u64().unwrap() as u32,
            draws: 0,
        };
        for step in row["steps"].as_array().unwrap() {
            assert_eq!(
                rng.sample().unwrap().to_bits(),
                step["value_bits"].as_u64().unwrap() as u32
            );
            assert_eq!(rng.seed, step["seed"].as_u64().unwrap() as u32);
            assert_eq!(rng.draws, step["draws"].as_u64().unwrap());
            rng = serde_json::from_slice(&serde_json::to_vec(&rng).unwrap()).unwrap();
        }
    }
}
#[test]
fn original_public_episode_gate_and_overlay_clear_match_128_calls() {
    for (i, row) in fixture()["episodes"].as_array().unwrap().iter().enumerate() {
        let mut state: Episode = serde_json::from_value(row["initial"].clone()).unwrap();
        let accepted = state.submit(
            row["ready"].as_bool().unwrap(),
            row["incoming"].as_u64().unwrap() as u32,
        );
        assert_eq!(
            serde_json::to_value(&state).unwrap(),
            row["expected"]["state"],
            "case {i}"
        );
        assert_eq!(
            u32::from(accepted),
            row["expected"]["flushes"].as_u64().unwrap() as u32
        );
    }
}
#[test]
fn original_battle_predicate_and_collector_match_1280_cases() {
    for (i, row) in fixture()["battle"].as_array().unwrap().iter().enumerate() {
        let input: Battle = serde_json::from_value(row["input"].clone()).unwrap();
        let enemies: Vec<Enemy> = serde_json::from_value(row["enemies"].clone()).unwrap();
        assert_eq!(
            serde_json::to_value(battle(&input, &enemies).unwrap()).unwrap(),
            row["expected"],
            "case {i}"
        );
    }
}
#[test]
fn original_group_selection_and_cache_words_match_including_float_boundaries() {
    for (i, row) in fixture()["groups"].as_array().unwrap().iter().enumerate() {
        let mut group: Group = serde_json::from_value(row["initial"].clone()).unwrap();
        let members: Vec<Member> = serde_json::from_value(row["members"].clone()).unwrap();
        for (j, step) in row["steps"].as_array().unwrap().iter().enumerate() {
            let random = group.requires_draw(members.len());
            let selected = group
                .select(
                    &members,
                    step["campaign"].as_u64().unwrap() as u8,
                    step["flags"].as_u64().unwrap() as u32,
                    step["draw"].as_f64().unwrap() as f32,
                )
                .unwrap();
            assert_eq!(
                selected,
                u32::try_from(step["expected"]["selected"].as_u64().unwrap()).unwrap(),
                "case {i}/{j}"
            );
            assert_eq!(
                serde_json::to_value(&group).unwrap(),
                step["expected"]["state"],
                "case {i}/{j}"
            );
            assert_eq!(
                usize::from(random),
                step["expected"]["random_calls"].as_u64().unwrap() as usize
            );
            group = serde_json::from_slice(&serde_json::to_vec(&group).unwrap()).unwrap();
        }
    }
}
fn base() -> Battle {
    Battle {
        ready: true,
        player: true,
        player_dead: false,
        player_location: [0.; 3],
        current: false,
        force_context: false,
        zone: false,
        player_in_zone: false,
        factories_exhausted: true,
        criteria: 1,
        threshold: 2,
        inner: 5.,
        outer: 10.,
    }
}
fn enemy() -> Enemy {
    Enemy {
        ai: true,
        pawn: true,
        hostile: true,
        dead: false,
        attack_flag: false,
        in_zone: false,
        location: [3., 4., 0.],
    }
}
#[test]
fn strict_radius_existing_threshold_and_forced_context_are_distinct() {
    let mut w = base();
    let mut e = enemy();
    assert!(!battle(&w, &[e.clone()]).unwrap().active);
    w.current = true;
    assert!(battle(&w, &[e.clone()]).unwrap().active);
    w.current = false;
    w.force_context = true;
    assert!(!battle(&w, &[e.clone()]).unwrap().active);
    e.location = [0.; 3];
    assert!(battle(&w, &[e]).unwrap().active);
}
#[test]
fn attack_flag_is_not_fabricated_from_proximity_or_health() {
    let mut w = base();
    w.criteria = 0;
    w.threshold = 1;
    let mut e = enemy();
    e.location = [0.; 3];
    assert!(!battle(&w, &[e.clone()]).unwrap().active);
    e.attack_flag = true;
    e.dead = true;
    assert!(battle(&w, &[e.clone()]).unwrap().active);
    w.criteria = 2;
    assert!(!battle(&w, &[e]).unwrap().active);
}
#[test]
fn battle_zone_holds_for_factories_but_not_for_missing_player() {
    let mut w = base();
    w.current = true;
    w.zone = true;
    w.player_in_zone = true;
    w.factories_exhausted = false;
    assert!(battle(&w, &[]).unwrap().active);
    w.player = false;
    assert!(!battle(&w, &[]).unwrap().active);
}
#[test]
fn invalid_observations_and_selection_are_transactional() {
    let mut w = base();
    w.inner = f32::NAN;
    assert!(battle(&w, &[]).is_err());
    let mut g: Group = serde_json::from_value(fixture()["groups"][1]["initial"].clone()).unwrap();
    let before = g.clone();
    assert!(g.select(&[], 0, 0, f32::NAN).is_err());
    assert_eq!(g, before);
    let mut g = Group {
        style: 1,
        first: 0,
        flags: 0,
        rendered: 0,
        total_bits: 0,
        which: 5,
    };
    let member = Member {
        sound: 1,
        context: 0,
        min: 0,
        max: 255,
        weight: 1.,
    };
    assert!(g.select(&[member], 0, 0, 0.5).is_err());
}
