//! Ledger metadata for the radial Graphviz engines, kept apart from `registry.rs` for the
//! house line cap.
//!
//! The first row here is `layout.twopi`, the only one ported so far. It is kept in a
//! `radial` module rather than appended to `closed_form` because its oracle is not
//! networkx: it is Graphviz's own engine, reached through the docker-only oracle image
//! (`docs/decisions/graphviz-oracle.md`), and because its units are points rather than
//! networkx's unit box.

use super::Metadata;
use super::bench_cap::MAX_BENCH_NODES;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// Node count `layout.twopi` was run at: the largest size `graph-cli bench` accepts,
/// which [`MAX_BENCH_NODES`] is, read here rather than written out so this figure and
/// the 3D ceiling cannot drift apart.
///
/// **Measured**, `--release`, `--repeat 3` medians on one host, with
/// `scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout layout.twopi
/// --n 220,10000,100000,1000000 --past-ceiling --repeat 3`: 0.05 ms at 220 nodes, 2.20 ms
/// at 10 000, 25.35 ms at 100 000 (154 978 edges) and 447.58 ms at 1 000 000 (1 549 929
/// edges), a flat ~0.45 us per node, so the `O(n + m)` in `complexity` is what the timings
/// show. The full table is in `docs/measurements/p13-gv1.md`. The figure is where
/// `bench` accepts and where the layout was run, not where it was found to stop working,
/// so this is a measured lower bound — nothing about the layout is quadratic.
///
/// Ponytail (scale_ceiling): what it gets wrong — this is a tool's parse range, not a wall
/// the layout found, and the timings behind it are one host's medians at one size.
/// Direction: it understates the ceiling, never overstates it. Escape hatch: raise
/// `MAX_BENCH_NODES` and re-measure, the one change that can move this number.
pub const RADIAL_CEILING: u64 = MAX_BENCH_NODES as u64;

const DEGRADATION: &str = "past the ceiling wasm32 cannot allocate and the module traps (no \
partial result); natively, memory permitting, the snapshot refuses with \
SnapshotError::Capacity once an id table's text would pass 2^32-1 bytes — a refusal, never a \
wrap or a truncation. Nothing degrades *within* the ceiling: the layout has no iteration \
budget, no cut-off and no force model, so a larger graph costs more time and returns the \
same kind of answer";

pub(super) const TWOPI: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "Graphviz 16.1.0 twopi -Tplain -Gstart=1 in the docker-only ge-graphviz-oracle \
image (pinned by sha256 in scripts/orch/fetch-refs.sh), over the same 1000 seeded fixtures, \
compared by harness/oracle-twopi.py: the largest absolute node-coordinate difference in points \
after both arms are rescaled to the same bounding box, measured at 7.10e-2 over the sweep \
(ceiling 1e-1). -Gstart is INERT for this engine (measured: the same fixture hashes \
identically at start=1, 7, 99 and with no -Gstart), so the comparison is against Graphviz's \
own closed form and not against a seed drift. The six analytically determined small cases (one \
node, two nodes, a 3-path, a 4-cycle, a 5-star and a 6-branch) are additionally compared byte \
for byte at the plain format's own printed precision — ours in \
crates/graph-core/src/layout/radial/twopi/tests.rs, Graphviz's in the harness — because a \
tolerance is weaker than the truth those cases carry",
    complexity: "O(n + m): two breadth-first searches (to the nearest leaf, then from the root), \
one parent walk per leaf, and one sweep per tree node over its own neighbours",
    scale_ceiling: RADIAL_CEILING,
    degradation: DEGRADATION,
    ponytail: "Ponytail (disconnected input): Graphviz lays out each connected component and then \
packs them apart with packSubgraphs; this port lays each component out around the origin and \
leaves them overlapping. Failing input: any graph with two components. Direction: overlap, the \
cosmetic one — every node still lands on its own ring at a finite point, and no edge is \
mis-drawn. Escape hatch: connect the graph before laying it out; the differential's fixtures are \
connected by construction, so the two arms are only ever compared where they agree. Ponytail \
(oracle resolution): -Tplain prints five significant digits, so the comparison cannot be tighter \
than about 7e-2 points at a drawing 17 inches across whatever the layout's f64 arithmetic is; \
the measured gap is in docs/measurements/p13-gv1.md and the ceiling is the next power of ten \
above it. Escape hatch: the six closed cases, compared byte for byte at the same printed \
precision, which is where the exactness lives. Ponytail (scale_ceiling): a measured lower \
bound — see RADIAL_CEILING",
};
