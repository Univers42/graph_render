//! The parallel fan at the two boundaries it exists for: a pair of edges between the
//! same two nodes, and the shipped fixture's four groups. Split from the parent, which
//! holds the case builders.

use super::*;

/// One `parallel_offset` step, the fan's spacing.
const FAN_SPACING: f32 = 0.05;

/// The slack a fan step is compared at: large enough for the `f32` the row was
/// written through, orders of magnitude below the step itself, so it can only ever
/// hide a representation difference and never a real one.
const FAN_SLACK: f32 = 1e-6;
/// A parallel pair is the boundary the fan exists for: two edges between the same
/// two nodes, whose rows must differ. A zero offset — or a fan keyed on direction
/// instead of on the unordered pair — makes them equal, and only this sees it.
///
/// Two cases, because the perpendicular is taken two different ways: nodes at
/// different positions, where the fan turns the chord's canonical direction, and two
/// nodes a layout put on one spot, where there is no chord and the fan slides the two
/// self-loops along `y` instead.
///
/// `Straight` is the documented exception and is asserted rather than skipped:
/// `EdgeGeometry::Line` stores no interior point, so there is nowhere for a gap to
/// live and the two chords are the same chord. `style_edges` refuses a non-zero
/// `parallel_offset` for that style rather than pretending otherwise, so the trade
/// is a refusal, never a silent overlay.
#[test]
fn a_parallel_pair_never_overlays_at_any_style() {
    let cases = [
        parallel_pair(),
        Case::new(
            "two nodes at one position",
            &[(0.0, 0.0), (0.0, 0.0)],
            &[("a", "b"), ("b", "a")],
        ),
    ];
    for case in &cases {
        for style in Style::ALL {
            let edges = styled(case, style, 0);
            if style == Style::Straight {
                assert_eq!(edges, EdgeGeometry::Line, "{}: {style:?}", case.name);
                continue;
            }
            let all = rows_of(case, style);
            assert!(
                all[0] != all[1],
                "{}: {style:?}: a parallel pair overlaid: {:?}",
                case.name,
                all[0]
            );
        }
    }
}

/// The shipped fixture at every style. Three things have to hold at once, and the
/// fixture is built so that each is only visible with the other two in place:
///
/// - the four-edge p–q group fans to four distinct, evenly spaced rows, one
///   `parallel_offset` apart;
/// - the two-edge r–s group also fans to two, even though `e1` and `e4` sit *between*
///   the p–q edges. A fan that treated each endpoint's edges as one contiguous run would
///   read r–s as two groups of one and leave both its edges on the chord — which is why
///   the fan is two chained counting sorts and not one;
/// - t's two self-loops are a parallel group of their own, and u–p is a group of one,
///   left exactly where it was — the strongest form of "takes no offset", checked against
///   the same edge styled alone rather than against a hand-written number.
#[test]
fn the_parallel_edges_fixture_fans_every_group_once() {
    let case = Case::fixture();
    assert_eq!(case.topology.edge_count(), 9, "the fixture's nine edges");
    // u -> p with u at the *higher* dense index as it is in the fixture, so the
    // canonical direction matches too and the two rows compare point for point.
    let u_to_p = Case::new("u-p alone", &[(0.0, 0.0), (20.0, 0.0)], &[("b", "a")]);
    for style in Style::ALL {
        if style == Style::Straight {
            continue;
        }
        let all = rows_of(&case, style);
        assert_eq!(distinct(&all, &P_Q), 4, "{style:?}: p-q is a group of four");
        assert_eq!(
            distinct(&all, &R_S),
            2,
            "{style:?}: r-s is an interleaved pair"
        );
        assert_eq!(
            distinct(&all, &T_T),
            2,
            "{style:?}: t's self-loops are a pair"
        );
        assert_eq!(
            all[8],
            rows_of(&u_to_p, style)[0],
            "{style:?}: u-p is a group of one and takes no offset"
        );
        let steps = fan_steps(&all);
        assert!(
            (steps[0] - steps[1]).abs() <= FAN_SLACK && (steps[1] - steps[2]).abs() <= FAN_SLACK,
            "{style:?}: the p-q fan is not evenly spaced: {steps:?}"
        );
        assert!(
            (steps[0] - FAN_SPACING).abs() <= FAN_SLACK,
            "{style:?}: the fan step is not one parallel_offset: {steps:?}"
        );
    }
}

/// The fixture's p–q group: `e0`, `e2`, `e3`, `e5` — interleaved with r–s.
const P_Q: [u32; 4] = [0, 2, 3, 5];
/// The fixture's r–s group: `e1` and `e4`, interleaved into the middle of p–q.
const R_S: [u32; 2] = [1, 4];
/// The fixture's self-loop group on t: `e6` and `e7`.
const T_T: [u32; 2] = [6, 7];

/// How many distinct rows `group` holds: a group of `k` whose edges all fanned to the
/// same offset is `k` identical rows, which is the overlay this file exists to see.
fn distinct(all: &[Vec<(f32, f32)>], group: &[u32]) -> usize {
    let mut keys: Vec<String> = group
        .iter()
        .map(|&e| format!("{:?}", all[e as usize]))
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys.len()
}

/// The three gaps between the p–q group's four rows, ascending, each written as
/// `second - first`.
///
/// Every chord in the fixture is horizontal, so the fan displacement is purely vertical,
/// and a reversed pair bulges to the same side — the four rows' first `y` values are
/// therefore the fan's own, with no bulge mixed in. A centred fan of four is evenly
/// spaced, and every gap is exactly one `parallel_offset`.
fn fan_steps(all: &[Vec<(f32, f32)>]) -> [f32; 3] {
    let mut ys: Vec<f32> = P_Q.iter().map(|&e| all[e as usize][0].1).collect();
    ys.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    let steps: Vec<f32> = ys.windows(2).map(|w| w[1] - w[0]).collect();
    [steps[0], steps[1], steps[2]]
}
