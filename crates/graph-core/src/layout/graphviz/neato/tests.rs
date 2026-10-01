//! What the reference's own arithmetic guarantees, asserted without a copy of it.
//!
//! **Nothing here is a number read out of the oracle.** Where a case has a closed answer —
//! the star's equal leaf radii, the path's equal hop spacing, the Laplacian's null vector —
//! it is derived from the stress model `lib/neatogen/stress.c` minimises and asserted as an
//! invariant, so the test states what the model says rather than what one run printed. The
//! only literals that come from outside this file are the `drand48` sequence, which is
//! the published algorithm (a 48-bit LCG), and the small cases, whose *scale* is inherited
//! from the initial placement and so has no closed value — only the ratios do.
//!
//! The cross-implementation comparison is the differential's job, not this file's:
//! `harness/oracle-graphviz.py` compares our coordinates with `neato -Tplain`'s byte for
//! byte on six closed cases and by rescaled gap over 1000 seeds
//! (`docs/measurements/p13-gv2-neato.md`).

use super::run;
use crate::layout::coords::probe::{graph, points};
use crate::layout::graphviz::neato::rng::Drand48;

/// `drand48` for seed 1, the first four draws: `(0x5DEECE66D * X + 0xB) mod 2^48` over
/// `X = (1 << 16) | 0x330E`, divided by `2^48`. Derived from the generator, not recorded
/// from a run — which is why the negative control below can be a *different seed* rather
/// than a tweaked constant.
const SEED_ONE: [f64; 4] = [
    0.041_630_344_771_878_214,
    0.454_492_444_728_629_15,
    0.834_817_218_166_914_9,
    0.335_986_030_145_200_23,
];

#[test]
fn an_empty_graph_has_no_points() {
    assert_eq!(points(&run(&graph(0, &[])).unwrap()), vec![]);
}

/// One node: `neatoLayout` returns before the majorization when `nG < 2`
/// (`neatoinit.c:1290`), so no coordinate is ever written and the node stays at the origin.
/// The oracle agrees: `neato -Tplain` on `graph g { n0; }` prints `node n0 0.375 0.25`,
/// which is the origin once the drawing is translated onto its bounding box.
#[test]
fn one_node_never_enters_the_iteration_and_sits_at_the_origin() {
    assert_eq!(points(&run(&graph(1, &[])).unwrap()), [(0.0, 0.0)]);
}

/// The generator is the reference's, and the seed is load-bearing: `-Gstart` is *not*
/// inert for this engine the way it is for twopi, because `initLayout` reads every
/// unpinned coordinate out of `drand48` (`stress.c:154-155`) and `checkStart` seeds it
/// from `start` (`neatoinit.c:989`). All 1000 differential fixtures draw differently at
/// `-Gstart` 1, 7 and 99 (`docs/measurements/p13-gv2-neato.md`).
#[test]
fn the_seeded_generator_is_the_reference_drand48() {
    let mut rng = Drand48::seeded(1);
    let drew: Vec<f64> = (0..4).map(|_| rng.next()).collect();
    assert_eq!(drew, SEED_ONE);
}

/// The negative control for the line above, and the reason the seed is reproduced at all:
/// a different seed is a different drawing from the first draw, not a perturbation of it.
#[test]
fn a_different_start_seed_draws_different_numbers() {
    let first: Vec<f64> = (0..4)
        .scan(Drand48::seeded(1), |r, _| Some(r.next()))
        .collect();
    let seventh: Vec<f64> = (0..4)
        .scan(Drand48::seeded(7), |r, _| Some(r.next()))
        .collect();
    assert_ne!(first, seventh);
    assert_eq!(first, SEED_ONE);
}

/// A 5-star's stress optimum puts every leaf at the *same* distance from the hub: all four
/// `d_ij` are 1 and the leaves are symmetric in the model, so any spread is a failure to
/// converge rather than a shape. Graphviz reaches 1.1029..1.1041 inches on its own, a
/// spread of 0.11%, which is the tolerance stated here times the room the stopping rule
/// leaves. The *scale* has no closed value — it is inherited from the initial placement —
/// so only the ratio is asserted.
#[test]
fn a_five_node_star_puts_its_leaves_at_one_radius() {
    let got = points(&run(&graph(5, &[(0, 1), (0, 2), (0, 3), (0, 4)])).unwrap());
    let (hub_x, hub_y) = got[0];
    let radii: Vec<f32> = got[1..]
        .iter()
        .map(|&(x, y)| distance((x, y), (hub_x, hub_y)))
        .collect();
    let (low, high) = spread(&radii);
    assert!(high > 0.0 && (high - low) / high < 5e-3, "{radii:?}");
}

/// A 3-path: the ends are two hops apart and the two adjacent pairs one, so the stress
/// optimum spaces the three nodes evenly. Graphviz's own run gives 1.0038, 1.0040 and
/// 1.9830 inches for the three distances — the adjacent pairs agree to 2e-4 and the long
/// pair is 0.85% short, which is what `Epsilon = 1e-4` on the *relative stress change*
/// leaves rather than a spacing error.
#[test]
fn a_three_node_path_spaces_its_nodes_by_hop_count() {
    let got = points(&run(&graph(3, &[(0, 1), (1, 2)])).unwrap());
    let near = distance(got[0], got[1]);
    let also_near = distance(got[1], got[2]);
    let far = distance(got[0], got[2]);
    assert!(near > 0.0, "{got:?}");
    assert!((near - also_near).abs() / near < 1e-2, "{got:?}");
    assert!((far - 2.0 * near).abs() / far < 2e-2, "{got:?}");
}

/// Every conjugate-gradient step begins by centring `x`, `b`, `p` and `r` against the
/// ones vector (`conjgrad.c:177-181`), so a converged drawing has both columns summing to
/// zero. The residual is `f32` accumulation, hence the tolerance and not `== 0.0`.
#[test]
fn the_drawing_is_centred_on_both_axes() {
    let got = points(&run(&graph(12, &ring(12))).unwrap());
    for axis in 0..2 {
        let total: f32 = got.iter().map(|p| if axis == 0 { p.0 } else { p.1 }).sum();
        assert!(
            total.abs() < 1e-3,
            "column {axis} sums to {total} over {got:?}"
        );
    }
}

/// A 4-cycle's stress optimum: an isosceles trapezoid, not a rhombus and not a square.
/// Graphviz's own run on this graph — sides 1.302, 0.812, 1.306, 0.809 inches and diagonals
/// 0.869, 1.202 — so the two **opposite sides** agree to 0.3% while the two pairs differ by
/// 38%, and the two diagonals differ from each other by 38% too.
///
/// Those numbers are the reference's, not this port's rounding: a "4-cycle is a square"
/// expectation is a different stress model. What the model constrains is the *pairing* —
/// opposite sides match — and the initial placement's random orientation is what breaks the
/// remaining symmetry, which is why the diagonals are unequal.
#[test]
fn a_four_cycle_is_a_trapezoid_whose_opposite_sides_agree() {
    let got = points(&run(&graph(4, &[(0, 1), (1, 2), (2, 3), (3, 0)])).unwrap());
    let sides = [
        distance(got[0], got[1]),
        distance(got[1], got[2]),
        distance(got[2], got[3]),
        distance(got[3], got[0]),
    ];
    let (short_side, long_side) = spread(&sides);
    assert!(short_side > 0.0, "{got:?}");
    for (a, b) in [(sides[0], sides[2]), (sides[1], sides[3])] {
        assert!((a - b).abs() / long_side < 1e-2, "{got:?}: {sides:?}");
    }
    // And the pairs are genuinely different lengths, so the agreement above is a property of
    // the drawing rather than of every side having come out the same.
    assert!(
        (long_side - short_side) / long_side > 0.3,
        "{got:?}: {sides:?}"
    );
}

/// **The stress model is invariant under translation and rotation, and the port's answer is
/// not** — because the initial placement is random, the drawing it converges to depends on
/// the seed, and a rotated seed converges to a rotated drawing.
///
/// This is stated as a test because it is the property most likely to be *assumed* and it is
/// false here: there is no canonical orientation for this layout, so two callers laying out
/// the same graph get drawings related by a rotation, not by a translation. The differential
/// does not hit it — it compares against the oracle at the same seed, and its
/// bounding-box rescale removes the translation but **not** the rotation, which is why the
/// metric is a gap on a shared box and not a set-to-set distance.
///
/// What is asserted here is the consequence: the drawing is centred (both columns sum to
/// zero) and every node sits at a distinct point, and the two are checked on the same run so
/// a layout that collapsed everything onto the origin would fail the second.
#[test]
fn a_cycle_is_centred_and_places_every_node_distinctly() {
    let got = points(&run(&graph(9, &ring(9))).unwrap());
    for axis in 0..2 {
        let total: f32 = got.iter().map(|p| if axis == 0 { p.0 } else { p.1 }).sum();
        assert!(
            total.abs() < 1e-2,
            "column {axis} sums to {total} over {got:?}"
        );
    }
    for (i, a) in got.iter().enumerate() {
        for (j, b) in got.iter().enumerate().skip(i + 1) {
            assert!(
                distance(*a, *b) > 1e-2,
                "nodes {i} and {j} coincide at {a:?}"
            );
        }
    }
}

/// The whole point of a hash-gated stage, and the reason the negative controls can be
/// trusted: the same graph twice is the same bytes, with no wall-clock and no `HashMap`
/// anywhere in the path.
#[test]
fn the_same_graph_twice_gives_the_same_coordinates() {
    let edges = ring(11);
    assert_eq!(
        points(&run(&graph(11, &edges)).unwrap()),
        points(&run(&graph(11, &edges)).unwrap())
    );
}

/// A node no edge reaches is pushed `closestDist + 10` hops beyond its component instead
/// of being dropped or left at infinity (`bfs.c:47-49`), so the drawing still has a finite
/// place for it. This port is a Ponytail here and says so: it draws the unreachable node
/// where the reference does, but it does not reproduce the reference's *choice* of which
/// component's eccentricity to add ten to, because that choice is decided by the order a
/// breadth-first queue happens to empty in.
#[test]
fn an_isolated_node_lands_at_a_finite_point() {
    let got = points(&run(&graph(3, &[(0, 1)])).unwrap());
    for (index, &(x, y)) in got.iter().enumerate() {
        assert!(x.is_finite() && y.is_finite(), "node {index} at {x},{y}");
    }
}

/// A repeated edge is one hop, not two: `makeGraphData` folds duplicates through its
/// `PointMap` before the search ever sees them (`neatoinit.c:801-806`), so a doubled edge
/// must draw exactly like the single one. Without the fold the doubled edge would be
/// invisible to an unweighted search anyway, which is why this is a test and not a comment.
#[test]
fn a_parallel_edge_is_one_neighbour_and_draws_like_a_single_one() {
    let single = points(&run(&graph(3, &[(0, 1), (1, 2)])).unwrap());
    let doubled = points(&run(&graph(3, &[(0, 1), (0, 1), (1, 2)])).unwrap());
    assert_eq!(single, doubled);
}

/// A self-loop is dropped before the search (`neatoinit.c:794-795`), so a node that loops
/// to itself and to nothing else is an isolated node, not a component of one.
#[test]
fn a_self_loop_is_not_an_edge() {
    let got = points(&run(&graph(2, &[(0, 0)])).unwrap());
    assert_eq!(got.len(), 2);
    for &(x, y) in &got {
        assert!(x.is_finite() && y.is_finite(), "{got:?}");
    }
}

/// `(count)` nodes in a ring, the fixture the centredness and null-space cases share: one
/// connected cycle, no leaf, so nothing here depends on a tie-break.
fn ring(count: u32) -> Vec<(u32, u32)> {
    (0..count).map(|i| (i, (i + 1) % count)).collect()
}

/// The Euclidean distance between two points, in `f32` so it can be compared with a `f32`
/// tolerance.
fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = f64::from(a.0 - b.0);
    let dy = f64::from(a.1 - b.1);
    libm::hypot(dx, dy) as f32
}

/// A sample's `(min, max)`.
fn spread(sample: &[f32]) -> (f32, f32) {
    let low = sample.iter().copied().fold(f32::INFINITY, f32::min);
    let high = sample.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    (low, high)
}
