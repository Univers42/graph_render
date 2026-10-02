//! Ledger metadata for the Graphviz packing engines, kept apart from `registry.rs` for
//! the house line cap and out of `registry/radial.rs` because the oracle is neither
//! networkx nor a radial layout: `layout.packing.osage` is Graphviz's own `osage`, reached
//! through the docker-only oracle image (`docs/decisions/graphviz-oracle.md`), and it is
//! closed form over rectangles rather than over a tree.

use super::Metadata;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// Node count `layout.packing.osage` was run at, the largest size `graph-cli bench`
/// accepts.
///
/// **Measured**, `--release`, `--repeat 3` medians on one host, with
/// `scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout
/// layout.packing.osage --n 220,10000,100000,1000000 --past-ceiling --repeat 3`: 0.00 ms at
/// 220 nodes, 0.03 ms at 10 000, 1.00 ms at 100 000 (154 978 edges) and 8.94 ms at 1 000 000
/// (1 549 929 edges) — a flat ~9 ns per node, and the only reason it is not faster than the
/// clock's resolution is that the layout reads no edges at all. The table is in
/// `docs/measurements/p13-gv1-osage.md`. 1 000 000 is the largest size `bench` accepts and
/// where it was run, not where it was found to stop working, so this is a measured lower
/// bound — nothing about the layout is quadratic.
pub const OSAGE_CEILING: u64 = 1_000_000;

const DEGRADATION: &str = "past the ceiling wasm32 cannot allocate and the module traps (no \
partial result); natively, memory permitting, the snapshot refuses with \
SnapshotError::Capacity once an id table's text would pass 2^32-1 bytes — a refusal, never a \
wrap or a truncation. Nothing degrades *within* the ceiling: the layout is closed form with \
no iteration budget, no cut-off and no force model, and it never reads an edge, so a larger \
graph costs more time and returns the same kind of answer";

pub(super) const OSAGE: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "Graphviz 16.1.0 osage -Tplain -Gstart=1 in the docker-only ge-graphviz-oracle \
image (pinned by sha256 in scripts/orch/fetch-refs.sh), over the same 1000 seeded fixtures, \
compared by harness/oracle-graphviz.py --differential: the largest absolute node-coordinate \
difference in points after both arms are rescaled to the same bounding box. THE TWO ARMS \
AGREE, at the resolution the oracle can print. The fixtures give every node an explicit box \
and the DOT pins it (fixedsize=true, width, height, label=\"\", margin=0), so Graphviz sizes \
each box from the attribute instead of from its rendered label, and width+height rises \
strictly with the node index, so arrayRects's qsort has no tie to break and glibc's unstable \
order never enters. Worst gap 6.31e-2 points at seed 574 (n = 576), ceiling 1e-1, the next \
power of ten above it and never widened to pass. That worst gap is the oracle's own printed \
resolution and not a disagreement: -Tplain writes five significant digits \
(lib/common/output.c:129-141), so at the largest fixture drawing, 18.9 inches, one printed \
digit is 0.001 inch = 0.072 points, and the worst gap is 0.88 of one digit; the median is \
3.99e-2 and no seed's gap reaches one cell stride (58 points), which an algorithmic \
difference would. Both former causes are removed at the fixture rather than absorbed by a \
ceiling: label-sized boxes (54 points for n0..n9, 57.942 for n10..n99) and qsort's tie order. \
Before that the arms differed by 122..1785 points on 982 of the 1000 seeds, at ceiling 1e4. \
-Gstart is INERT for this engine (measured: the same fixture hashes identically at start=1, \
7, 99), so none of the residual is seed drift. The six analytically determined small cases \
(one node, two nodes, a 3-path, a 4-cycle, a 5-star and a 6-branch) are additionally compared \
byte for byte at the plain format's own printed precision against the DEFAULT nodesize -- ours \
in crates/graph-core/src/layout/graphviz/osage/tests.rs, Graphviz's in the harness -- and all \
six match exactly, because six closed cases are all below eleven nodes and every box ties \
there. Full numbers and commands: docs/measurements/p13-gv1-osage.md",
    complexity: "O(n) and no more: the grid size is one sqrt, the gather is one ordered pass \
over the node count, and the edges are never read, so m does not appear at all. The sized \
entry point run_sized adds one sort of the n boxes and two linear passes, so it is O(n log n) \
in the node count and still never reads an edge",
    scale_ceiling: OSAGE_CEILING,
    degradation: DEGRADATION,
    ponytail: "Ponytail (node box size -- \
the one that used to cost the agreement): the REGISTERED path takes every rectangle to be \
Graphviz's default 0.75 x 0.5 inch nodesize, which holds only while every node's label fits \
inside the minimum and stops holding at eleven nodes, where n10's box is 57.942 points wide. \
Failing input: any graph of eleven or more nodes whose ids carry three or more characters. \
Direction: a different drawing, not a worse one -- the uniform grid is 58 x 40 where the \
reference's columns vary. Escape hatch: run_sized, which takes the boxes explicitly and is \
what the differential runs; the width itself is a font metric of Graphviz's own text layout \
and graph-core has no font engine, so the fixtures pin the size instead of computing it. \
Ponytail (the sized path is not a gather, D10): unlike the uniform grid, where node i's cell \
depends only on i and the node count, a node's cell under run_sized depends on which column \
and row the SORT put it in, and a column's width is a maximum over every box in it -- so one \
coordinate is a function of all the sizes. It is still deterministic (the fixture table makes \
width+height strictly increasing, so the sort is a total order, and every pass is in one fixed \
order) but D10's per-node gather does not describe that path and grid.rs still does. \
Ponytail (qsort tie order): arrayRects sorts by width+height with a qsort that is not stable, \
so among tied boxes the cell order would be glibc's. The fixture table removes the tie rather \
than guessing at it; below eleven nodes in a label-sized graph every box still ties, and there \
the sort cannot move the geometry, only the names. Ponytail (attributes): the reference reads \
pack, packmode and nodesize and the registered path reads none, so it always packs the array \
the reference's defaults pack -- the default answer, not a different one. Failing input: a \
Graphviz graph carrying packmode=\"node\" or a nodesize. Escape hatch: run_sized, which is \
where an explicit size enters. Ponytail (clusters): the reference packs subclusters recursively \
and this port has no clusters, the motor's Topology being a flat node set. Failing input: a DOT \
graph with subgraph cluster_*. Direction: the clusters become plain nodes in one flat array \
instead of nested boxes -- a different drawing, still a valid one. Escape hatch: none needed, \
the motor has no cluster concept. Ponytail (scale_ceiling): a measured lower bound -- see \
OSAGE_CEILING",
};
