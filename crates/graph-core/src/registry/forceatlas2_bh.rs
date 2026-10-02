//! Ledger metadata for `layout.forceatlas2.barnes_hut`, kept apart from `registry.rs` for
//! the house line cap.

use super::Metadata;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// Node count past which `layout.forceatlas2.barnes_hut` stops being usable: eighteen times
/// [`super::force::FA2_CEILING`], under the same 30-second budget.
///
/// **Time-bound, measured** (`docs/measurements/perf-fa2bh.md`; `graph-cli bench --layout
/// layout.forceatlas2.barnes_hut`, native release, one thread): 10 000 nodes 583.83 ms,
/// 100 000 8 310.29 ms, 1 000 000 123 031.69 ms. Each tenfold step costs 14.2x and 14.8x, an
/// exponent of 1.17, so 30 s falls at `100 000 * (30 / 8.31)^(1 / 1.17) = 300 000`, rounded
/// down to 250 000 for the one-sample timings on a loaded host.
///
/// Ponytail (scale_ceiling): interpolated between two measured points, not measured at
/// 250 000; past it nothing refuses, the time keeps growing as n log n.
pub const FA2_BH_CEILING: u64 = 250_000;

pub(super) const FA2_BH: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "layout.forceatlas2, the exact dense sum this layout approximates: every other force \
    (attraction, gravity, swing, traction, speed) is the same code, and the tree repulsion is \
    differentially compared against the dense repulsion on the same state (rms error per node \
    < 2%, on the whole field < 5%, at 1500 and 5000 nodes after 0-100 iterations; \
    graph-core forceatlas2/state/barnes_hut/tests.rs). The finished layout is checked as \
    different but not worse than the exact one by graph-cli's stress correlation over 32 seeds \
    (stress/fa2.rs)",
    complexity: "O(n log n) per iteration (one quadtree build, one bottom-up mass pass, one \
    stackless preorder walk per node) x max_iter=100; attraction is O(m), gravity O(n)",
    scale_ceiling: FA2_BH_CEILING,
    degradation: "past the ceiling there is no refusal: the layout keeps returning finite \
    geometry, in time growing as n log n, so the caller must apply its own timeout. A \
    non-finite position (D9) refuses with StageError::NonFinite rather than reaching the \
    snapshot",
    ponytail: "Ponytail (theta): a far cell is a point mass at its centre, so a node's \
    repulsion is off by the cell's spread, worst on a settled layout where the repulsion \
    nearly cancels: at theta 0.8, up to 1.1% of a node's force and 4.0% of the whole field \
    (docs/measurements/perf-fa2bh.md). The layout is therefore a different picture from \
    layout.forceatlas2's, not a perturbed one; the escape hatch is layout.forceatlas2 itself. \
    Ponytail (early exit): as layout.forceatlas2, run() stops once a tick's total movement \
    falls under 1e-10",
};
