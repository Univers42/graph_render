//! Ledger metadata for `layout.force.fdp`, Graphviz's own force-directed placement.
//!
//! Its own file, and not `registry/force.rs`, for the house line cap and because its oracle
//! is neither networkx nor a sibling force model: `layout.force.fdp` is Graphviz 16.1.0's
//! `fdp` reached through the docker-only oracle image
//! (`docs/decisions/graphviz-oracle.md`), seeded by `-Gstart` and iterative where every row
//! in `force.rs` is closed form or networkx-shaped.
//!
//! Its own file also keeps the parallel Graphviz engine jobs from colliding: each engine's
//! metadata lives in its own module, and `registry.rs` takes one additive `mod` line and
//! one `LAYOUTS` append.

use super::Metadata;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

pub use crate::layout::graphviz::fdp::FDP_CEILING;

const DEGRADATION: &str = "past the ceiling there is no refusal and no trap either: the \
    expansion phase is O(pass1 * (m + grid work)) and the overlap phase is O(tries * n^2) \
    with tries = 9, so the geometry stays finite and the cost keeps growing with the square \
    of the node count -- measured at 727.58 ms for 220 nodes and 14 371.82 ms for 1 000, a \
    per-node cost that rises from 3.31 ms to 14.37 ms -- and the caller must apply its own \
    timeout. The ceiling is a measured lower bound, the largest size run, not a size at which \
    the layout was found to stop working. \
    One refusal does exist and it is the reference's own: a graph of fewer than 2 nodes \
    never enters the overlap phase (layout.c:852), and a non-finite position refuses with \
    StageError::NonFinite rather than reaching the snapshot. \
    Nothing degrades *within* the ceiling, and the prism overlap packing that would separate \
    the last overlaps is not ported at all (see the ponytail field), so a drawing inside the \
    ceiling can still carry a residual overlap";

pub(super) const FDP: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "Graphviz 16.1.0 `fdp -Tplain` at `-Gstart=1`, reached through the docker-only \
    image `ge-graphviz-oracle` by `harness/oracle-graphviz.py` and recorded as \
    `target/gv-fdp/graphviz-fdp.jsonl` (docs/decisions/graphviz-oracle.md). \
    Unlike `layout.twopi` and `layout.packing.osage` the seed is NOT inert: `initPositions` \
    seeds the whole initial placement from it (tlayout.c:487), and `-Gstart` 1, 7 and 99 \
    give three visibly different layouts, measured in docs/measurements/p13-gv2-fdp.md. So \
    the port reproduces the reference's `srand48`/`drand48` sequence exactly rather than \
    drawing from the house's own generator, and the two agree to the oracle's printed \
    quantum on the seeded placement itself. \
    The force phases are 300 cooled ticks of a Fruchterman-Reingold model, which is \
    compared by the largest absolute coordinate gap in points after both arms are rescaled \
    to one bounding box, plus the six analytically-determined small cases; and the port is \
    NOT gated on that number, for the reason in the ponytail field",
    complexity: "O(pass1 * (m + n)) for the expansion phase — pass1 = unscaled * maxIters / \
    100 = 300 ticks, the attraction O(m) each and the repulsion confined to the 9 grid cells \
    around each node at Cell = 3K — then O(tries * n^2) for the overlap phase, tries = 9, \
    which is the term that dominates past a few hundred nodes; O(n + m) memory",
    scale_ceiling: FDP_CEILING,
    degradation: DEGRADATION,
    ponytail: "force layouts are CHAOTIC, and for this one the oracle is not even \
    self-reproducible, which is the whole of what this row can honestly claim. Running the \
    pinned Graphviz 16.1.0 `fdp -Tplain -Gstart=1` twice over the same graph gives \
    byte-different output in the fifth significant digit for some inputs and not for others: \
    it is byte-stable for -Gmaxiter up to 99 and is not at the default 600, the divergence \
    survives -Goverlap=0 so it is in the expansion phase rather than in the packing, and the \
    six closed shapes are stable over four runs each. Two full positional runs over the same \
    1000 seeds are byte-identical on 961 and differ on 39, by up to 2.16e3 points, and whether a \
    given graph is stable is a property of that graph rather than of its size: seed 620 at \
    n = 22 gives 8 distinct outputs in 8 runs while seed 763 at n = 165 is byte-stable. A \
    300-tick cooled force model amplifies one bit of difference in the placement into a \
    different picture. Failing input: a graph whose expansion trajectory is unstable, which is \
    39 of the 1000 gate seeds and cannot be told in advance from size. Direction: a \
    *different* drawing, not a worse one — both arms run the same algorithm, so the measured \
    gap of 4.5e5 points is the reference's own spread and not a defect in this port, and the \
    metric is absolute against drawings up to 448 358 points across. The gap does not decay \
    gracefully with n either: 2.5e-5 points at n = 2 and 14 points at n = 3, so the \
    comparison stops being informative immediately rather than slowly. Escape hatch: none \
    inside the motor, and the row is therefore Status::Implemented and never gated; what would \
    make a ceiling meaningful is a reproducible oracle, or an oracle arm that runs the engine \
    twice and gates on the pair's own spread. The measured oracle self-gap is recorded in \
    docs/measurements/p13-gv2-fdp.md as the floor on any achievable agreement. \
    Ponytail (what the port does reproduce): the seeded initial placement is not iterative \
    and is exact — running the oracle at -Gmaxiter=1 -Goverlap=true prints that placement and \
    nothing else, and this port matches it to the oracle's printed quantum on the two-node and \
    three-node cases, which pins K, the box formula, the seed and the whole drand48 sequence \
    at once. The one-node case is exact to the last bit for a second reason: nothing moves, \
    and compute_bb's box corner puts it at half its default node. \
    Ponytail (hypot): the reference writes hypot() for every separation and this port writes \
    sqrt(dx*dx + dy*dy), because hypot is a libm function rather than an IEEE operation and \
    glibc's and wasm32's differ: with it in the repulsion kernel the hash gate's \
    native-against-wasm32 identity failed on 3 of 8 seeds, each arm agreeing with itself. \
    Failing input: any graph, on any target pair whose hypot differs. Direction: the last ulp \
    against the reference, amplified by 300 ticks into 2.7e-3 points on the two-node case. \
    Escape hatch: one line in layout::graphviz::fdp::distance, at the cost of that identity. \
    Ponytail (the prism overlap packing): removeOverlapAs(g, \"prism\") is not ported, so \
    whatever the nine x_layout tries leave touching, this port leaves where they put it. \
    Failing input: a drawing where the tries do not reach zero overlaps, which is exactly \
    the case prism exists for. Direction: a drawing with a residual overlap, i.e. two nodes \
    closer than the four-point margin. Escape hatch: the overlap count x_layout already \
    returns is the number to test. \
    Ponytail (disconnected graphs): the reference splits into connected components, runs \
    fdp_tLayout on each and packs them with putGraphs (layout.c:828-883); this port runs one \
    expansion over the whole node set, so the components repel each other and each one's \
    internal drawing is not the reference's. Failing input: any graph of two or more \
    components. Direction: a different drawing, still a force layout of the same graph. \
    Escape hatch: a connected-components pass over the edge list this module already builds. \
    Ponytail (clusters): the reference's expandCluster recursion and its boundary ports on \
    the enclosing ellipse are not ported. Failing input: any DOT graph with a \
    `subgraph cluster_*`. Direction: the cluster's nodes are laid out as ordinary nodes and \
    no cluster box is drawn. Escape hatch: none — the motor's Topology is a flat node set \
    with no cluster membership to lay out. \
    Ponytail (attributes): len, weight, overlap, sep, K, T0 and maxiter are all read by the \
    reference and by none of this port, because the motor publishes no attribute channel to \
    a layout at its default parameters. Every one is a documented default here and every one \
    is a different drawing. \
    Ponytail (node box size): the overlap test uses Graphviz's default nodesize of 0.75 x \
    0.5 inch, which is exact only while every node's label fits inside the minimum — the \
    limit layout.packing.osage records, biting at the same place. It reaches this port only \
    through the overlap phase's constants, so an overflowing label changes its X_ov and the \
    reference's differently. Escape hatch: none; there is no font engine in graph-core",
};
