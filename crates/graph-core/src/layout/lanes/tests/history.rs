//! The seeded shapes the gate's own model cannot produce, and the one claim they all check:
//! no vertex sits on an edge that runs past it.
//!
//! The seeded gate model holds every `version` at 0.0 and points its arcs from a higher
//! index to a lower one, which is already a topological order — so no gate runs the
//! cycle-breaking branch, the `EdgeReversed` note path, or the ready heap's `version`
//! comparison. These four cases are that coverage. `docs/decisions/dag-lanes.md`
//! conditions 2 and 7.
//!
//! It also carries the two shapes that pin rule D (`docs/decisions/dag-lanes-merge.md`
//! conditions 3 and 4) in numbers: no structural check in the tree tells rule D from rule C,
//! so a hand-written lane assignment is the only evidence that the `S < lane(v)` guard is
//! there.

use super::*;
use crate::index::index_model;
use crate::records::build::edge;
use crate::records::{EdgeRecord, NodeRecord};
use std::collections::BTreeMap;

/// A seeded history-shaped DAG: vertex `i` points at one or two later vertices, with a
/// fan-in up to 50 on every 97th vertex. Deterministic (an LCG, no clock, no `rand`).
pub(super) fn history(n: u32, seed: u64) -> (Vec<NodeRecord>, Vec<EdgeRecord>) {
    let mut state = seed;
    let mut next = |bound: u32| {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) % u64::from(bound.max(1))) as u32
    };
    let ids: Vec<String> = (0..n).map(|i| format!("v{i}")).collect();
    let nodes = ids
        .iter()
        .enumerate()
        .map(|(i, id)| vertex(id, f64::from(n - i as u32)))
        .collect();
    let mut edges = Vec::new();
    for i in 0..n.saturating_sub(1) {
        let parents = if i % 97 == 0 { 50 } else { 1 + next(2) };
        for k in 0..parents {
            let p = (i + 1 + next(40)).min(n - 1);
            edges.push(arc(
                &format!("e{i}_{k}"),
                &ids[i as usize],
                &ids[p as usize],
            ));
        }
    }
    (nodes, edges)
}

/// The claim, as one reusable check: `nodes`, `edges` and `label` in, a pass or the
/// assertion that names the offending vertex out. Every case below routes through it, so
/// none of them can quietly stop checking.
fn assert_nothing_sits_on_an_edge(nodes: &[NodeRecord], edges: &[EdgeRecord], label: &str) {
    let topology = index_model(nodes, edges).expect("fits");
    let (x, y, paths, _) = drawn(nodes, edges);
    let cols = topology.edges();
    let mut at: BTreeMap<(u32, u32), u32> = BTreeMap::new();
    for (v, (&px, &py)) in x.iter().zip(&y).enumerate() {
        at.insert((px as u32, py as u32), v as u32);
    }
    for (edge, (&s, &d)) in cols.source.iter().zip(&cols.target).enumerate() {
        let (s, d) = (s as usize, d as usize);
        let span = paths.offsets[edge] as usize..paths.offsets[edge + 1] as usize;
        let lane = span.clone().next().map_or(x[s], |p| paths.pts[2 * p]);
        for row in (y[s].min(y[d]) as u32 + 1)..(y[s].max(y[d]) as u32) {
            let sitting = at.get(&(lane as u32, row));
            assert!(
                sitting.is_none(),
                "{label} edge {edge}: vertex {sitting:?} on lane {lane} row {row}"
            );
        }
    }
}

#[test]
fn no_vertex_sits_on_an_edge_it_does_not_end() {
    for seed in 1..=8 {
        let (n, e) = history(2_000, seed);
        let (_, _, _, notes) = drawn(&n, &e);
        assert!(notes.is_empty(), "seed {seed}: a DAG reverses nothing");
        assert_nothing_sits_on_an_edge(&n, &e, &format!("seed {seed}"));
    }
}

/// Parallel edges between one pair, directed and undirected mixed: each carries the pair's
/// own lane, so the second must not push a vertex onto the first's run
/// (`docs/decisions/dag-lanes.md` condition 2).
#[test]
fn parallel_edges_between_one_pair_leave_nothing_sitting() {
    let n = [vertex("a", 1.0), vertex("b", 2.0), vertex("c", 3.0)];
    let e = [
        edge("u", "a", "b"),
        arc("d1", "b", "a"),
        arc("d2", "b", "a"),
        arc("bc", "b", "c"),
        arc("c1", "c", "a"),
        arc("c2", "c", "a"),
        arc("c3", "c", "a"),
    ];
    assert_nothing_sits_on_an_edge(&n, &e, "parallel");
}

/// A merge of fan-in 50 whose sources sit at 50 different rows, which is what makes the
/// convergence lanes real: under rule D every source but the first shares the column already
/// waiting for `sink`, so the fan-in costs the columns the sources occupy and not one column
/// per source, and the sink's own row must find none of them running past it.
#[test]
fn a_merge_with_a_fifty_way_fan_in_from_different_rows_leaves_nothing_sitting() {
    let mut n: Vec<NodeRecord> = (0..50)
        .map(|i| vertex(&format!("s{i}"), f64::from(50 - i)))
        .collect();
    n.push(vertex("sink", 100.0));
    let mut e: Vec<EdgeRecord> = (0..50)
        .map(|i| arc(&format!("e{i}"), &format!("s{i}"), "sink"))
        .collect();
    // A chain above the sources, so the 50 sources are not all at row 0 and the fan-in
    // arrives across fifty rows rather than one.
    n.push(vertex("head", 200.0));
    e.push(arc("head0", "head", "s0"));
    for i in 1..50 {
        e.push(arc(&format!("h{i}"), "head", &format!("s{i}")));
    }
    assert_nothing_sits_on_an_edge(&n, &e, "fan-in 50");
}

/// `(lane, carried, width)` straight from the assignment, for the tests that pin *which*
/// column an edge runs down rather than the polyline it drew. The width is the pool's: one
/// past the largest lane any vertex or edge used, self-loops (`NONE`) excluded.
fn assigned(nodes: &[NodeRecord], edges: &[EdgeRecord]) -> (Vec<u32>, Vec<u32>, u32) {
    let topology = index_model(nodes, edges).expect("fits");
    let rows = rows::Rows::of(&topology);
    let drawing = assign::Drawing::of(&topology, &rows);
    let used = drawing
        .lane
        .iter()
        .chain(drawing.carried.iter())
        .filter(|&&l| l != assign::NONE)
        .max()
        .copied()
        .unwrap_or(0);
    (drawing.lane, drawing.carried, used + 1)
}

/// Rule D, both arms in one vertex (`docs/decisions/dag-lanes-merge.md` condition 3). `d→b`
/// pushes lane 0, so `a` settles on lane 1 and its **first** edge `a→b` shares the column
/// already waiting for `b` (`S = 0 < 1`), bending left; `a→c` finds nothing waiting and
/// pushes `lane(a) = 1`; `a→e` then finds its own lane already carried and opens a fresh
/// lane 2. Dropping the `S < lane(v)` clause draws `c→e` into lane 2 too — rule C's picture,
/// width 4 — and that passes every structural check in the tree.
#[test]
fn a_line_bends_left_into_a_column_already_waiting_and_keeps_a_fresh_one_for_its_third_edge() {
    let n = [
        vertex("a", 1.0),
        vertex("b", 2.0),
        vertex("c", 3.0),
        vertex("d", 4.0),
        vertex("e", 5.0),
    ];
    let e = [
        arc("ab", "a", "b"),
        arc("ac", "a", "c"),
        arc("ae", "a", "e"),
        arc("ce", "c", "e"),
        arc("db", "d", "b"),
    ];
    let (x, y, paths, notes) = drawn(&n, &e);
    let (lane, carried, width) = assigned(&n, &e);
    assert_eq!(x, [1.0, 0.0, 1.0, 0.0, 1.0], "a, b, c, d, e");
    assert_eq!(y, [1.0, 4.0, 2.0, 0.0, 3.0], "a, b, c, d, e");
    assert_eq!(
        carried,
        [0, 1, 2, 1, 0],
        "ab shares 0, ae opens 2, ce joins 1"
    );
    assert_eq!(width, 3);
    assert_eq!(paths.offsets, [0, 1, 1, 3, 3, 3]);
    assert_eq!(paths.pts, [0.0, 1.5, 2.0, 1.5, 2.0, 2.5]);
    assert!(notes.is_empty());
    assert_eq!(lane[0], 1, "a holds the lane no edge took");
}

/// Rule D on the converging case the spec describes (`condition 4`): three lines forked
/// separately off one base, the second and third of which find a column already waiting for
/// it. Each shares it, so the base settles in one lane and the drawing is width 2.
#[test]
fn three_lines_forked_from_one_base_share_the_column_waiting_for_it() {
    let n = [
        vertex("a", 1.0),
        vertex("b", 2.0),
        vertex("c", 3.0),
        vertex("d", 4.0),
    ];
    let e = [
        arc("da", "d", "a"),
        arc("ca", "c", "a"),
        arc("ba", "b", "a"),
    ];
    let (x, y, paths, notes) = drawn(&n, &e);
    let (_, carried, width) = assigned(&n, &e);
    assert_eq!(x, [0.0, 1.0, 1.0, 0.0], "a, b, c, d");
    assert_eq!(y, [3.0, 2.0, 1.0, 0.0], "a, b, c, d");
    assert_eq!(
        carried,
        [0, 0, 0],
        "all three forks share the base's column"
    );
    assert_eq!(width, 2);
    assert_eq!(paths.offsets, [0, 0, 1, 2]);
    assert_eq!(paths.pts, [0.0, 1.5, 0.0, 2.5]);
    assert!(notes.is_empty());
}

/// A reversed (head-to-tail) edge spanning several rows: the shape the seeded gate model
/// never produces, because its arcs are already a topological order, and therefore the one
/// most likely to be wrong (`docs/decisions/dag-lanes.md` conditions 2 and 7).
#[test]
fn a_reversed_edge_spanning_several_rows_leaves_nothing_sitting() {
    let n: Vec<NodeRecord> = (0..8)
        .map(|i| vertex(&format!("v{i}"), f64::from(8 - i)))
        .collect();
    let mut e: Vec<EdgeRecord> = (0..7)
        .map(|i| arc(&format!("c{i}"), &format!("v{i}"), &format!("v{}", i + 1)))
        .collect();
    e.push(arc("back", "v7", "v0"));
    let (_, _, _, notes) = drawn(&n, &e);
    assert_eq!(notes, [7], "only the back edge runs against the rows");
    assert_nothing_sits_on_an_edge(&n, &e, "reversed");
}
