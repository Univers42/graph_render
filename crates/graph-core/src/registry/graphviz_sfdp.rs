//! Ledger metadata for `layout.force.sfdp`, in its own file.
//!
//! Separate from `force.rs` so that the parallel Graphviz engine jobs each touch
//! `registry.rs` with a one-line additive `mod` and a `LAYOUTS` append and nothing else.
//!
//! This is the Graphviz engine, **not** SciGraphs' own `YIFAN_HU`: SciGraphs reaches `sfdp`
//! through `_graphviz_engine_layout` (`SciGraphs/core/scigraphs_core/mesh/layouts/yifan_hu.py:340`)
//! and the motor already ships that separate algorithm as `layout.force.yifan_hu`. It shares the
//! oracle with the other Graphviz engines — Graphviz's own engine in the docker-only
//! `ge-graphviz-oracle` image, `docs/decisions/graphviz-oracle.md` — and differs from them in
//! the one way that decides this row's `oracle` string: `sfdp` is **seed-sensitive**, so its
//! output is compared as a *measurement* rather than as a closed form.

use super::Metadata;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// Node count `layout.force.sfdp` was run at, and the largest size measured end to end.
///
/// **Measured**, `--release` on one host with
/// `scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout layout.force.sfdp`:
/// 66.5 ms at 220 nodes, 570 ms at 2 000, 3.47 s at 10 000 and 23.9 s at 50 000 (the last
/// `--repeat 1`, because `--repeat 3` there costs 72 s per repeat). The full table and the two
/// measurements that shaped the implementation are in `docs/measurements/p13-gv2-sfdp.md`.
///
/// **This is the largest size measured, not a limit**, and the difference is the point: the
/// cost is superlinear, growing roughly as `n^1.3` across the range above, because the
/// Barnes-Hut tree is rebuilt from scratch on every one of up to 500 iterations and the
/// multilevel hierarchy re-solves at each of its `O(log n)` levels. Extrapolating that fit to
/// the 1 000 000 nodes the other engines reach would predict on the order of half an hour for
/// one layout — which is why the ceiling is stated at the size that was actually run rather
/// than at a number no run here supports. Nothing *breaks* above it; it becomes slow, and a
/// ledger row claiming otherwise would be a claim nobody measured.
pub const SFDP_CEILING: u64 = 50_000;

const DEGRADATION: &str = "past the ceiling wasm32 cannot allocate and the module traps (no \
partial result); natively, memory permitting, the snapshot refuses with \
SnapshotError::Capacity once an id table's text would pass 2^32-1 bytes — a refusal, never a \
wrap or a truncation. Within the ceiling nothing degrades either: the layout always runs its \
full multilevel schedule to the same convergence test, so a bigger graph costs more time and \
returns the same kind of answer. The one thing that does change with size is the *drawing*: \
the multilevel hierarchy has more levels to descend, so the quality of a very large layout \
depends on the coarsening matching the reference randomises and this port does not (see \
`ponytail`), which is a quality limit and not a correctness one";

pub(super) const SFDP: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "Graphviz 16.1.0 sfdp -Tplain -Gstart=1 in the docker-only ge-graphviz-oracle image \
(pinned by sha256 in scripts/orch/fetch-refs.sh), over the same 1000 seeded fixtures, compared \
by harness/oracle-graphviz.py --differential: the largest absolute node-coordinate difference in \
points after both arms are rescaled to the same bounding box. **This engine is \
SEED-SENSITIVE**, measured not assumed: the same fixture hashes differently at -Gstart=1, 7 \
and 99, so there is no closed answer to gate on and no seed-stability row to gate on either. \
The decisive measurement is that the oracle compared **against itself** at -Gstart=7 rather \
than 1 differs by up to 4.81e+2 points on this very metric over the same 1000 seeds, which is \
LARGER than the 3.88e+2 gap our own arm shows and is the spread a port that does not draw \
glibc's exact coarsening-permutation stream cannot get below; the measured worst gap and its \
ceiling are in docs/measurements/p13-gv2-sfdp.md. One small case IS a \
closed answer: the one-node graph, where the oracle returns the same 0.375 x 0.25 inch box \
centre at every seed measured, and our arm is pinned to it in \
crates/graph-core/src/layout/graphviz/sfdp/tests.rs. The two-node, 3-path, 4-cycle, 5-star \
and 6-branch cases are **not** closed for this engine — their answers move with the seed, \
measured at three different separations for -Gstart 1, 7 and 99 — so they are not compared \
byte for byte, and the row is Status::Implemented rather than gated",
    complexity: "O((n + m) log n) per iteration over the multilevel hierarchy: each level \
rebuilds a Barnes-Hut quadtree and gathers one force per node, and the hierarchy is O(log n) \
levels deep. The dominant term is the iteration count, not the per-iteration cost: the \
reference's loop runs to its own convergence test (up to 500 iterations) and the tree is \
rebuilt on each one, which is what makes the measured cost grow as roughly n^1.3 across 220 to \
50 000 nodes rather than as the n log n of a single pass",
    scale_ceiling: SFDP_CEILING,
    degradation: DEGRADATION,
    ponytail: "Ponytail (coarsening matching): the reference draws a random permutation to \
order its multilevel matchings and re-seeds between levels (lib/sfdpgen/Multilevel.c, via \
gv_permutation); this port matches in dense index order, which yields one of the maximal \
matchings the reference could have drawn but not necessarily the one it drew. Failing input: \
every graph, in the last digits. Direction: the drawing is a different but equally valid sfdp \
layout of the same graph, not a wrong one — and that is why the differential records a \
measured gap rather than asserting agreement. Escape hatch: run_seeded is the seam; a port \
that drew glibc's permutation stream would only have to replace multilevel::coarsen and \
re-seed per level. Ponytail (the random start): the start positions ARE glibc's, implemented \
and pinned against the system libc in start.rs, because a merely similar recurrence would \
start every layout somewhere else and still pass a determinism test. Ponytail (attributes): \
the reference reads K, levels, quadtree, smoothing and rotation; this port reads none of \
them, because the motor's Topology carries no attribute channel to a layout at its defaults \
(the same decision layout::radial::twopi records), so all five run at their default values. \
Ponytail (scale_ceiling): a measured lower bound, not a failure point — see SFDP_CEILING",
};
