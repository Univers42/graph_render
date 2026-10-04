//! The reference steps this port is pinned on, one test per step, each written so that it
//! fails on its own.
//!
//! The layout these cover is not a re-derivation of anything: every number below is read off
//! Graphviz 16.1.0's `lib/sfdpgen/` and named to the line it comes from. The subject is the
//! spring-electrical **iteration** and the **prolongation**, which is where a port silently
//! diverges while still drawing something plausible.
//!
//! `prompts/jobs/sg-sfdp-collapse.md` defects a-e are a, b, c, d, e; f (the D10 record) is a
//! document, not a test. The last two tests are the collapse itself, on the two graphs the job
//! names: the gallery graph and a lattice. A port whose step control is wrong is finite,
//! deterministic and direction-independent, and only these two catch it.

use super::multilevel::Level;
use super::prolongation::{self, DELTA_SCALE, Lay};
use super::shape::{assert_spread, grid_edges, lesmis};
use super::solve::Solve;
use super::start;
use super::*;
use crate::layout::coords::probe;

/// One prolongation, at an explicit `k` so the test states the jitter scale it is checking.
///
/// The `seed` is the stream's: the jitter comes from the same `drand()` the random start is
/// drawn from, so a test that wants a particular draw seeds a generator and hands it over.
fn prolongate_onto(
    coarse: &[(f64, f64)],
    level: &Level,
    edges: &[(u32, u32)],
    k: f64,
    seed: u32,
) -> (Vec<f64>, Vec<f64>) {
    let lay = Lay {
        level,
        edges,
        count: level.pair.len() as u32,
        delta: prolongation::delta(k),
    };
    let (x, y): (Vec<f64>, Vec<f64>) = coarse.iter().copied().unzip();
    prolongation::prolongate(&x, &y, &lay, &mut start::Glibc::seeded(seed))
}

/// The two-node level every step test runs on: one edge, `K = 2`, so the force on node 0 is
/// `0.4` of attraction against `2.0` of repulsion and the node moves along `-x`.
///
/// The arithmetic is written out because the test asserts on it: with `p = -1`, `KP = K² = 4`,
/// `CRK = C/K = 0.1`, and `dist = 2`, so the first iteration's total force is `0.4 - 2·1 = -1.6`
/// and the second is `0.441 - 1.905 = -1.464`. Both are negative, so both nodes move along `-x`
/// and the two displacements add.
fn two_nodes() -> Solve {
    Solve::with_k(vec![0.0, 2.0], vec![0.0, 0.0], &[(0u32, 1u32)], 2.0)
}

/// The `L1` length of the move from `a` to `b`, which is the distance a node **travelled**
/// whatever direction it travelled in — the quantity the step size actually fixes, and the one
/// that separates "moved by the step" from "moved by the step divided by the force length
/// against a step that never cooled".
fn travelled(a: (&[f64], &[f64]), b: (&[f64], &[f64])) -> f64 {
    (a.0[0] - b.0[0]).abs()
        + (a.0[1] - b.0[1]).abs()
        + (a.1[0] - b.1[0]).abs()
        + (a.1[1] - b.1[1]).abs()
}

/// Defects a and b. The reference moves every vertex by the step **as it stands at the start of
/// that iteration** (`spring_electrical.c:638`), and below the coarsest level the step has no
/// adaptive branch at all (`:171-174`, `:1160-1161`), so each iteration moves `0.9` of the one
/// before. A port that divides the force by its length against the *initial* step instead
/// moves `0.1` every iteration — and never converges, because only the copy it stops on cools.
///
/// The first two assertions hold whatever the force model is, so they isolate a and b from
/// defect e. The third is the reference's own arithmetic on this level, and it pins e as well:
/// below `quadtree_size` the reference sums all pairs (`spring_electrical.c:543`), so the force
/// on node 0 is `-1.6` and the two moves add along `-x`.
#[test]
fn a_fine_level_moves_each_node_by_the_cooled_step() {
    let start = (&[0.0, 2.0][..], &[0.0, 0.0][..]);
    let mut one = two_nodes();
    one.relax(0.1, 1);
    let mut two = two_nodes();
    two.relax(0.1, 2);
    let after_one = (one.x.as_slice(), one.y.as_slice());
    let after_two = (two.x.as_slice(), two.y.as_slice());
    assert!(
        (travelled(start, after_one) - 0.2).abs() < 1e-12,
        "one iteration moved the pair by {}, want 0.1 each: the step is not the one given",
        travelled(start, after_one)
    );
    assert!(
        (travelled(after_one, after_two) - 0.18).abs() < 1e-12,
        "the second iteration moved the pair by {}, want 0.09 each: the step did not cool by 0.9",
        travelled(after_one, after_two)
    );
    assert!(
        (after_two.0[0] + 0.19).abs() < 1e-12,
        "two iterations moved node 0 to {}, want -0.19: the reference's arithmetic",
        after_two.0[0]
    );
}

/// Defect c. The reference's test is `step > tol` with `tol = 0.001` **absolute**
/// (`spring_electrical.c:47`, `:650`) inside a `do`-while, so a fine level runs
/// `ceil(log(0.001/0.1)/log(0.9)) = 44` iterations whatever `K` is. A port that tests
/// `step > tol / K` runs a `K`-dependent number instead: at `K = 2` it would run 50, so a
/// budget of 45 would still be moving when the budget ran out and would disagree with an
/// unlimited run.
#[test]
fn a_fine_level_converges_in_forty_four_iterations() {
    let run = |limit| {
        let mut solve = Solve::with_k(
            vec![0.0, 2.0, 4.0, 2.0],
            vec![0.0, 0.5, 0.0, 0.5],
            &[(0u32, 1u32), (1, 2), (2, 3), (3, 0)],
            2.0,
        );
        solve.relax(0.1, limit);
        (solve.x, solve.y)
    };
    let (done, forever) = (run(44), run(500));
    assert_eq!(
        (
            done.0.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            done.1.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        ),
        (
            forever.0.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            forever.1.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        ),
        "44 iterations did not finish the level: the stop test is not `step > tol`"
    );
    let short = run(43);
    assert!(
        short.0.iter().zip(&forever.0).any(|(a, b)| a != b),
        "43 iterations already agreed with 44: the level converges early"
    );
}

/// Defect d, part one. The reference's `prolongate` multiplies by `P`, then runs
/// `interpolate_coord` over the fine graph (`spring_electrical.c:837-840`): every node is
/// pulled halfway toward the mean of its neighbours, in CSR row order and **in place**, so a
/// node reads neighbours that a lower index has already moved. With `P` alone the two members
/// of a matched pair land exactly on one another; this is what separates them by a fraction of
/// `K` instead of by a jitter.
#[test]
fn prolongation_pulls_a_node_toward_its_neighbours_mean() {
    let level = Level {
        pair: vec![0, 1, 2],
        coarse: 3,
    };
    let (x, _) = prolongate_onto(
        &[(10.0, 0.0), (20.0, 0.0), (30.0, 0.0)],
        &level,
        &[(0u32, 1u32), (1, 2)],
        0.0,
        1,
    );
    // `P` gives `[10, 20, 30]`. Then on the path `0-1-2`, in row order: node 0 takes the mean of
    // itself and node 1, `0.5·10 + 0.5·20 = 15`; node 1 sees the **moved** node 0 and takes
    // `0.5·20 + 0.25·(15 + 30) = 21.25`; node 2 sees the moved node 1 and takes
    // `0.5·30 + 0.5·21.25 = 25.625`.
    assert!((x[0] - 15.0).abs() < 1e-12, "node 0 at {}", x[0]);
    assert!((x[1] - 21.25).abs() < 1e-12, "node 1 at {}", x[1]);
    assert!(
        (x[2] - 25.625).abs() < 1e-12,
        "node 2 at {}: interpolate_coord ran Jacobi, not Gauss-Seidel",
        x[2]
    );
}

/// Defect d, part two. The reference's third step adds `K·0.001·(drand() - 0.5)` per coordinate
/// to **every member of an `R` row after the first** (`spring_electrical.c:843-852`), which is
/// what stops a matched pair from being bit-coincident. The port it replaced used a ±5e-7
/// absolute jiggle, five hundred times smaller at `K = 1` and independent of the level's scale:
/// on `lesmis` that is what left 40 pairs of output points within `1e-6` of the drawing's span.
///
/// The bound is the assertion: `delta/2` per coordinate, so the pair's separation is at most
/// `delta·√2/2` and at least a fraction of `delta`. Both ends matter — a jitter that were zero
/// would leave the pair stacked, and one that were unbounded would move the drawing.
#[test]
fn prolongation_jitters_a_matched_pair_by_the_reference_s_own_scale() {
    let delta = 0.1f64;
    let k = delta / DELTA_SCALE;
    let level = Level {
        pair: vec![0, 0],
        coarse: 1,
    };
    let (x, y) = prolongate_onto(&[(10.0, 30.0)], &level, &[(0u32, 1u32)], k, 1);
    // The pair is coincident after `P` and after `interpolate_coord` (a two-node level pulls
    // each node to the mean, which is the point itself), so all of the separation is the jitter.
    let (dx, dy) = (x[0] - x[1], y[0] - y[1]);
    let apart = dx.hypot(dy);
    assert!(
        apart > 0.0 && apart <= delta * 0.708,
        "the matched pair is {apart} apart, want at most {}",
        delta * 0.708
    );
    assert!(
        apart > delta / 4.0,
        "the matched pair is {apart} apart, which is jiggle-scale not delta-scale"
    );
}

/// The collapse itself, on the graph the row is measured on. `lesmis` is where the motor read
/// as a one-dimensional strip with 40 coincident pairs; Graphviz's own answer has a shape
/// ratio of 0.414 (small over large eigenvalue of the 2-D covariance), so 0.15 is the floor
/// that separates "a layout" from "a line".
#[test]
fn the_layout_spreads_on_lesmis() {
    let (nodes, edges) = lesmis();
    let points = probe::points(&run(&probe::graph(nodes, &edges)).expect("lesmis lays out"));
    assert_spread(&points);
}

/// The same two properties on a lattice, which is the case the studio shows when the collapse
/// is invisible: a grid has an unambiguous answer, so a strip means the solver, not the graph.
#[test]
fn the_layout_spreads_on_a_ten_by_ten_grid() {
    let edges = grid_edges(10);
    let points = probe::points(&run(&probe::graph(100, &edges)).expect("the grid lays out"));
    assert_spread(&points);
}
