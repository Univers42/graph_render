//! Ledger metadata for the Phase 6 spectral family (`layout.spectral`,
//! `layout.mds.pivot`), kept apart from `registry.rs` for the house line cap.

use super::Metadata;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// Node count past which `layout.spectral` stops being reliable, and why it is this one.
///
/// Measured, release build (`docs/measurements/phase06-eigen.md`): the worst input is a
/// path, whose spectral gap is O(1/n^2). A 700-node path solves in 131 ms (747 LOBPCG
/// iterations); an 800-node path exhausts `maxiter = 1500` and fails the residual gate,
/// as do 900 to 4096. A 100x100 grid (10 000 nodes) solves in 1.3 s and the gate model
/// at 100 000 nodes in 6.9 s, so the ceiling is a property of the spectrum, not of n.
pub const SPECTRAL_CEILING: u64 = 700;

/// Node count past which `layout.mds.pivot` is not measured, and why it is this one.
///
/// Measured, release build, gate model: 273 ms at 10 000 nodes, 795 ms at 30 000 and
/// 4.3 s at 100 000 (`graph-cli bench`). Its O(n k) distance matrix is 80 MB at 100 000
/// nodes and k = 100; nothing was measured past 100 000, which is `graph-cli`'s own
/// `MAX_NODES`, and the two-figure wasm32 limit 4 GiB / (8 k B) = 5.3 M nodes is arithmetic, not a
/// measurement.
pub const PIVOT_MDS_CEILING: u64 = 100_000;

pub(super) const SPECTRAL: Metadata = Metadata {
    tier: 2,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "SciGraphs networkx_layouts._spectral_component_coordinates on scipy 1.16.2 \
(lobpcg above 256 nodes per component), compared per connected component as the largest \
principal angle between the two 2-column spans over the 1000 gate seeds \
(harness/oracle-spectral.py); tolerance, not bytes, because the two solvers differ",
    complexity: "O(c^3) per component of c <= 256 nodes (tred2/tql2); above that LOBPCG, \
O(iterations * (m + n b^2)) with block b = 4 and at most 1500 iterations",
    scale_ceiling: SPECTRAL_CEILING,
    degradation: "past the ceiling a path-like component (spectral gap O(1/n^2)) may not \
converge in 1500 iterations; it then fails the residual gate and is skipped, its nodes left at \
the origin and its packing cell, and the run is refused with StageError::Param if no component \
solved — never a random layout (C12). Grid-like and gate-model components converged to 100 000 \
nodes",
    ponytail: "Ponytail (solver): the block start uses a Weyl sequence and a constant-diagonal \
preconditioner, so LOBPCG converges slower than the reference's random start; failing input: a \
single path or cycle of 800 nodes or more. Direction: under-reporting is impossible (the \
residual and orthonormality gate decides, not the iteration count) and the failure is a skipped \
component, not a wrong one. Ponytail (sign): inside a degenerate eigenspace the chosen sign \
and rotation are an artifact of the solver, so a symmetric graph may be mirrored relative to \
the reference; cosmetic. Ponytail (scale_ceiling): measured on the spectrum above, see \
SPECTRAL_CEILING.",
};

pub(super) const PIVOT_MDS: Metadata = Metadata {
    tier: 2,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "SciGraphs networkx_layouts._pivot_mds_component_coordinates on scipy 1.16.2 and \
numpy 2.3.3, compared per connected component as the largest coordinate difference after \
peak normalisation over the 1000 gate seeds (harness/oracle-spectral.py); tolerance for f32 \
output and BLAS reduction order",
    complexity: "O(k (n + m)) time and O(n k) memory, k = min(100, n); the k x k eigenproblem \
is dense",
    scale_ceiling: PIVOT_MDS_CEILING,
    degradation: "past the measured ceiling nothing changes in kind: time and the n x k \
distance matrix grow linearly and wasm32 traps when it cannot allocate; unreachable pivots \
(other components) are zeroed, which distorts geometry rather than failing, and a component \
whose k x k solve fails the residual gate is skipped as in spectral",
    ponytail: "Ponytail (pivots): farthest-point selection starts at index 0 and breaks ties by \
the lowest index, so the pivot set is a heuristic covering, not an optimum; failing input: a \
graph whose farthest node is one of many equally far, which changes the pivots and so the \
picture, not its validity. Ponytail (distance): unreachable pairs count as 0 hops, which pulls \
nodes of different components together inside a block. Ponytail (scale_ceiling): the ceiling \
is the largest size measured, not a limit found.",
};

/// The 3D spectral arms' ceiling: the 2D family's, because the eigensolve is the same
/// `L = D - A` per component and only the number of eigenpairs kept changes. Re-measured
/// for the 3D run in `docs/measurements/p12-t4a.md`.
pub const SPECTRAL_3D_CEILING: u64 = SPECTRAL_CEILING;
/// The 3D pivot-MDS arm's ceiling: the 2D family's, for the same reason.
pub const PIVOT_MDS_3D_CEILING: u64 = PIVOT_MDS_CEILING;

pub(super) const SPECTRAL_3D: Metadata = Metadata {
    tier: 2,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "SciGraphs networkx_layouts._spectral_component_coordinates on scipy 1.16.2 at \
dims=3 (the reference's own _spectral_layout_3d:249-269), compared per connected component as \
the largest principal angle between the two 3-column spans over the 1000 gate seeds \
(harness/oracle-spectral.py); tolerance, not bytes, because the two solvers differ",
    complexity: "O(c^3) per component of c <= 256 nodes (tred2/tql2); above that LOBPCG, \
O(iterations * (m + n b^2)) with block b = 5 and at most 1500 iterations",
    scale_ceiling: SPECTRAL_3D_CEILING,
    degradation: "past the ceiling a path-like component (spectral gap O(1/n^2)) may not \
converge in 1500 iterations; it then fails the residual gate and is skipped, its nodes left at \
the origin and its packing cell, and the run is refused with StageError::Param if no component \
solved — never a random layout (C12). Grid-like and gate-model components converged to 100 000 \
nodes",
    ponytail: "Ponytail (solver): the block start uses a Weyl sequence and a constant-diagonal \
preconditioner, so LOBPCG converges slower than the reference's random start; failing input: a \
single path or cycle of 800 nodes or more. Direction: under-reporting is impossible (the \
residual and orthonormality gate decides, not the iteration count) and the failure is a skipped \
component, not a wrong one. Ponytail (sign): inside a degenerate eigenspace the chosen sign and \
rotation are an artifact of the solver, so a symmetric graph may be mirrored relative to the \
reference; cosmetic. Ponytail (n < 4): the reference returns a RANDOM layout for fewer than \
four nodes (:257-258) and for a graph no component solved (:262-263); this arm refuses instead \
per C12, and a 3-node component therefore SOLVES here (dims_eff = 2, third column filled by \
scatter's copy rule) where the reference would have scattered it at random — a deliberate, \
recorded divergence. Ponytail (scale_ceiling): measured on the spectrum above",
};

pub(super) const PIVOT_MDS_3D: Metadata = Metadata {
    tier: 2,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "SciGraphs networkx_layouts._pivot_mds_component_coordinates on scipy 1.16.2 and \
numpy 2.3.3 at dims=3 with _MDS_PIVOTS=100 (the reference's own _mds_layout_3d:271-291), \
compared per connected component as the largest coordinate difference after peak normalisation \
over the 1000 gate seeds (harness/oracle-spectral.py); tolerance for f32 output and BLAS \
reduction order",
    complexity: "O(k (n + m)) time and O(n k) memory, k = min(100, n); the k x k eigenproblem \
is dense",
    scale_ceiling: PIVOT_MDS_3D_CEILING,
    degradation: "past the measured ceiling nothing changes in kind: time and the n x k \
distance matrix grow linearly and wasm32 traps when it cannot allocate; unreachable pivots \
(other components) are zeroed, which distorts geometry rather than failing, and a component \
whose k x k solve fails the residual gate is skipped as in spectral",
    ponytail: "Ponytail (pivots): farthest-point selection starts at index 0 and breaks ties by \
the lowest index, so the pivot set is a heuristic covering, not an optimum; failing input: a \
graph whose farthest node is one of many equally far, which changes the pivots and so the \
picture, not its validity. Ponytail (distance): unreachable pairs count as 0 hops, which pulls \
nodes of different components together inside a block. Ponytail (n < 4): as in spectral, the \
reference's random fallback (:283-284) is not ported and this refuses. Ponytail \
(scale_ceiling): the ceiling is the largest size measured, not a limit found",
};
