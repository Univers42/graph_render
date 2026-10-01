//! Ledger metadata for `layout.treemap.patchwork`, in its own file.
//!
//! Separate from `radial.rs` so that the parallel Graphviz engine jobs each touch
//! `registry.rs` with a one-line additive `mod` and a `LAYOUTS` append and nothing else.
//!
//! `radial.rs` holds the engines whose drawing is organised around a centre; this one is
//! organised around an area. It shares the oracle with them — Graphviz's own engine in the
//! docker-only `ge-graphviz-oracle` image, `docs/decisions/graphviz-oracle.md` — and
//! differs in the two ways that matter for a metadata row: the geometry is a set of squares
//! packed into a field, so `edges` are lines between centres and the layout says nothing
//! about them; and it reads no attribute at all, so its ceiling is a pure node count.

use super::Metadata;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// Node count `layout.treemap.patchwork` was run at, the largest size `graph-cli bench`
/// accepts.
///
/// **Measured**, `--release`, `--repeat 3` medians on one host, run twice, with
/// `scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout
/// layout.treemap.patchwork --n 220,10000,100000,1000000 --repeat 3`: 0.01/0.00 ms at 220 nodes,
/// 0.12 ms at 10 000, 1.96/1.88 ms at 100 000 and 20.58/21.25 ms at 1 000 000 — a flat ~20 ns
/// per node from 10 000 up and a decade-to-decade factor of ~10, which is the `O(n)` in
/// `complexity` showing up in the timings. The full table is in
/// `docs/measurements/p13-gv1-patchwork.md`.
///
/// The layout reads no edge at all, so the timings are flat per *node* with no `m` term —
/// which is also why the ceiling is a node count and nothing else. 1 000 000 is the largest
/// size `bench` accepts and where it was run, not where it was found to stop working, so
/// this is a measured lower bound.
pub const PATCHWORK_CEILING: u64 = 1_000_000;

const DEGRADATION: &str = "past the ceiling wasm32 cannot allocate and the module traps (no \
partial result); natively, memory permitting, the snapshot refuses with \
SnapshotError::Capacity once an id table's text would pass 2^32-1 bytes — a refusal, never a \
wrap or a truncation. Nothing degrades *within* the ceiling: the layout has no iteration \
budget, no cut-off and no force model, so a larger graph costs more time and returns the \
same kind of answer";

pub(super) const PATCHWORK: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "Graphviz 16.1.0 patchwork -Tplain -Gstart=1 in the docker-only ge-graphviz-oracle \
image (pinned by sha256 in scripts/orch/fetch-refs.sh), over the same 1000 seeded fixtures, \
compared by harness/oracle-graphviz.py: the largest absolute node-coordinate difference in \
points after both arms are rescaled to the same bounding box, measured at 6.61e-2 over the \
sweep (ceiling 1e-1). -Gstart is INERT for this engine (measured: the same fixture hashes \
identically at start=1, 7 and 99, all 1000 seeds), so the comparison is against Graphviz's \
own closed form and not against a seed drift. The five analytically determined small cases \
(one node, two nodes, a 3-path, a 4-cycle and a 5-star) are additionally compared node by \
node against the closed form — ours in crates/graph-core/src/layout/graphviz/patchwork/tests.rs, \
Graphviz's printed lines recorded in docs/measurements/p13-gv1-patchwork.md. Byte-exactness \
against the oracle's own text is NOT claimed and is not reachable: -Tplain prints five \
significant digits, and Graphviz's drawing extent differs from the closed form by up to \
3.62e-3 pt (measured over isolated graphs, n = 1..100), which moves that fifth digit",
    complexity: "O(n): one squarified fill of the field, each row closed once and each node \
written at its own index. No edge is read, so there is no m term",
    scale_ceiling: PATCHWORK_CEILING,
    degradation: DEGRADATION,
    ponytail: "Ponytail (area and inset): Graphviz takes each node's area from the `area` \
attribute and each cluster's margin from `inset`; this port always uses the default area \
of 1 and no inset, because the motor's Topology and graph-contract's node and edge records \
carry neither field. Failing input: any DOT with a node `area` other than 1, or a cluster \
`inset`. Direction: the tiling is wrong — the field is still a correctly filled square, no \
node escapes it and no edge is mis-drawn, so the error is cosmetic. Escape hatch: close this \
by widening graph-contract, not here; the differential's fixtures are flat graphs on both \
arms, the only place the two can be compared. Ponytail (clusters): the reference tiles a \
cluster tree; with no cluster membership in Topology there is no tree to walk, so a flat \
graph is the whole input space. Ponytail (oracle resolution): -Tplain prints five \
significant digits, so no comparison can be tighter than the printed quantum at the \
drawing's own size — 0.072 points at the largest gate drawing, 10.34 inches across. The \
measured worst gap is 0.92 of that quantum, so the ceiling reflects the oracle's formatter \
rather than a disagreement; it is in docs/measurements/p13-gv1-patchwork.md. \
Ponytail (scale_ceiling): a measured lower bound — see PATCHWORK_CEILING",
};
