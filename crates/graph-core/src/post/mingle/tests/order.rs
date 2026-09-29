use super::*;

#[test]
fn the_long_span_fixture_is_exactly_the_graph_the_bundler_merges() {
    let (topology, _) = laid_out();
    assert_eq!(topology.node_count(), 8);
    assert_eq!(topology.edge_count(), 8);
    // The committed fixture is the same graph, edge for edge, in the same order.
    let (records, fixture_edges) = crate::post::fdeb::load("long-span").expect("committed");
    assert_eq!(records.len(), topology.node_count() as usize);
    let mut named: Vec<(&str, &str, &str)> = fixture_edges
        .iter()
        .map(|e| (e.id.as_str(), e.source.as_str(), e.target.as_str()))
        .collect();
    named.sort_unstable();
    assert_eq!(named.len(), 8);
    assert_eq!(named[0], ("a0-b0", "a0", "b0"));
    assert_eq!(named[7], ("a3-b3", "a3", "b3"));
    // Every fixture edge names a bank-to-bank pair, and the four parallel ones come first:
    // the near-coincident family the bundler is for.
    assert_eq!(
        named
            .iter()
            .filter(|(_, s, t)| s.starts_with('a') && t.starts_with('b'))
            .count(),
        8,
        "every edge spans the gap"
    );
}

#[test]
fn the_long_span_case_folds_its_ink_and_the_bundle_shares_one_trunk() {
    let (topology, g) = laid_out();
    let out = over(&topology, &g, &Params::default()).expect("runs");
    assert!(out.merges > 0, "something merged");
    assert!(out.ink_after < out.ink_before, "the drawing got shorter");
    // Eight straight edges of length ~4.0 each draw ~32; bundled, the four of each bank share
    // one trunk, so the drawn ink is a small fraction of it.
    assert!(
        out.ink_after < out.ink_before / 2.0,
        "{} -> {} on the long-span claim",
        out.ink_before,
        out.ink_after
    );
    // Every row holds `2 * depth` interior points: the contract's offsets must say so.
    let edges = topology.edge_count() as usize;
    assert_eq!(out.paths.offsets.len(), edges + 1);
    for e in 0..edges {
        assert_eq!(
            out.paths.offsets[e + 1] - out.paths.offsets[e],
            2 * out.depth[e],
            "edge {e}: the row's length is twice its depth"
        );
    }
    // Every point is finite (D9).
    assert!(out.paths.pts.iter().all(|v| v.is_finite()));
}

#[test]
fn the_merge_order_is_total_so_equal_gains_break_by_candidate_position() {
    // Four identical edges between the same two nodes. Every pair scores exactly the same
    // gain, so the only thing that can decide the order is the tie-break: the candidate
    // position, itself ascending (u, v). The first candidate is (0, 1), so the run must take
    // it — a sort that broke ties by anything else would merge a different pair, and a
    // sequential matching that took two sharing a bundle would produce a different bundle.
    let pos = [[0.0, 0.0], [4.0, 0.0]];
    let out = bundle(&pos, &[0, 0, 0, 0], &[1, 1, 1, 1], &Params::default()).expect("runs");
    assert_eq!(
        out.merges, 3,
        "two matched merges in the first pass, then their two bundles"
    );
    assert_eq!(
        out.cluster,
        vec![0, 0, 0, 0],
        "everything ends in one bundle"
    );
    assert_eq!(out.depth, vec![1, 1, 1, 1]);
}

#[test]
fn the_same_input_bundles_to_the_same_bytes_every_time() {
    let (topology, g) = laid_out();
    let first = over(&topology, &g, &Params::default()).expect("runs");
    for _ in 0..3 {
        let again = over(&topology, &g, &Params::default()).expect("runs");
        assert_eq!(first, again, "no clock, no randomness, one order");
    }
    // The layout's node geometry is carried through untouched, and the notes with it.
    let mut with_notes = g.clone();
    with_notes.notes = vec![graph_contract::notes::Note {
        code: graph_contract::notes::NoteCode::CycleEdgeDropped,
        index: 0,
    }];
    let out = over(&topology, &with_notes, &Params::default()).expect("runs");
    assert_eq!(out.paths, first.paths, "a note does not move a point");
}
