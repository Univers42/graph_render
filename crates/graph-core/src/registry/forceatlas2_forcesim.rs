//! Ledger metadata for `layout.forceatlas2.forcesim`, kept apart from `registry.rs` for
//! the house line cap, exactly as `forceatlas2_bh.rs` is.

use super::Metadata;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// Node count past which `layout.forceatlas2.forcesim` stops being usable.
///
/// **Time-bound and extrapolated, not measured at the ceiling.** The direct all-pairs
/// repulsion is `n^2` work per iteration and the layout runs 50 of them by default, so the
/// total is `50 n^2`. `layout.forceatlas2` is the same `O(n^2 * max_iter)` shape and is
/// pinned at 14 000 (`force.rs:31-36`); this port runs half the iterations at roughly four
/// times the cost per pair — the `f64` coefficient block and a `f64` dot per source, where
/// the networkx port builds one `f64` matrix per iteration — and 2 000 also keeps the
/// transient coefficient block (`CHUNK * n * 8` bytes, `pair_force.rs`) under 33 MB.
///
/// Ponytail (scale_ceiling): interpolated from the shape and the sibling's measurement,
/// **not measured at 2 000 nodes**; past it nothing refuses and the time keeps growing as
/// `n^2`. The escape hatch is `layout.forceatlas2.barnes_hut`, and for graphs over 400
/// nodes the reference itself stops using this path (`DIRECT_MAX`,
/// `simulation.py:14`) — see the `Ponytail (repulsion mode)` note in
/// `layout/force/forcesim.rs`.
pub const FA2_FORCESIM_CEILING: u64 = 2_000;

pub(super) const FA2_FORCESIM: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "SciGraphs' own ForceSim (model='FA2', repulsion_mode direct) as \
     _forceatlas2_forcesim runs it — SciGraphs/core/scigraphs_core/mesh/layouts/forceatlas.py:92-147 \
     into simulation.py:230-1094 — compared byte for byte over the scigraphs-conformance \
     fixture set by the FORCEATLAS2 row (scripts/scigraphs-conformance.sh, 24 fixtures, 1020 \
     coordinates). This is a different layout from layout.forceatlas2, which is held to \
     networkx 3.6 forceatlas2_layout by oracle-fa2 and stays that way; the two differ in the \
     generator (PCG64 against Mulberry32), the state width (f32 against f64), the gravity \
     constant (x0.1), the move cap (k = scale / cbrt(n)), dim (3 against 2) and the final \
     rescale (forceatlas.py:47-56), so neither can stand in for the other",
    complexity: "O(n^2) per iteration (one f64 coefficient block per chunk of 2048 targets, \
     one f64 dot per source) x iterations=50; attraction is O(m), gravity and the swing rule \
     O(n). Positions are f32 throughout the simulation and f64 only inside the repulsion \
     coefficient and the move cap",
    scale_ceiling: FA2_FORCESIM_CEILING,
    degradation: "past the ceiling there is no refusal: the layout keeps returning finite \
     geometry, in time growing as 50 n^2, so the caller must apply its own timeout. A \
     non-finite position (D9) refuses with StageError::NonFinite rather than reaching the \
     snapshot",
    ponytail: "Ponytail (repulsion mode): the reference switches on repulsion_mode at \
     DIRECT_MAX = 400 nodes (simulation.py:14,1003-1012) — to a quadtree, or failing that to \
     a KD-tree near field plus a coarse monopole grid the reference's own docstring measures \
     at 5-13% median force error (:623-625). This layout takes the exact all-pairs path at \
     every n, so above 400 nodes it is a different picture from the reference's, not a \
     perturbed one. Direction: exact repulsion, so the layout is more spread, never less. \
     Escape hatch: layout.forceatlas2.barnes_hut. Ponytail (scale_ceiling): \
     FA2_FORCESIM_CEILING is extrapolated from layout.forceatlas2's measured ceiling and this \
     port's cost per pair, not measured at 2 000 nodes. Ponytail (params): \
     strong_gravity, lin_log_mode, barnes_hut_optimize, barnes_hut_theta and \
     edge_weight_influence are fixed at their forceatlas.py defaults rather than exposed — \
     the last is inert without edge weights, and the others select force laws this port does \
     not implement (layout/force/forcesim.rs says which)",
};
