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

use super::multilevel::{self, Level};
use super::solve::Solve;
use super::*;
use crate::layout::coords::probe;
use graph_contract::canonical_json::{Value, parse};

/// The gallery graph, 77 nodes and 254 edges (`fixtures/scigraphs/lesmis.json`, the same file
/// `conformance/fixtures/named.rs` reads for the `GRAPHVIZ_SFDP` row).
const LESMIS: &str = include_str!("../../../../../../fixtures/scigraphs/lesmis.json");

/// The two-node level every step test runs on: one edge, `K = 2`, so the force on node 0 is
/// `0.4` of attraction against `2.0` of repulsion and the node moves along `-x`.
///
/// The arithmetic is written out because the test asserts on it: with `p = -1`, `KP = K² = 4`,
/// `CRK = C/K = 0.1`, and `dist = 2`, so the first iteration's total force is `0.4 - 2·1 = -1.6`
/// and the second is `0.441 - 1.905 = -1.464`. Both are negative, so both nodes move along `-x`
/// and the two displacements add.
fn two_nodes() -> Solve<'static> {
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
        pair: vec![0, 0, 1],
        coarse: 2,
    };
    let (x, _) = multilevel::prolongate(&[10.0, 20.0], &[0.0, 0.0], &level, 3, 7);
    // `P` gives `[10, 10, 20]`. Then, on the path `0-1-2`, node 1 takes the mean of its two
    // neighbours: `0.5·10 + 0.25·(10 + 20) = 12.5`, and node 2 then sees the *moved* node 1:
    // `0.5·20 + 0.5·12.5 = 16.25`.
    assert!(
        (x[1] - 12.5).abs() < 1e-3,
        "node 1 at {}, want 12.5: interpolate_coord did not run",
        x[1]
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

/// The 10x10 lattice's edges, in row-major order.
fn grid_edges(side: u32) -> Vec<(u32, u32)> {
    let index = |r: u32, c: u32| r * side + c;
    let mut out = Vec::new();
    for r in 0..side {
        for c in 0..side {
            if c + 1 < side {
                out.push((index(r, c), index(r, c + 1)));
            }
            if r + 1 < side {
                out.push((index(r, c), index(r + 1, c)));
            }
        }
    }
    out
}

/// No two points within `1e-6` of the drawing's own span, and a shape ratio above 0.15.
fn assert_spread(points: &[(f32, f32)]) {
    let n = points.len() as f64;
    let (mut mx, mut my) = (0.0f64, 0.0f64);
    for (x, y) in points {
        mx += f64::from(*x);
        my += f64::from(*y);
    }
    (mx, my) = (mx / n, my / n);
    let (mut sxx, mut syy, mut sxy) = (0.0f64, 0.0f64, 0.0f64);
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for (x, y) in points {
        let (dx, dy) = (f64::from(*x) - mx, f64::from(*y) - my);
        (sxx, syy, sxy) = (sxx + dx * dx, syy + dy * dy, sxy + dx * dy);
        lo = lo.min(f64::from(*x)).min(f64::from(*y));
        hi = hi.max(f64::from(*x)).max(f64::from(*y));
    }
    let (sxx, syy, sxy) = (sxx / n, syy / n, sxy / n);
    let half = 0.5 * (sxx + syy);
    let disc = (0.5 * (sxx - syy) * (sxx - syy) + sxy * sxy).sqrt();
    let ratio = (half - 0.5 * disc) / (half + 0.5 * disc);
    assert!(
        ratio > 0.15,
        "shape ratio {ratio}: the drawing is a strip or a point"
    );
    let near = 1e-6 * (hi - lo);
    for i in 0..points.len() {
        for j in i + 1..points.len() {
            let (dx, dy) = (points[i].0 - points[j].0, points[i].1 - points[j].1);
            assert!(
                f64::from(dx.hypot(dy)) > near,
                "nodes {i} and {j} are {} apart, under {near}",
                dx.hypot(dy)
            );
        }
    }
}

/// `lesmis.json` as `(node count, edges)`. Its node ids are integers that are also their own
/// positions, which is what `conformance/fixtures/named.rs:28-46` checks, so an edge's ends
/// are used as dense indices unchanged.
fn lesmis() -> (u32, Vec<(u32, u32)>) {
    let root = parse(LESMIS).expect("lesmis json");
    let Value::Object(fields) = &root else {
        panic!("lesmis is an object");
    };
    let listed = fields
        .iter()
        .find(|(key, _)| key == "nodes")
        .map(|(_, value)| match value {
            Value::Array(items) => items.len(),
            other => panic!("lesmis nodes: {other:?}"),
        })
        .expect("lesmis has nodes");
    let edges = fields
        .iter()
        .find(|(key, _)| key == "edges")
        .map(|(_, value)| match value {
            Value::Array(items) => items
                .iter()
                .map(|item| match item {
                    Value::Array(pair) => {
                        let mut ends = pair.iter().map(|v| match v {
                            Value::Number(text) => text.parse::<u32>().expect("node id"),
                            other => panic!("edge end: {other:?}"),
                        });
                        (ends.next().expect("from"), ends.next().expect("to"))
                    }
                    other => panic!("edge: {other:?}"),
                })
                .collect(),
            other => panic!("lesmis edges: {other:?}"),
        })
        .expect("lesmis has edges");
    (listed as u32, edges)
}