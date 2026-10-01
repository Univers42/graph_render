//! What is pinned against Graphviz 16.1.0's own `fdp -Tplain -Gstart=1`, and what cannot be.
//!
//! Three things are checked here, and the split between them is the finding of this job
//! rather than a matter of taste:
//!
//! 1. **The seeded placement, exactly.** `initPositions` is not iterative: two `drand48`
//!    draws per node into a box of half-extent `1.2 * K * (sqrt(n) + 1) / 2`. Running the
//!    oracle with `-Gmaxiter=1 -Goverlap=true` suppresses both the expansion ticks and the
//!    overlap removal, so what `-Tplain` prints *is* that placement, and the port
//!    reproduces it to the oracle's printed quantum. This is the strongest check available
//!    and it is a real one: it fixes `K`, the box formula, the seed, and the whole `drand48`
//!    sequence at once.
//! 2. **The reference's own closed arithmetic, exactly.** The one-node case never moves, and
//!    `compute_bb`'s box corner puts it at `(27, 18)` points to the last bit.
//! 3. **The force phases, against a measured number rather than a closed answer.** 300
//!    cooled ticks of a Fruchterman-Reingold model are chaotic, and the oracle is *not even
//!    self-reproducible* at that size — running it twice over the same graph gives
//!    byte-different output in the fifth significant digit (`docs/measurements/p13-gv2-fdp.md`).
//!    So there is no oracle answer to pin here. What is pinned is the two-node gap, which
//!    is the tightest the chaos allows and is a real regression bound, plus the invariants
//!    that hold whatever the forces do.

use crate::layout::coords::probe::{assert_close, graph, points};

use super::model::Model;
use super::run;

/// The oracle's own quantum: `-Tplain` prints five significant digits, so a coordinate it
/// reports is only known to about one part in `10^5` of itself.
const ORACLE_QUANTUM: f32 = 1e-3;

/// One closed shape: its name, its node count and its edges.
type Shape = (&'static str, u32, &'static [(u32, u32)]);

/// The measured two-node disagreement between this port and the oracle, in points, at
/// `-Gstart=1`: `x` off by 5.2e-3 and `y` by 7.1e-4 (`docs/measurements/p13-gv2-fdp.md`).
/// Rounded up to 1e-2 so the bound is a bound and not a knife edge. The figure moved once,
/// from 2.5e-3 to 5.2e-3, when `hypot` was replaced by `sqrt` in the repulsion kernel for
/// native/wasm32 bit-identity — which is the honest direction: the substitution costs an ulp
/// per force and the chaos turns it into half a milli-point after 300 ticks.
const TWO_NODE_GAP: f32 = 1e-2;

/// `fdp -Tplain -Gstart=1 -Gmaxiter=1 -Goverlap=true` over `n0 -- n1`, which is the seeded
/// placement with nothing else run: the oracle prints `n0 0.375 0.35283` and
/// `n1 1.06437 0.25`, and the port reproduces the same two offsets from the origin.
///
/// The comparison is on the *differences*, because the drawing's absolute origin is the box
/// corner and carries no information about the placement.
#[test]
fn the_seeded_placement_matches_the_oracle_exactly() {
    let model = Model::new(&graph(2, &[(0, 1)]));
    let dx = (model.x[1] - model.x[0]) * 72.0;
    let dy = (model.y[0] - model.y[1]) * 72.0;
    assert_close(
        &[(dx as f32, dy as f32)],
        &[(49.635, 7.416)],
        ORACLE_QUANTUM,
    );
}

/// The same check on a three-node path, where the box is a different size because it scales
/// with `sqrt(n)`. Oracle: `n0 0.375 0.69528`, `n1 1.154 0.57872`, `n2 0.89024 0.25`.
#[test]
fn the_seeded_placement_scales_with_the_node_count() {
    let model = Model::new(&graph(3, &[(0, 1), (1, 2)]));
    let at = |node: usize| {
        (
            (model.x[node] - model.x[0]) * 72.0,
            (model.y[node] - model.y[0]) * 72.0,
        )
    };
    let want = [(0.0, 0.0), (56.169, -8.392), (37.097, -32.06)];
    let got = [at(0), at(1), at(2)].map(|(x, y)| (x as f32, y as f32));
    assert_close(&got, &want, ORACLE_QUANTUM);
}

/// `graph g { n0; }` — nothing moves, and `compute_bb`'s box corner puts `n0` at half its
/// default `0.75 x 0.5` inch node, `(27, 18)` points.
#[test]
fn a_single_node_sits_at_half_its_default_box() {
    assert_close(
        &points(&run(&graph(1, &[])).unwrap()),
        &[(27.0, 18.0)],
        ORACLE_QUANTUM,
    );
}

/// The two-node force result, against the oracle with the measured disagreement as the
/// bound. See [`TWO_NODE_GAP`].
#[test]
fn two_nodes_land_on_the_oracle_within_the_measured_gap() {
    let want = [(27.0, 27.7762), (92.4336, 18.0)];
    assert_close(
        &points(&run(&graph(2, &[(0, 1)])).unwrap()),
        &want,
        TWO_NODE_GAP,
    );
}

/// The drawing's lower-left **box** corner is the origin, so the lowest coordinate on each
/// axis is half a node box and not zero — the same fact the one-node case pins.
#[test]
fn the_drawing_is_translated_onto_the_origin() {
    let placed = points(&run(&graph(3, &[(0, 1), (1, 2)])).unwrap());
    let low_x = placed.iter().map(|&(x, _)| x).fold(f32::INFINITY, f32::min);
    let low_y = placed.iter().map(|&(_, y)| y).fold(f32::INFINITY, f32::min);
    assert!((low_x - 27.0).abs() < ORACLE_QUANTUM, "lowest x {low_x}");
    assert!((low_y - 18.0).abs() < ORACLE_QUANTUM, "lowest y {low_y}");
}

/// The hash gate's own determinism requirement, and the one the oracle cannot meet at this
/// size: the same topology run twice settles on the same geometry.
#[test]
fn the_same_topology_runs_to_the_same_geometry_twice() {
    let edges = [(0, 1), (1, 2), (2, 3), (3, 0), (0, 2)];
    let first = points(&run(&graph(4, &edges)).unwrap());
    let second = points(&run(&graph(4, &edges)).unwrap());
    assert_eq!(first, second, "the layout is not deterministic");
}

/// The forces cannot put two nodes on top of each other, and cannot produce a non-finite
/// coordinate, on any of the six closed shapes. This is the invariant that survives the
/// chaos, and it is the one that would catch a broken force or a broken cooling schedule.
#[test]
fn every_closed_shape_lays_out_finitely_and_without_coincident_nodes() {
    let shapes: [Shape; 6] = [
        ("one-node", 1, &[]),
        ("two-nodes", 2, &[(0, 1)]),
        ("three-path", 3, &[(0, 1), (1, 2)]),
        ("four-cycle", 4, &[(0, 1), (1, 2), (2, 3), (3, 0)]),
        ("five-star", 5, &[(0, 1), (0, 2), (0, 3), (0, 4)]),
        ("six-branch", 6, &[(0, 1), (0, 2), (0, 3), (2, 4), (4, 5)]),
    ];
    for (name, count, edges) in shapes {
        let placed = points(&run(&graph(count, edges)).unwrap());
        assert_eq!(placed.len(), count as usize, "{name}: node count");
        for &(x, y) in &placed {
            assert!(x.is_finite() && y.is_finite(), "{name}: {x},{y}");
            assert!(x >= 0.0 && y >= 0.0, "{name}: {x},{y} is off the origin");
        }
        for a in 0..placed.len() {
            for b in (a + 1)..placed.len() {
                let (dx, dy) = (placed[a].0 - placed[b].0, placed[a].1 - placed[b].1);
                assert!(dx != 0.0 || dy != 0.0, "{name}: nodes {a} and {b} coincide");
            }
        }
    }
}

/// An empty graph has nothing to place, and must not reach `sqrt(0)` in the initial
/// placement.
#[test]
fn an_empty_graph_has_no_points() {
    assert_eq!(points(&run(&graph(0, &[])).unwrap()), vec![]);
}
