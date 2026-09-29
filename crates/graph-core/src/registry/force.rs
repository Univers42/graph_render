//! Ledger metadata for the Phase 6 force family (`layout.force.barnes_hut`,
//! `layout.forceatlas2`), kept apart from `registry.rs` for the house line cap.

use super::Metadata;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// Node count past which `layout.force.barnes_hut` stops being usable, and why it is
/// this one.
///
/// **Time-bound, measured, not memory-bound** (`docs/measurements/phase06-force.md`;
/// reproduce with `cargo run --release -p graph-cli -- bench --layout
/// layout.force.barnes_hut --n <n>`, which times the registered run only). Natively,
/// release, x86_64, inside the toolchain
/// image, at 220 nodes / 329 edges 17.15 ms, 10 000 / 15 474 edges 2 271.35 ms, and
/// 100 000 / 154 978 edges **46 781.60 ms**. Theta-approximated many-body is
/// `O(n log n)` per tick and there are a fixed 112 of them, so 100 000 sits inside a
/// 60-second budget and 200 000 — extrapolated, *not* run, because it is itself
/// impractically slow — lands near 110 s and past it. 100 000 is round and stated at
/// the size actually measured.
///
/// Ponytail (scale_ceiling): the extrapolation past the measured point is an `O(n log n)`
/// reading of two ratios (45x nodes -> 132x time; 10x nodes -> 20.6x time, the latter
/// above `n log n`'s own ~12x because quadtree depth and collide's fixed-radius
/// neighbour queries both grow with n), not a measurement at 200 000. Unlike the grid's
/// and the hierarchy layouts' ceilings, this one is *not* a memory wall and is *not*
/// projected onto wasm32's 4 GiB: a caller inside wasm32 gets the same time curve in a
/// slower machine, with no new refusal to detect it by. The `u32` index space binds
/// far later (2^32 nodes).
pub const FORCE_CEILING: u64 = 100_000;

/// Node count past which `layout.forceatlas2` stops being usable, and why it is this
/// one — two orders of magnitude below [`FORCE_CEILING`], and for the same structural
/// reason the two ceilings differ at all.
///
/// **Time-bound, measured** (`docs/measurements/phase06-force.md`, the same command with
/// `--layout layout.forceatlas2`): 220 / 329 edges 6.58 ms, 1 000 / 1 541 edges 131.53 ms,
/// 2 000 / 3 075 edges
/// 524.82 ms, 5 000 / 7 721 edges 3 301.05 ms, 10 000 / 15 474 edges **13 522.24 ms**.
/// The 5 000 -> 10 000 step is 4.09x for exactly 2x the nodes: this is a clean `O(n^2)`
/// (networkx 3.6's dense all-pairs repulsion, ported verbatim — there is no spatial
/// approximation in the algorithm being ported, unlike Barnes-Hut's theta-tree). Under a
/// 30-second budget, `10 000 * sqrt(30 / 13.522) = 14 880`, rounded down to two figures
/// and to a round 14 000. 14 000 is *not* run: 13.5 s is already the slowest thing this
/// branch measures, and the point is the shape, not the digit.
///
/// Ponytail (scale_ceiling): the projection past the last measured point assumes the
/// `O(n^2)` the two measured ratios support (3.97x and 4.09x for 2x nodes) holds at
/// larger n. It is the *algorithm's* cost, not this port's: `repulsion()` visits each
/// unordered pair once and never materialises an `n x n` matrix, so this is a time
/// ceiling and not a memory one. Past it nothing refuses; the layout keeps returning
/// finite geometry, just for tens of seconds and growing quadratically, so a caller must
/// apply its own timeout — the honest ceiling is a budget, and a budget needs a
/// timeout to be real.
pub const FA2_CEILING: u64 = 14_000;

pub(super) const BARNES_HUT: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "d3-force@3.0.0 src/{manyBody,link,center,collide}.js — a port of the frozen force \
    set (theta 0.9, charge -90, distanceMax 520, linkDistance 60, collideRadius 16, alphaDecay \
    0.06, velocityDecay 0.42) at TICKS=112, in Jacobi/gather form with a counter-based jiggle; \
    differentially compared by the d3-force arm of harness/stress-d3.mjs, driven by graph-cli \
    stress --oracle d3, over >=1000 seeds. \
    Identity is NOT claimed and cannot be: link and collide are Jacobi gathers where d3 scatters \
    in visit order, and a force simulation amplifies a 1-ULP difference into a different \
    picture, so the gate is the stress metric (Pearson hop/euclid correlation over 32 max-min \
    pivots, margin -0.05 vs d3) — 'different, but not worse', measured in \
    docs/measurements/phase06-stress.md",
    complexity: "O(n log n) per tick x TICKS=112, so O(112 n log n) overall; collide's \
    fixed-radius neighbour query and the quadtree's depth growth sit on top of the \
    many-body term's own n log n",
    scale_ceiling: FORCE_CEILING,
    degradation: "past the ceiling there is no refusal and no trap: the layout still returns \
    finite geometry, it just takes longer than a 60-second budget and keeps growing — the caller \
    must apply its own timeout, exactly as at the ceiling itself. A non-finite position (D9) \
    refuses with StageError::NonFinite rather than reaching the snapshot",
    ponytail: "force layouts are CHAOTIC: the same graph with one node added or removed is a \
    different picture, not a perturbed one, and there is no failing input narrower than 'any \
    topology change'. Direction: cosmetic-but-surprising, never silently wrong — this layout is \
    graded on the stress metric, not on visual stability with the input. Escape hatch: a fixed \
    seed, and this stage's own determinism (the same topology run twice settles to the same \
    geometry every time, 4-way hash equal). Ponytail (theta): the opening angle trades accuracy \
    for speed; failing input is two dense well-separated clusters whose combined bounding box is \
    small relative to a far node's distance from them, where the tree treats a whole cluster as \
    one point mass too eagerly; direction is OVER-CLUMPING (distant structure collapsing \
    together), and theta is a frozen engine constant here, not a per-call knob. \
    Ponytail (scale_ceiling): time-bound and measured at 100 000 only; 200 000 is an O(n log n) \
    extrapolation, not a run",
};

pub(super) const FA2: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "networkx 3.6 forceatlas2_layout (networkx/drawing/layout.py:1604-1875) — a port of \
    the whole function at its default configuration (linlog=False, distributed_action=False, \
    strong_gravity=False, adjust_sizes=False, dim=2, weight=None), with swing/traction carried \
    cumulatively across iterations as the reference itself does; differentially compared against \
    the real library in the ge-python-oracle image (pinned networkx 3.6, numpy, scipy 1.16.2) \
    over >=1000 seeds, in harness/oracle-fa2.py, gating at 2 iterations rather than \
    networkx's default 100: FA2 is chaotic enough that past 2 iterations networkx diverges \
    from a ONE-ULP perturbation of its own start by more than the port diverges from it, so \
    no coordinate tolerance is honest at 100 and the full-100-iteration comparison is \
    reported, not gated (docs/measurements/fa2-chaos.md). Two deviations from the reference, \
    both stated \
    rather than hidden: initial positions come from graph-core's own seeded Mulberry32 instead \
    of numpy's global RNG (D5 — there is no global RNG to reach for), and an exact coincidence \
    (d2 == 0) is nudged apart by the counter hash so no Infinity/NaN factor can reach a node \
    (D9), which the reference's dense form has no guard for",
    complexity: "O(n^2) per iteration x max_iter=100, so O(100 n^2) worst case; attraction is \
    O(m), gravity is O(n), and neither changes the shape",
    scale_ceiling: FA2_CEILING,
    degradation: "past the ceiling there is no refusal and no trap either: the dense all-pairs \
    repulsion still returns finite geometry, it just takes tens of seconds and keeps growing \
    quadratically (13.5 s measured at 10 000 nodes), so the caller must apply its own timeout. \
    A non-finite position (D9) refuses with StageError::NonFinite rather than reaching the \
    snapshot",
    ponytail: "force layouts are CHAOTIC, identically to Barnes-Hut: one added node is a \
    different picture, not a perturbed one. Direction: cosmetic-but-surprising, never silently \
    wrong; the escape hatch is the same — a fixed seed and the stage's own run-to-run \
    determinism. Ponytail (early exit): the iteration count is not fixed the way Barnes-Hut's \
    TICKS is: run() stops as soon as a tick's total movement falls under networkx's own 1e-10, \
    so a quiet graph finishes in fewer than 100 iterations and two graphs of the same size can \
    do different amounts of work. Ponytail (scale_ceiling): time-bound, and the projection past \
    10 000 assumes the O(n^2) the measured ratios support holds at larger n",
};

pub(super) const YIFAN_HU: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "none: NOT Graphviz sfdp and never compared against it. A multilevel scheme (greedy \
    matching coarsening, the Barnes-Hut force simulation of layout.force.barnes_hut at the \
    coarsest level and as the refinement solver at each finer one). Held only to unit tests \
    (coarsening maps, determinism, neighbours nearer than the average pair) and to the stress \
    metric's method; no differential has run",
    complexity: "O(n log n) per tick per level; levels shrink by about half, so the total is \
    O(n log n) x (112 + 48 x levels) ticks worst case",
    scale_ceiling: FORCE_CEILING,
    degradation: "past the ceiling there is no refusal: every level is a Barnes-Hut solve, so \
    time grows as O(n log n) and the caller must apply its own timeout; a non-finite position \
    refuses with StageError::NonFinite rather than reaching the snapshot",
    ponytail: "Ponytail: this is NOT Graphviz sfdp; it shares only the multilevel idea, so \
    coordinates and scale differ from sfdp's for the same graph. Coarsening is a greedy \
    index-order matching (not label-invariant) and refinement is a fixed 48 ticks from alpha \
    0.3, so a folded coarse layout can survive into the result. Force layouts are chaotic: \
    one added node is a different picture. Ponytail (scale_ceiling): measured, single run, 60.9 s at 100 000 nodes / 154 978 edges \
    (3.1 s at 10 000); nothing was run above it, so it is a lower bound on the wall, and one \
    timing on one host is not a median",
};
