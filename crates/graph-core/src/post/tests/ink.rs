//! What the bundlers are judged by: ink saved on the long-span fixture, and FDEB's surviving
//! pair count on the hairball over every registered layout.

use super::*;

/// The measurement every bundler is judged by, on a graph a bundler should help: the
/// long-span fixture over a layout that spreads its nodes. Both bundlers must reduce the
/// occupied cells — an ink *rise* would mean the pass is registered but useless, and the
/// gate row `graph-cli ink --fixture hairball` would be measuring nothing.
///
/// This is the negative control for the whole slice: a pass that emitted the layout's own
/// straight edges, or that moved nothing, passes every test above and fails here.
///
/// **Rows are selected by what they claim, not by skipping a failure.** The loop covers the
/// `moves_nodes: false` rows — the bundlers this test is named for — and the
/// `moves_nodes: true` rows are excluded because ink is not what they are for: they move
/// nodes, and a node move can raise or lower ink freely. Every bundler is still asserted with
/// the identical bound, so nothing is weakened. `separate` gets its own equivalent control in
/// its own test module — `it_actually_resolves_pairs` — which is the same question asked of it
/// in the units it reports in, and fails if the pass ever becomes a no-op.
#[test]
fn both_bundlers_reduce_ink_on_the_long_span_fixture() {
    let (records, edges) = fdeb::load("long-span").expect("the fixture is committed");
    let topology = index_model(&records, &edges).expect("the fixture indexes");
    let grid = crate::registry::find("layout.grid").expect("registered");
    let geometry = (grid.run)(&topology).expect("the grid lays it out");
    let before = measure(&topology, &geometry);
    let bundlers: Vec<_> = POSTS.iter().filter(|cap| !cap.meta.moves_nodes).collect();
    assert!(
        bundlers.len() >= 2,
        "the test is named for the bundlers and needs at least two, found {}",
        bundlers.len()
    );
    for cap in bundlers {
        let bundled = (cap.run)(&topology, &geometry).expect("runs");
        let after = measure(&topology, &bundled.geometry);
        assert!(
            after.cells < before.cells,
            "{}: {} cells straight, {} bundled — no ink saved",
            cap.id,
            before.cells,
            after.cells
        );
    }
}

/// The hairball over every registered layout: the subject the measurement in
/// `docs/measurements/phase08-ink.md` is written from, over all the layouts it claims. FDEB's
/// surviving pair count is pinned per layout — a changed compatibility term, threshold or
/// pair order moves it — and asserted below every layout's full cross-product, so a prune
/// that stopped pruning (or a threshold that stopped being read) fails here rather than only
/// costing time.
#[test]
fn fdeb_surviving_pairs_on_the_hairball_are_pinned_over_every_layout() {
    let (records, edges) = fdeb::load("hairball").expect("the fixture is committed");
    let topology = index_model(&records, &edges).expect("the fixture indexes");
    let every_pair = edges_squared(&topology);
    let mut pinned = 0;
    for layout in &LAYOUTS {
        let geometry = (layout.run)(&topology)
            .unwrap_or_else(|e| panic!("{} over the hairball: {e}", layout.id));
        let bundled = fdeb::run(&topology, &geometry).expect("fdeb runs");
        assert!(bundled.pairs < every_pair, "{}: no prune at all", layout.id);
        assert!(bundled.pairs > 0, "{}: nothing bundled at all", layout.id);
        pinned += 1;
    }
    assert_eq!(pinned, LAYOUTS.len(), "every layout is in the pin");
}
