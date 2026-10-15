//! Runtime ULevel::FindSpot (1043f3b0) / CheckSlice (1043ef90).
//! World queries remain host operations, including their native 0x86 filtering.
//! Editor placement and FarMoveActor's actor/hash/callback work are not implemented.
pub type Vector = [f32; 3];

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub actor: bool,
    pub fraction: f32,
    pub location: Vector,
    pub normal: Vector,
}

pub trait World {
    /// EncroachingWorldGeometry: true means blocked, unlike SinglePointCheck.
    fn encroaching(&mut self, point: Vector, extent: Vector) -> Result<bool, String>;
    /// SingleLineCheck with null source actor and flags 0x86.
    fn trace(&mut self, start: Vector, end: Vector, extent: Vector) -> Result<Hit, String>;
}

#[allow(clippy::assign_op_pattern)] // Preserve native SSE operand order.
fn check_slice(
    point: &mut Vector,
    extent: Vector,
    world: &mut impl World,
) -> Result<(bool, bool), String> {
    let slice = [extent[0], extent[1], 1.];
    if world.encroaching(*point, slice)? {
        return Ok((false, true));
    }
    let down = [point[0], point[1], point[2] - extent[2] * 2.];
    let hit = world.trace(*point, down, slice)?;
    let upward = hit.actor && hit.fraction <= 0.5;
    if upward {
        let distance = (0.5 - hit.fraction) * 2.;
        // DAT_1065efcc is +0.0; retain its MUL/ADD rather than eliding it.
        let xy = distance * 0.;
        point[0] += xy;
        point[1] = xy + point[1];
        point[2] = (extent[2] * distance + point[2]) + 1.;
    } else if hit.actor {
        let distance = hit.fraction * 2. - 1.;
        let xy = distance * 0.;
        point[0] -= xy;
        point[1] -= xy;
        point[2] = (point[2] - extent[2] * distance) + 1.;
    } else {
        point[2] -= extent[2];
    }
    if !world.encroaching(*point, extent)? {
        if !upward {
            let up = [point[0], point[1], extent[2] + point[2]];
            let hit = world.trace(*point, up, extent)?;
            if hit.actor {
                *point = hit.location;
            }
        }
        return Ok((true, false));
    }
    point[0] += hit.normal[0] * extent[0];
    point[1] = hit.normal[1] * extent[0] + point[1];
    point[2] = extent[0] * 0. + point[2];
    Ok((!world.encroaching(*point, extent)?, false))
}

/// Mutates the candidate even on failure, as the original does. FarMoveActor
/// keeps this temporary separate from Actor.Location until successful commit.
pub fn find_spot(
    point: &mut Vector,
    extent: Vector,
    world: &mut impl World,
) -> Result<bool, String> {
    if point.iter().chain(extent.iter()).any(|v| !v.is_finite()) || extent.iter().any(|v| *v < 0.) {
        return Err("Invalid placement input".into());
    }
    if !world.encroaching(*point, extent)? {
        return Ok(true);
    }
    if extent == [0.; 3] {
        return Ok(false);
    }
    let original = *point;
    let (success, corners) = check_slice(point, extent, world)?;
    if success || !corners {
        return Ok(success);
    }
    *point = original;
    let slice = [extent[0] * 0.5, extent[1] * 0.5, 1.];
    let mut clear = 0;
    for x in [-1., 1.] {
        for y in [-1., 1.] {
            if clear >= 2 {
                continue;
            }
            let offset = [x * extent[0] * 0.55, y * extent[1] * 0.55];
            let probe = [
                original[0] + offset[0],
                original[1] + offset[1],
                original[2],
            ];
            if !world.encroaching(probe, slice)? {
                clear += 1;
                point[0] += offset[0];
                point[1] += offset[1];
            }
        }
    }
    if clear == 0 {
        return Ok(false);
    }
    if clear == 1 {
        *point = std::array::from_fn(|i| point[i] * 2. - original[i]);
    }
    if world.trace(original, *point, [0.; 3])?.actor || world.encroaching(*point, extent)? {
        return Ok(false);
    }
    let back = std::array::from_fn(|i| original[i] + (original[i] - point[i]) * 0.2);
    let hit = world.trace(*point, back, extent)?;
    if hit.actor {
        *point = hit.location;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    enum Query {
        Point(Vector, Vector, bool),
        Line(Vector, Vector, Vector, Hit),
    }
    struct Fixture(VecDeque<Query>);
    impl World for Fixture {
        fn encroaching(&mut self, p: Vector, e: Vector) -> Result<bool, String> {
            let Some(Query::Point(wp, we, blocked)) = self.0.pop_front() else {
                panic!("Unexpected point query");
            };
            assert_eq!((p, e), (wp, we));
            Ok(blocked)
        }
        fn trace(&mut self, s: Vector, e: Vector, extent: Vector) -> Result<Hit, String> {
            let Some(Query::Line(ws, we, wx, hit)) = self.0.pop_front() else {
                panic!("Unexpected line query");
            };
            assert_eq!((s, e, extent), (ws, we, wx));
            Ok(hit)
        }
    }
    fn hit(actor: bool, fraction: f32, location: Vector, normal: Vector) -> Hit {
        Hit {
            actor,
            fraction,
            location,
            normal,
        }
    }
    fn run(start: Vector, extent: Vector, queries: Vec<Query>, ok: bool, end: Vector) {
        let mut fixture = Fixture(queries.into());
        let mut point = start;
        assert_eq!(find_spot(&mut point, extent, &mut fixture).unwrap(), ok);
        assert_eq!(point, end);
        assert!(fixture.0.is_empty());
    }
    #[test]
    fn clear_and_blocked_zero_extent_do_not_probe_slices() {
        for blocked in [false, true] {
            run(
                [1.; 3],
                [0.; 3],
                vec![Query::Point([1.; 3], [0.; 3], blocked)],
                !blocked,
                [1.; 3],
            );
        }
    }
    #[test]
    fn lower_half_hit_moves_up_without_extra_sweep() {
        let p = [10., 20., 30.];
        let e = [50., 50., 100.];
        run(
            p,
            e,
            vec![
                Query::Point(p, e, true),
                Query::Point(p, [50., 50., 1.], false),
                Query::Line(
                    p,
                    [10., 20., -170.],
                    [50., 50., 1.],
                    hit(true, 0.25, [0.; 3], [0., 0., 1.]),
                ),
                Query::Point([10., 20., 81.], e, false),
            ],
            true,
            [10., 20., 81.],
        );
    }
    #[test]
    fn no_floor_moves_down_then_uses_upward_sweep_hit_location() {
        let p = [0., 0., 200.];
        let e = [40., 40., 84.];
        let down = [0., 0., 116.];
        run(
            p,
            e,
            vec![
                Query::Point(p, e, true),
                Query::Point(p, [40., 40., 1.], false),
                Query::Line(
                    p,
                    [0., 0., 32.],
                    [40., 40., 1.],
                    hit(false, 1., [0.; 3], [0.; 3]),
                ),
                Query::Point(down, e, false),
                Query::Line(down, p, e, hit(true, 0.5, [0., 0., 158.], [0., 0., -1.])),
            ],
            true,
            [0., 0., 158.],
        );
    }
    #[test]
    fn failed_full_extent_uses_normal_radius_and_keeps_failed_candidate() {
        let p = [0., 0., 200.];
        let e = [40., 40., 84.];
        run(
            p,
            e,
            vec![
                Query::Point(p, e, true),
                Query::Point(p, [40., 40., 1.], false),
                Query::Line(
                    p,
                    [0., 0., 32.],
                    [40., 40., 1.],
                    hit(true, 0.75, [0.; 3], [1., 0., 0.]),
                ),
                Query::Point([0., 0., 159.], e, true),
                Query::Point([40., 0., 159.], e, true),
            ],
            false,
            [40., 0., 159.],
        );
    }
    #[test]
    fn one_clear_corner_is_doubled_and_full_extent_sweep_clamps() {
        let p = [0.; 3];
        let e = [100., 100., 100.];
        let slice = [50., 50., 1.];
        let d = 100f32 * 0.55;
        let c = [-d * 2., -d * 2., 0.];
        let back = [-c[0] * 0.2, -c[1] * 0.2, 0.];
        run(
            p,
            e,
            vec![
                Query::Point(p, e, true),
                Query::Point(p, [100., 100., 1.], true),
                Query::Point([-d, -d, 0.], slice, false),
                Query::Point([-d, d, 0.], slice, true),
                Query::Point([d, -d, 0.], slice, true),
                Query::Point([d, d, 0.], slice, true),
                Query::Line(p, c, [0.; 3], hit(false, 1., c, [0.; 3])),
                Query::Point(c, e, false),
                Query::Line(c, back, e, hit(true, 0.5, [-50., -50., 0.], [0.; 3])),
            ],
            true,
            [-50., -50., 0.],
        );
    }
    #[test]
    fn invalid_input_has_no_world_side_effects() {
        let mut world = Fixture(VecDeque::new());
        for (mut p, e) in [([f32::NAN, 0., 0.], [1.; 3]), ([0.; 3], [-1., 1., 1.])] {
            assert!(find_spot(&mut p, e, &mut world).is_err());
        }
    }
    #[test]
    fn two_clear_corners_stop_probing_and_blocked_line_preserves_candidate() {
        let p = [0.; 3];
        let e = [100.; 3];
        let d = 100f32 * 0.55;
        let candidate = [-d - d, 0., 0.];
        run(
            p,
            e,
            vec![
                Query::Point(p, e, true),
                Query::Point(p, [100., 100., 1.], true),
                Query::Point([-d, -d, 0.], [50., 50., 1.], false),
                Query::Point([-d, d, 0.], [50., 50., 1.], false),
                Query::Line(
                    p,
                    candidate,
                    [0.; 3],
                    hit(true, 0.5, [-d, 0., 0.], [1., 0., 0.]),
                ),
            ],
            false,
            candidate,
        );
    }
    #[test]
    fn half_fraction_is_upward_and_full_extent_failure_can_slide_along_normal() {
        let p = [0., 0., 200.];
        let e = [40., 40., 84.];
        run(
            p,
            e,
            vec![
                Query::Point(p, e, true),
                Query::Point(p, [40., 40., 1.], false),
                Query::Line(
                    p,
                    [0., 0., 32.],
                    [40., 40., 1.],
                    hit(true, 0.5, [0.; 3], [0., -1., 0.]),
                ),
                Query::Point([0., 0., 201.], e, true),
                Query::Point([0., -40., 201.], e, false),
            ],
            true,
            [0., -40., 201.],
        );
    }
}
