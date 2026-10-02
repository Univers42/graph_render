//! Ledger metadata for the igraph-derived force layouts, kept apart from `registry.rs`
//! for the house line cap. Rules and reference pin: `docs/decisions/layouts-igraph.md`.

use super::Metadata;
use crate::layout::force::davidson_harel::DH_CEILING;
use crate::layout::force::drl::DRL_CEILING;
use crate::layout::force::fruchterman_reingold::FR_CEILING;
use crate::layout::force::graphopt::GRAPHOPT_CEILING;
use crate::layout::force::kamada_kawai::KK_CEILING;
use crate::layout::force::lgl::LGL_CEILING;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};
pub(super) const FRUCHTERMAN_REINGOLD: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "python-igraph 0.11.9 Graph.layout_fruchterman_reingold (dim=2, niter=500) in the \
    ge-python-oracle image, started from our start positions; harness/oracle-igraph.py compares \
    layout stress (ours/igraph, ceiling in graph-cli oracle_python/igraph.rs), not coordinates: \
    igraph's noise comes from its own generator, ours from a counter hash",
    complexity: "O(niter * (n^2 + m)) with niter = 500, dense repulsion; O(n + m) memory",
    scale_ceiling: FR_CEILING,
    degradation: "past the ceiling there is no refusal: the dense loop still returns finite \
    geometry, only slower (quadratically), so the caller applies its own timeout; igraph's grid \
    variant for n > 1000 is not implemented; a non-finite position refuses with \
    StageError::NonFinite",
    ponytail: "Ponytail: chaotic like every force layout - one added node is a different \
    picture, not a perturbed one; the direction is cosmetic, never silently wrong. Ponytail \
    (scale_ceiling): estimated from the O(500 n^2) operation count, not measured at 2 000. \
    Ponytail (noise): a counter hash stands in for igraph's generator, so coordinates differ \
    from igraph's even from the same start",
};

pub(super) const KAMADA_KAWAI: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "python-igraph 0.11.9 Graph.layout_kamada_kawai (dim=2, default maxiter 50 n) in the \
    ge-python-oracle image, started from our circle start; harness/oracle-igraph.py compares \
    layout stress (ours/igraph, ceiling in graph-cli oracle_python/igraph.rs)",
    complexity: "O(n^2) set-up (all-pairs BFS, one n x n matrix) + O(50 n * n) moves, so O(n^2)",
    scale_ceiling: KK_CEILING,
    degradation: "past the ceiling there is no refusal: memory grows as 8 n^2 bytes and time as \
    50 n^2, so a caller applies its own timeout; an edgeless graph with n >= 2 takes every \
    distance as 1 instead of dividing by zero; a non-finite position refuses with \
    StageError::NonFinite",
    ponytail: "Ponytail: Newton descent finds a local minimum of the spring energy, so a folded \
    start can settle folded - cosmetic, never invalid. Ponytail (scale_ceiling): estimated from \
    the operation count and matrix size, not measured at 2 000. Ponytail (edgeless): the \
    all-distances-1 fallback is our choice; igraph yields non-finite output there",
};

pub(super) const FRUCHTERMAN_REINGOLD_3D: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "python-igraph 0.11.9 Graph.layout_fruchterman_reingold (dim=3, niter=500) in the \
    ge-python-oracle image — the dimension SciGraphs actually asks for \
    (igraph_layouts.py:74), so this is the row the conformance matrix compares; \
    harness/oracle-igraph.py compares layout stress (ours/igraph, ceiling in graph-cli \
    oracle_python/igraph.rs), not coordinates: the start is a random box and igraph's generator \
    is out of licence, so the two streams part company at the first coordinate",
    complexity: "O(niter * (n^2 + m)) with niter = 500, dense repulsion; O(n + m) memory",
    scale_ceiling: FR_CEILING,
    degradation: "past the ceiling there is no refusal: the dense loop still returns finite \
    geometry, only slower (quadratically), so the caller applies its own timeout; igraph's grid \
    variant does not exist in 3D and this port has no grid at either dimension; a non-finite \
    position refuses with StageError::NonFinite naming the axis it sat on",
    ponytail: "Ponytail: chaotic in three dimensions rather than two — one added node is a \
    different picture, not a perturbed one; the direction is cosmetic, never silently wrong. \
    Ponytail (scale_ceiling): the 2D stage's estimate, carried over: the operation count is the \
    same and nothing here was measured at 2 000. Ponytail (noise): a counter hash stands in for \
    igraph's generator, so coordinates differ from igraph's even from the same box. Ponytail \
    (disconnected pair term): the spec records a mistyped axis in igraph's own 3D disconnected \
    correction; this port uses the intended axis and says so in the spec",
};

pub(super) const KAMADA_KAWAI_3D: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "python-igraph 0.11.9 Graph.layout_kamada_kawai (dim=3, default maxiter 50 n) in \
    the ge-python-oracle image — the dimension SciGraphs actually asks for \
    (igraph_layouts.py:99), so this is the row the conformance matrix compares; \
    harness/oracle-igraph.py compares layout stress (ours/igraph, ceiling in graph-cli \
    oracle_python/igraph.rs)",
    complexity: "O(n^2) set-up (all-pairs BFS, one n x n matrix) + O(50 n * n) moves, so O(n^2)",
    scale_ceiling: KK_CEILING,
    degradation: "past the ceiling there is no refusal: memory grows as 8 n^2 bytes and time as \
    50 n^2, so a caller applies its own timeout; an edgeless graph with n >= 2 takes every \
    distance as 1 instead of dividing by zero, which is where igraph returns non-finite output \
    and this port does not; a non-finite position refuses with StageError::NonFinite naming the \
    axis it sat on",
    ponytail: "Ponytail: Newton descent finds a local minimum of the spring energy, so a folded \
    start can settle folded - cosmetic, never invalid. Ponytail (scale_ceiling): the 2D stage's \
    estimate, carried over: the same matrices and the same move count, nothing measured at 2 000. \
    Ponytail (sphere start): igraph places vertices on a spiral and this port follows the spec's \
    written formula for it, so the two starts agree; the port's 2D circle is its own and is not \
    the one igraph uses - the 2D id is pinned byte for byte and is not re-derived here",
};

pub(super) const GRAPHOPT: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "python-igraph 0.11.9 Graph.layout_graphopt (niter=500) in the ge-python-oracle \
    image, started from our start positions; harness/oracle-igraph.py compares layout stress \
    (ours/igraph, ceiling in graph-cli oracle_python/igraph.rs), not coordinates",
    complexity: "O(niter * (n^2 + m)) with niter = 500; O(niter * m) when node_charge is zero; \
    O(n + m) memory",
    scale_ceiling: GRAPHOPT_CEILING,
    degradation: "past the ceiling there is no refusal: the pair loop still returns finite \
    geometry, only slower (quadratically), so the caller applies its own timeout; pairs 500 or \
    more apart stop repelling; a non-finite position refuses with StageError::NonFinite",
    ponytail: "Ponytail: no cooling and no convergence test, so the run stops after 500 steps \
    wherever it is, possibly still oscillating at the movement cap - cosmetic, never invalid. \
    Ponytail (cut-off): the 500-unit repulsion limit is the original's heuristic and wrong for \
    layouts much wider than that. Ponytail (scale_ceiling): estimated from the operation \
    count, not measured at 2 000",
};

pub(super) const DAVIDSON_HAREL: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "python-igraph 0.11.9 Graph.layout_davidson_harel (defaults) in the ge-python-oracle \
    image, started from our start positions; harness/oracle-igraph.py compares layout stress \
    (ours/igraph, ceiling in graph-cli oracle_python/igraph.rs), not coordinates: annealing \
    draws differ between the two generators",
    complexity: "O(rounds * 30 * n * (n + deg(v) m)) with 10 rounds by default; \
    O(n + m) memory",
    scale_ceiling: DH_CEILING,
    degradation: "past the ceiling there is no refusal: rounds still return finite geometry, \
    only slower (cubic in n for dense graphs), so the caller applies its own timeout; an empty \
    graph returns empty geometry; a non-finite position refuses with StageError::NonFinite",
    ponytail: "Ponytail: simulated annealing finds a local optimum only, and quality depends on \
    weights igraph itself calls graph dependent - cosmetic, never invalid. Ponytail (zero \
    distance): squared distances are floored at 1e-12 where the source divides by zero. \
    Ponytail (scale_ceiling): estimated from the operation count, not measured at 500",
};

pub(super) const LGL: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "python-igraph 0.11.9 Graph.layout_lgl (defaults) in the ge-python-oracle image; \
    harness/oracle-igraph.py compares layout stress (ours/igraph, ceiling in graph-cli \
    oracle_python/igraph.rs), not coordinates: root choice and scatter draw from different \
    generators",
    complexity: "O(L * maxit * (m + n * c)) over L breadth-first layers, c the vertices in \
    neighbouring cells; O(n + m) memory",
    scale_ceiling: LGL_CEILING,
    degradation: "past the ceiling there is no refusal: layers still anneal, only slower, so \
    the caller applies its own timeout; vertices unreachable from the root keep their random \
    start; a non-finite position refuses with StageError::NonFinite",
    ponytail: "Ponytail: cell-limited repulsion means far clusters never push each other, so a \
    disconnected graph is drawn badly - cosmetic, never invalid. Ponytail (scale_ceiling): \
    estimated from the operation count, not measured at 1 000",
};

pub(super) const DRL: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "python-igraph 0.11.9 Graph.layout_drl (default preset) in the ge-python-oracle \
    image; harness/oracle-igraph.py compares layout stress (ours/igraph, ceiling in graph-cli \
    oracle_python/igraph.rs), not coordinates: igraph works in single precision and the \
    liquid stage amplifies any difference",
    complexity: "O(S * n * (deg + 441)) with S about 550 sweeps and a fixed 1000 x 1000 grid \
    (8 MB) regardless of n",
    scale_ceiling: DRL_CEILING,
    degradation: "past the ceiling there is no refusal: sweeps stay linear in n, only slower; \
    a node outside the plane is held by the border wall instead of raising an error; a \
    non-finite position refuses with StageError::NonFinite",
    ponytail: "Ponytail: sequential index-ordered sweeps make the picture depend on node \
    numbering, and edge cutting is permanent and one-directional - cosmetic, never invalid. \
    Ponytail (scale_ceiling): estimated from the operation count, not measured at 5 000",
};
