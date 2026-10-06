//! The lanes layout's own tests: the hand-built shapes, over [`history`] the seeded
//! shapes the gate's own model cannot produce.
//!
//! The correctness claim — no vertex sits on an edge that runs past it — is checked in
//! [`history`] from every side (a seeded DAG, parallel edges, a wide fan-in from sources at
//! different rows, and a reversed head-to-tail edge spanning several rows). Conditions 2
//! and 7 of `docs/decisions/dag-lanes.md`.

mod history;
mod horizontal;

use super::*;
use crate::index::index_model;
use crate::records::build::{edge, node};
use crate::records::{EdgeRecord, NodeRecord};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry, Paths};
use graph_contract::notes::NoteCode;
use history::history;

fn vertex(id: &str, version: f64) -> NodeRecord {
    NodeRecord {
        version,
        ..node(id, "")
    }
}

fn arc(id: &str, source: &str, target: &str) -> EdgeRecord {
    EdgeRecord {
        directed: true,
        ..edge(id, source, target)
    }
}

/// `(x, y, paths, note edge indices)` of one run at unit spacing.
fn drawn(nodes: &[NodeRecord], edges: &[EdgeRecord]) -> (Vec<f32>, Vec<f32>, Paths, Vec<u32>) {
    let topology = index_model(nodes, edges).expect("fits");
    let geometry = run(&topology, &LanesParams::default()).expect("unit spacing is legal");
    let NodeGeometry::Point { x, y } = geometry.nodes else {
        panic!("not Point nodes")
    };
    let EdgeGeometry::Polyline(paths) = geometry.edges else {
        panic!("not Polyline edges")
    };
    assert!(
        geometry
            .notes
            .iter()
            .all(|n| n.code == NoteCode::EdgeReversed)
    );
    let notes = geometry.notes.iter().map(|n| n.index).collect();
    (x, y, paths, notes)
}

#[test]
fn a_chain_is_one_lane_and_one_row_per_vertex() {
    let n = [
        vertex("d", 4.0),
        vertex("c", 3.0),
        vertex("b", 2.0),
        vertex("a", 1.0),
    ];
    let e = [
        arc("dc", "d", "c"),
        arc("cb", "c", "b"),
        arc("ba", "b", "a"),
    ];
    let (x, y, paths, notes) = drawn(&n, &e);
    assert_eq!(x, [0.0, 0.0, 0.0, 0.0]);
    assert_eq!(y, [0.0, 1.0, 2.0, 3.0]);
    assert_eq!(paths.offsets, [0, 0, 0, 0]);
    assert!(notes.is_empty());
}

#[test]
fn a_branch_and_its_merge_take_two_lanes() {
    // m merges b (first) and c; b and c both descend from a. Rows: m, c (newer), b, a.
    let n = [
        vertex("a", 1.0),
        vertex("b", 2.0),
        vertex("c", 3.0),
        vertex("m", 4.0),
    ];
    let e = [
        arc("mb", "m", "b"),
        arc("mc", "m", "c"),
        arc("ba", "b", "a"),
        arc("ca", "c", "a"),
    ];
    let (x, y, paths, notes) = drawn(&n, &e);
    assert_eq!(x, [0.0, 0.0, 1.0, 0.0], "a, b, c, m");
    assert_eq!(y, [3.0, 2.0, 1.0, 0.0], "a, b, c, m");
    assert_eq!(paths.offsets, [0, 0, 1, 1, 2]);
    assert_eq!(paths.pts, [1.0, 0.5, 1.0, 2.5]);
    assert!(notes.is_empty());
}

#[test]
fn a_directed_cycle_is_broken_at_the_lowest_index_and_noted() {
    let n = [vertex("a", 0.0), vertex("b", 0.0), vertex("c", 0.0)];
    let e = [
        arc("ab", "a", "b"),
        arc("bc", "b", "c"),
        arc("ca", "c", "a"),
    ];
    let (x, y, paths, notes) = drawn(&n, &e);
    assert_eq!(y, [0.0, 1.0, 2.0]);
    assert_eq!(x, [0.0, 0.0, 0.0]);
    assert_eq!(notes, [2], "c -> a runs against the rows");
    assert_eq!(paths.offsets, [0, 0, 0, 2]);
    assert_eq!(paths.pts, [1.0, 1.5, 1.0, 0.5], "source c to target a");
}

/// The seed of `docs/decisions/dag-lanes.md` condition 7(c): the seeded gate model holds
/// every `version` at 0.0 and never breaks a cycle, so the `Ready` heap's
/// `f64::total_cmp` comparison is never executed by any gate. Here `version` rises with the
/// dense index, so the two orders disagree and the version must win.
///
/// `a` and `b` start; `b` has no predecessor, then `a` is placed as the cycle break, which
/// makes `c` and `d` ready together. `c` has the lower index and `d` the higher `version`,
/// so `d` takes row 2 and `c` row 3 — the reverse of what index order would give. Then `c`
/// releases `e`, and `e -> a` is the one reversed edge.
#[test]
fn distinct_versions_break_a_cycle_and_the_heap_orders_by_version() {
    let n = [
        vertex("a", 1.0),
        vertex("b", 2.0),
        vertex("c", 3.0),
        vertex("d", 4.0),
        vertex("e", 5.0),
    ];
    let e = [
        arc("ac", "a", "c"),
        arc("ad", "a", "d"),
        arc("bd", "b", "d"),
        arc("be", "b", "e"),
        arc("ce", "c", "e"),
        arc("ea", "e", "a"),
    ];
    let (_, y, _, notes) = drawn(&n, &e);
    assert_eq!(y, [1.0, 0.0, 3.0, 2.0, 4.0], "b, a, d, c, e");
    assert_ne!(y, [0.0, 1.0, 2.0, 3.0, 4.0], "not dense index order");
    assert_eq!(notes, [5], "only e -> a runs against the rows");
}

#[test]
fn equal_versions_fall_back_to_index_order() {
    let n = [vertex("p", 0.0), vertex("q", 0.0), vertex("r", 0.0)];
    let (_, y, _, _) = drawn(&n, &[]);
    assert_eq!(y, [0.0, 1.0, 2.0]);
}

#[test]
fn parallel_and_undirected_edges_are_routed_without_notes() {
    let n = [vertex("a", 1.0), vertex("b", 2.0)];
    let e = [
        edge("u", "a", "b"),
        arc("d1", "b", "a"),
        arc("d2", "b", "a"),
    ];
    let (_, y, paths, notes) = drawn(&n, &e);
    assert_eq!(y, [1.0, 0.0], "b is newer");
    assert_eq!(paths.offsets.len(), 4);
    assert!(
        notes.is_empty(),
        "an undirected edge is never reversed: {notes:?}"
    );
}

#[test]
fn degenerate_graphs_draw() {
    let (x, _, paths, _) = drawn(&[], &[]);
    assert!(x.is_empty() && paths.offsets == [0]);
    let (x, _, paths, notes) = drawn(&[vertex("a", 0.0)], &[arc("aa", "a", "a")]);
    assert_eq!(x, [0.0]);
    assert_eq!(paths.offsets, [0, 0]);
    assert!(notes.is_empty());
}

#[test]
fn spacing_scales_both_axes_and_a_bad_one_is_refused() {
    let n = [vertex("m", 2.0), vertex("a", 1.0), vertex("b", 1.0)];
    let e = [arc("ma", "m", "a"), arc("mb", "m", "b")];
    let t = index_model(&n, &e).expect("fits");
    let wide = LanesParams {
        lane_spacing: 2.0,
        row_spacing: 3.0,
        horizontal: false,
    };
    let NodeGeometry::Point { x, y } = run(&t, &wide).expect("legal").nodes else {
        panic!()
    };
    assert_eq!((x[2], y[2]), (2.0, 6.0), "b: lane 1, row 2");
    for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(
            run(
                &t,
                &LanesParams {
                    lane_spacing: bad,
                    row_spacing: 1.0,
                    horizontal: false
                }
            )
            .is_err()
        );
        assert!(
            run(
                &t,
                &LanesParams {
                    lane_spacing: 1.0,
                    row_spacing: bad,
                    horizontal: false
                }
            )
            .is_err()
        );
    }
}

/// The half-row bend is `(row as f32) + 0.5`, exact in `f32` only while `row < 2^23`; from
/// there the bend lands on an adjacent vertex row and the layout's own promise is quietly
/// voided. `docs/decisions/dag-lanes.md` condition 3, refusal form (a).
#[test]
fn a_graph_at_the_half_row_limit_is_refused_and_one_row_below_it_is_not() {
    assert!(
        rows_fit(MAX_ROWS - 1).is_ok(),
        "the last row whose + 0.5 is exact"
    );
    assert!(
        rows_fit(MAX_ROWS).is_err(),
        "the first row whose + 0.5 is not"
    );
    let (n, e) = history(4, 1);
    let t = index_model(&n, &e).expect("fits");
    assert!(
        run(&t, &LanesParams::default()).is_ok(),
        "a small graph draws"
    );
}
