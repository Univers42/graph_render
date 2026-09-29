//! The per-pair forces: which pair is pushed apart, which is only weakly repelled, which
//! is left at rest, and the temperature cap and settle rounds that follow.

use super::super::super::{nudge, separated};
use super::super::{
    cap_and_apply, edge_key_set, overlap_and_repel, settle, settle_round, settle_rounds,
};
use super::{bits, field_for};

#[test]
fn a_close_pair_that_is_not_an_edge_is_weakly_repelled() {
    // Not overlapping, not an edge, but within the cutoff: the weak `0.1 * sums / (d^2
    // + 0.1)` term applies. With an edge instead, the term is exactly zero.
    let radii = [0.5, 0.5];
    let positions = [(0.0, 0.0), (1.2, 0.0)];
    let without = field_for(&[], 2, &radii, 2.0);
    let mut forces = vec![(0.0, 0.0); 2];
    overlap_and_repel(&positions, &without, &mut forces);
    assert!(forces[0].0 < 0.0, "repelled apart: {:?}", forces[0]);
    let expected: f64 = 0.1 * 1.0 / (1.2 * 1.2 + 0.1) * (1.2 / 1.2);
    assert_eq!((-forces[0].0).to_bits(), expected.to_bits());

    let keys = edge_key_set(2, &[(0, 1)]);
    let with = field_for(&keys, 2, &radii, 2.0);
    let mut forces = vec![(0.0, 0.0); 2];
    overlap_and_repel(&positions, &with, &mut forces);
    assert_eq!(
        forces[0],
        (0.0, 0.0),
        "a graph edge is neither pushed nor repelled"
    );
}

#[test]
fn overlap_and_repel_pushes_overlaps_apart_and_leaves_tangencies_and_edges_alone() {
    let radii = [0.5, 0.5, 0.5, 0.5, 0.5];
    // 0-1 overlap by 0.5; 2-3 are exactly tangent and share an edge; 4 is far beyond the
    // cutoff, so no pair it is in contributes.
    let edges = [(2, 3)];
    let positions = [(0.0, 0.0), (0.5, 0.0), (0.0, 5.0), (1.0, 5.0), (100.0, 0.0)];
    let keys = edge_key_set(5, &edges);
    let field = field_for(&keys, 5, &radii, 2.0);
    let mut forces = vec![(0.0, 0.0); 5];
    overlap_and_repel(&positions, &field, &mut forces);

    // 0,1 overlap by `sums - dist = 0.5`, so each is pushed `(sums - dist) * 0.5` along
    // the axis, away from the other, and the two are equal and opposite.
    assert_eq!(
        forces[0].0.to_bits(),
        (-0.25_f64).to_bits(),
        "{:?}",
        forces[0]
    );
    assert_eq!(forces[1].0.to_bits(), 0.25_f64.to_bits(), "{:?}", forces[1]);
    assert_eq!(forces[0].0.to_bits(), (-forces[1].0).to_bits());
    assert_eq!(forces[0].1.to_bits(), 0.0_f64.to_bits());
    assert_eq!(forces[1].1.to_bits(), 0.0_f64.to_bits());
    // 2,3 touch exactly (`dist == radii sum`) and share an edge: neither push nor repel.
    assert_eq!(
        forces[2],
        (0.0, 0.0),
        "an exact tangency on an edge is at rest"
    );
    assert_eq!(forces[3], (0.0, 0.0));
    // 4 is beyond the cutoff, so no pair involving it contributes at all.
    assert_eq!(forces[4], (0.0, 0.0));
}

#[test]
fn a_pair_exactly_tangent_is_neither_pushed_nor_repeled_by_the_overlap_pass() {
    // `dist < sums` decides overlap (`circle_packing.py:470`): at exactly the radii sum
    // the circles touch, which is not an overlap, so the pair gets the weak term (or, on
    // an edge, nothing).
    let radii = [0.5, 0.5];
    let positions = [(0.0, 0.0), (1.0, 0.0)];
    let keys = edge_key_set(2, &[]);
    let field = field_for(&keys, 2, &radii, 2.0);
    let mut touching = vec![(0.0, 0.0); 2];
    overlap_and_repel(&positions, &field, &mut touching);
    let expected: f64 = 0.1 * 1.0 / (1.0 * 1.0 + 0.1);
    assert_eq!(
        (-touching[0].0).to_bits(),
        expected.to_bits(),
        "a touching pair is only weakly repelled"
    );
    // A hair closer and it is an overlap, pushed apart by `(sums - dist) * 0.5` instead.
    let closer = [(0.0, 0.0), (libm::nextafter(1.0, 0.0), 0.0)];
    let mut overlapping = vec![(0.0, 0.0); 2];
    overlap_and_repel(&closer, &field, &mut overlapping);
    assert!(
        overlapping[0].0 < -touching[0].0,
        "an overlap is pushed harder than a touch: {:?} vs {:?}",
        overlapping[0],
        touching[0]
    );
}

#[test]
fn the_overlap_pass_stops_at_the_cutoff_but_the_settle_pass_does_not_need_to() {
    // Two different guards, and both matter: the overlap pass skips a pair at or beyond
    // its cutoff (`circle_packing.py:457`), while the settle round's own test is the
    // overlap alone (`circle_packing.py:507`) — a pair that is clear but within the
    // cutoff must still count as "nothing to settle", not as a pair to correct.
    let radii = [0.5, 0.5];
    let field = field_for(&[], 2, &radii, 1.5);
    let clear = [(0.0, 0.0), (1.2, 0.0)];
    let mut shifts = [(0.0, 0.0); 2];
    let moved = settle_round(&mut shifts, &field);
    assert!(
        !moved,
        "1.2 apart with radii summing to 1.0: nothing to settle"
    );
    assert_eq!(shifts, [(0.0, 0.0), (0.0, 0.0)]);
    // The same pair is inside the overlap pass's cutoff, so it does contribute there.
    let mut forces = vec![(0.0, 0.0); 2];
    overlap_and_repel(&clear, &field, &mut forces);
    assert!(forces[0].0 != 0.0, "within the cutoff, so weakly repelled");
}

#[test]
fn cap_and_apply_caps_at_the_temperature_but_leaves_a_small_force_alone() {
    let mut positions = [(0.0, 0.0), (0.0, 0.0), (0.0, 0.0)];
    // (3, 4) has magnitude 5: above the 1.0 cap, so it is scaled to length 1.
    cap_and_apply(&mut positions, &[(3.0, 4.0), (0.3, 0.4), (0.0, 0.0)], 1.0);
    assert_eq!(positions[0].0.to_bits(), 0.6_f64.to_bits());
    assert_eq!(positions[0].1.to_bits(), 0.8_f64.to_bits());
    // Below the cap: applied whole, not scaled.
    assert_eq!(positions[1].0.to_bits(), 0.3_f64.to_bits());
    assert_eq!(positions[1].1.to_bits(), 0.4_f64.to_bits());
    // A zero force must leave a position bit-for-bit unchanged, signs included.
    assert_eq!(positions[2], (0.0, 0.0));
}

#[test]
fn capping_divides_by_the_magnitude_and_not_by_the_temperature() {
    // The cap is `f * temperature / magnitude`. At a temperature of 1 the two orders
    // coincide by accident, so this pins the case where they do not.
    let mut positions = [(0.0f64, 0.0f64), (0.0, 0.0)];
    cap_and_apply(&mut positions, &[(3.0, 4.0), (-3.0, -4.0)], 2.0);
    // Both have magnitude 5, so both are scaled to 2. `3 * 2 / 5` and `3 / 2 / 5` differ,
    // so a division by the temperature instead would be caught.
    let want = |f: f64| (f * 2.0 / 5.0).to_bits();
    assert_eq!(positions[0].0.to_bits(), want(3.0));
    assert_eq!(positions[0].1.to_bits(), want(4.0));
    assert_eq!(positions[1].0.to_bits(), want(-3.0));
    assert_eq!(positions[1].1.to_bits(), want(-4.0));
}

#[test]
fn a_force_exactly_at_the_cap_is_left_uncapped() {
    // `magnitude > temperature` is strict, so a force whose magnitude is *exactly* the
    // temperature is applied whole. Rescaling it would be a no-op in value but not in
    // bits: `x * t / t` is not always `x`.
    let force = (-0.4812919713439847, -2.886492121120881);
    let magnitude = libm::hypot(force.0, force.1);
    let mut positions = [(0.0, 0.0), (0.0, 0.0)];
    cap_and_apply(&mut positions, &[force, force], magnitude);
    assert_eq!(positions[0].0.to_bits(), force.0.to_bits(), "not rescaled");
    assert_eq!(positions[0].1.to_bits(), force.1.to_bits(), "not rescaled");
    // Below that same magnitude, and it is capped — the move comes out at the cap rather
    // than at the force's own length.
    let mut under = [(0.0f64, 0.0f64)];
    let cap = magnitude * 0.5;
    cap_and_apply(&mut under, &[force], cap);
    let moved = libm::hypot(under[0].0, under[0].1);
    assert!(
        (moved - cap).abs() < 1e-12 && moved < magnitude,
        "capped to {moved} against a cap of {cap}"
    );
}

#[test]
fn a_settle_round_moves_only_overlapping_pairs_and_reports_whether_it_moved() {
    let radii = [0.5, 0.5, 0.5];
    let field = field_for(&[], 3, &radii, 2.0);
    // One overlapping pair, one clear: the overlapping pair splits by `0.55 * (sums -
    // dist) / dist` each, the clear pair is not touched at all.
    let mut positions = [(0.0, 0.0), (0.5, 0.0), (10.0, 0.0)];
    let moved = settle_round(&mut positions, &field);
    assert!(moved, "one overlapping pair is enough to count as a move");
    // `s * diff.0` with `s = 0.55 * (sums - dist) / dist` and `diff.0 = 0.5`, applied to
    // the positions the pair started at (0.0 and 0.5).
    let shift: f64 = 0.55 * (1.0 - 0.5) / 0.5 * 0.5;
    assert_eq!(positions[0].0.to_bits(), (0.0 - shift).to_bits());
    assert_eq!(positions[1].0.to_bits(), (0.5 + shift).to_bits());
    assert_eq!(positions[2], (10.0, 0.0), "a far pair is untouched");
    // And a second round on the now-separated packing moves nothing.
    assert!(!settle_round(&mut positions, &field));
}

#[test]
fn settle_runs_a_bounded_number_of_rounds_and_stops_as_soon_as_one_moves_nothing() {
    // The tightest pair among four on a line, read off a slice rather than a closure so
    // `positions` can still be mutated between calls.
    fn tightest(positions: &[(f64, f64)]) -> f64 {
        (0..positions.len())
            .flat_map(|i| ((i + 1)..positions.len()).map(move |j| (i, j)))
            .map(|(i, j)| {
                libm::hypot(
                    positions[i].0 - positions[j].0,
                    positions[i].1 - positions[j].1,
                )
            })
            .fold(f64::INFINITY, f64::min)
    }
    let radii = [0.5; 4];
    let field = field_for(&[], 4, &radii, 2.0);
    let mut positions = [(0.0, 0.0), (0.1, 0.0), (0.2, 0.0), (0.3, 0.0)];
    let before = tightest(&positions);
    // `iterations / 2` rounds at 20 is only 10, so the four circles are pushed apart but
    // not yet clear: the pass is bounded and does not run to convergence.
    settle(&mut positions, &field, 20);
    assert!(
        tightest(&positions) > before,
        "settling must separate the pile: {}",
        tightest(&positions)
    );

    // Given the round cap's worth it does reach it.
    let mut converged = [(0.0, 0.0), (0.1, 0.0), (0.2, 0.0), (0.3, 0.0)];
    settle(&mut converged, &field, 400);
    for i in 0..4 {
        for j in (i + 1)..4 {
            let dist = libm::hypot(
                converged[i].0 - converged[j].0,
                converged[i].1 - converged[j].1,
            );
            assert!(
                dist >= 1.0 - 1e-9,
                "circles {i},{j} still overlap at {dist} after settling"
            );
        }
    }
    // And once clear, a further pass is a no-op bit for bit: the loop stops on the round
    // that moves nothing rather than grinding out its whole budget.
    let once = converged;
    settle(&mut converged, &field, 400);
    assert_eq!(bits(&converged), bits(&once), "settled means settled");
}

#[test]
fn the_settle_round_cap_is_half_the_budget_floored_at_ten() {
    // SciGraphs' own `max(int(iterations) // 2, 10)` (`circle_packing.py:493`): integer
    // halving, not a float one, and the floor is on the *halved* budget, so a budget of
    // 40 gets 20 rounds and one of 19 still gets 10.
    for (iterations, want) in [
        (0u32, 10u32),
        (1, 10),
        (19, 10),
        (20, 10),
        (21, 10),
        (39, 19),
        (40, 20),
        (500, 250),
        (501, 250),
        (1000, 500),
    ] {
        assert_eq!(settle_rounds(iterations), want, "iterations = {iterations}");
    }
}

#[test]
fn a_coincident_pair_is_only_substituted_at_the_guards_own_threshold() {
    // `dist < 1e-6` is the reference's own `weak = dist < 1e-6` (`circle_packing.py:447`).
    // At exactly 1e-6 the measured pair is *not* weak, so it keeps its own direction; one
    // ULP below, it takes the deterministic substitute.
    let just_under: f64 = libm::nextafter(1e-6, 0.0);
    assert_eq!(
        separated((1e-6, 0.0), 1, 2),
        ((1e-6, 0.0), 1e-6),
        "at the threshold"
    );
    assert_eq!(
        separated((just_under, 0.0), 1, 2),
        (nudge(1, 2), 1.0),
        "below it"
    );
    assert_eq!(
        separated((2e-6, 0.0), 1, 2),
        ((2e-6, 0.0), 2e-6),
        "above it"
    );
}
