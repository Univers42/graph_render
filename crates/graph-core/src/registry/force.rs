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

/// Node count past which `layout.force.spring` stops being usable, and why it is this one
/// — above [`FA2_CEILING``, and for the same reason: the iteration count, not the
/// arithmetic.
///
/// **Time-bound, and bracketed by measurement rather than extrapolated**
/// (`docs/measurements/p12-t2.md`; `cargo run --release -p graph-cli -- bench --layout
/// layout.force.spring --n 220,1000,2000,5000,10000 --repeat 3 --past-ceiling`, plus a
/// second run over `--n 14000,20000`, which times the registered run only). Natively,
/// release, x86_64, inside the toolchain image, `--repeat 3` medians: 220 / 329 edges
/// 13.21 ms, 1 000 / 1 541 264.52 ms, 2 000 / 3 075 1 055.65 ms, 5 000 / 7 721
/// 3 345.76 ms, 10 000 / 15 474 9 638.01 ms, 14 000 / 21 712 20 183.17 ms, 20 000 /
/// 31 048 **39 893.49 ms**. Under the 30-second budget `FA2_CEILING` uses, both ends of
/// this ceiling were run: 14 000 is **under** it at 20.2 s and 20 000 is **over** it at
/// 39.9 s, so 16 000 is a figure with a measurement on each side of it rather than a
/// projection off the last point. (A pure `O(n^2)` fit to those seven points puts the
/// 30-second crossing near 18 000 and the last steps are noisy in both directions — 10 000
/// -> 14 000 is 2.09x for 1.4x the nodes, steeper than quadratic, because the early exit
/// bites unevenly as the graphs differ. The ceiling is set on the bracketing, not on the
/// fit.)
///
/// Two shapes of limit, and the second is the one a caller is likelier to hit: above
/// **500 nodes** `spring_layout` stops being the algorithm this port implements
/// (`method="auto"`, `layout.py:140-141` — the energy minimiser takes over). 500 is far
/// below any time ceiling, so the honest statement is that the port's oracle stops
/// describing it at 500, not that it stops working.
///
/// Ponytail (scale_ceiling): this is a *time* ceiling with no memory wall behind it: the
/// repulsion visits each unordered pair once and materialises no `n x n` matrix. Past it
/// nothing refuses — the layout keeps returning finite geometry, just for tens of seconds
/// — so a caller must apply its own timeout, the same bargain [`FA2_CEILING`] strikes.
/// Timings are this one host's medians; another host moves every digit and the bracketing
/// is what carries the claim, not the digits.
pub const SPRING_CEILING: u64 = 16_000;

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

/// Node count past which `layout.force.particle_mesh` stops being usable: the whole
/// 112-tick stage inside the 60-second budget [`FORCE_CEILING`] uses.
///
/// Time-bound, measured (`docs/measurements/perf-p2-pm.md`; `graph-cli tick --layout
/// particle-mesh --n <n>` and `bench --layout layout.force.particle_mesh --n <n>`).
pub const PM_CEILING: u64 = 1_000_000;

pub(super) const PARTICLE_MESH: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "graph-cli stress --oracle d3 --layout layout.force.particle_mesh (record \
    stress-pm): the frozen d3-force@3.0.0 force set at TICKS=112, Barnes-Hut's link, center \
    and integrate called as they are, many-body summed on a P x P mesh by FFT and collide \
    resolved over a hashed cell list; held to the same margin -0.05 against d3 as \
    Barnes-Hut. Unit-checked against the direct sum (field rms < 5% at range), the \
    pairwise collide scan (equal within 1e-9) and Barnes-Hut's settled link length (5%)",
    complexity: "O(n + P^2 log P) per tick x TICKS=112, P = clamp(next_pow2(ceil(sqrt n)), \
    128, 1024): CIC deposit and read O(n), two P x P FFTs, collide a counting sort plus nine \
    buckets per node, link O(m)",
    scale_ceiling: PM_CEILING,
    degradation: "past the ceiling there is no refusal: the stage returns finite geometry \
    after more than the 60-second budget, and the mesh side stays capped at 1024, so cells \
    widen and more of the many-body force falls in the smoothed range. A non-finite \
    position refuses with StageError::NonFinite",
    ponytail: "Ponytail (mesh): the charge force is the law convolved at cell resolution, \
    smooth below about two cells; failing input is a dense cluster several nodes per cell \
    wide, where nodes repel less than under Barnes-Hut; direction UNDER-SPREADING at small \
    scale, graded by the stress-pm record; escape hatch layout.force.barnes_hut. Force \
    layouts are chaotic as for Barnes-Hut: a topology change is a different picture",
};

pub(super) const SPRING: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "networkx 3.6 spring_layout at dim=2 (networkx/drawing/layout.py:452-651), the \
    method='force' branch _fruchterman_reingold (layout.py:660-727) at its own defaults \
    (iterations=50, threshold=1e-4, scale=5.0, k=sqrt(1/n), d clipped to 0.01) — the call \
    SciGraphs' _spring_layout_2d makes (networkx_layouts.py:16-24) and nothing more. Compared \
    in the ge-python-oracle image (pinned networkx 3.6, numpy 2.3.3) over 1000 seeds by \
    harness/oracle-spring.py, and NOT by coordinate gap: the port starts from graph-core's \
    seeded Mulberry32 rather than numpy's RandomState (D5) and diverges inside the first \
    step, so the gate is a stress metric — Pearson correlation between BFS hop distance and \
    Euclidean distance, each arm scored separately over 32 max-min pivots by graph-core's \
    own crates/graph-cli/src/stress/metric.rs (the harness writes the reference's positions \
    and computes nothing), and the gated quantity is the deficit max(0, theirs - ours), \
    not a ratio of ours to theirs, since both correlations may be negative. The ceiling is \
    on the MEDIAN deficit over seeds: 1e-1, the next power of ten above the 7.288e-3 \
    measured at 1000 seeds, beside a p90 of 6.950e-2 and a worst of 1.861e-1 (at seed 17) \
    which are recorded and not gated, because the extreme of 1000 small-graph samples is \
    tail and a ceiling admitting it could not be falsified. Measured: 998 of 1000 seeds \
    correlated (docs/measurements/p12-t2.md). What carries the gate beside that number is \
    exact: the reference's own rescale contract and its one-node answer, asserted on BOTH \
    arms, plus the analytically-determined small cases (one node, two nodes, a path, a star) \
    in layout/force/spring/tests.rs. Four departures from the reference, all stated in the \
    module doc: the start stream, the split reduction, the undirected simple graph with A \
    in {0,1}, and the n >= 500 fork where networkx's method='auto' switches to the L-BFGS \
    energy minimiser this port does not reproduce. Not Graphviz neato and never compared \
    against it (p13-gv2-neato owns that)",
    complexity: "O(n^2) per iteration x iterations=50, so O(50 n^2) overall; the dense all-pairs \
    repulsion dominates and the attraction is O(m) inside it",
    scale_ceiling: SPRING_CEILING,
    degradation: "past the ceiling there is no refusal and no trap either: the dense all-pairs \
    repulsion still returns finite geometry, it just takes tens of seconds and keeps growing \
    quadratically (9.64 s measured at 10 000 nodes, 39.89 s at 20 000), so the caller must \
    apply its own timeout. \
    Two refusals do exist and both are the reference's own: a graph of fewer than 2 nodes never \
    enters the force loop and is the centre or the empty layout (layout.py:618-624), and a \
    non-finite position refuses with StageError::NonFinite rather than reaching the snapshot",
    ponytail: "force layouts are CHAOTIC, identically to Barnes-Hut and FA2: one added node is \
    a different picture, not a perturbed one. Direction: cosmetic-but-surprising, never \
    silently wrong; the escape hatch is the same — a fixed seed and the stage's own \
    run-to-run determinism. Ponytail (above 500 nodes): networkx's method='auto' hands \
    n >= 500 to the L-BFGS energy minimiser (_energy_fruchterman_reingold, layout.py:814-880) \
    and this port does not reproduce it, so past that size the port and the reference are \
    different algorithms and the stress deficit compares two different questions. \
    Ponytail (the stress metric itself): a Pearson hop/euclid correlation certifies \
    distances and says nothing about orientation, so it cannot see a mirrored or rotated \
    but otherwise equivalent embedding — that is the strongest true claim a force layout \
    admits and the weakest one a caller may want. It is also a statistic, so the gate is \
    on its median over 1000 seeds (7.288e-3, ceiling 1e-1) and its worst (1.861e-1) is \
    recorded rather than gated: on a small graph 50 iterations from a random start is far \
    from settled, and the tail is noise. Ponytail (early exit): the iteration \
    count is an upper bound, not a fixed amount of work: run() stops as soon as a step's \
    total movement falls under networkx's own 1e-4 (layout.py:725-726), so a quiet graph \
    finishes in fewer than 50 iterations and two graphs of the same size can do different \
    amounts of work. Escape hatch, from both ends: SpringParams::iterations and \
    SpringParams::scale are parameters, so run_with lays the graph out at any budget and \
    emit-spring-fixtures --max-iter re-measures the whole comparison at another one; \
    GM_MUTATE_SPRING_ITERATIONS perturbs this stage against the same gate, so the claim is \
    falsifiable from the port's side",
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
