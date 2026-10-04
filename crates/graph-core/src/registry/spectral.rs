//! Ledger metadata for the Phase 6 spectral family (`layout.spectral`, `layout.mds.pivot`,
//! and their 3D siblings `layout.spectral3d`, `layout.mds.pivot3d`), kept apart from
//! `registry.rs` for the house line cap. The two 3D siblings' whole entries are here too,
//! because `registry/layouts.rs` is at the 300-line cap.

use super::{Capability, LayoutParams, Metadata};
use crate::layout::spectral_stage;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// **The node ceiling, and what it is a ceiling *of*.** `700` is the largest **path-like**
/// component — the worst spectral-gap shape there is, `O(1/n^2)` — that the iterative tier
/// was measured to place with LOBPCG alone. It is *not* a node limit on the layout: a 100x100
/// grid (10 000 nodes) and the gate model at 100 000 nodes both converge, and 100 000 is
/// four orders of magnitude above this number.
///
/// **What the ceiling bounds, precisely.** Since LF-09 and LF-10, `layout.spectral` no longer
/// skips a component it cannot place — `spectral_stage` refuses the run — so "past the
/// ceiling" is a *refusal*, not a degradation, and it refuses exactly when the **per-component
/// spectral gap** is too tight for LOBPCG's 1500 iterations. That gap threshold is not a node
/// count and is not this constant: it is measured per component, by the residual and
/// orthonormality gate in `layout::spectral::solve`, and the two happen to agree on 700 only
/// for a path. `shift_invert::DENSE_INVERT_LIMIT` (1024) is a third, separate number and a
/// budget rather than a threshold — see that constant's Ponytail line.
///
/// Measured, release build (`docs/measurements/phase06-eigen.md`, and re-measured for
/// `docs/measurements/fix-spectral.md` on the shift-invert cascade): a 700-node path solved
/// in 131 ms on 747 LOBPCG iterations; an 800-node path exhausted `maxiter = 1500`, as did
/// 900 to 4096, and **now solves through the shift-invert retry** (peak residual `7.3e-11` at
/// 800, `1.9e-10` at 1024) — the retry's own budget, `DENSE_INVERT_LIMIT = 1024`, is what
/// stops it, one node later. A 100x100 grid solves in 1.3 s and the gate model at 100 000
/// nodes in 6.9 s.
pub const SPECTRAL_CEILING: u64 = 700;

/// **The node ceiling, and what it is a ceiling *of*.** The largest size at which
/// `layout.mds.pivot` was measured — nothing here is a threshold at all. Unlike
/// [`SPECTRAL_CEILING`] no shape of this layout fails at any size: the solve is a dense
/// `k x k` eigensolve with `k = min(100, n_c)`, so it is `n`, not the spectrum, that runs out.
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
O(iterations * (m + n b^2)) with block b = 4 and at most 1500 iterations; when that misses the \
gate and c <= 1024, one dense Cholesky of (L + 1e-3 I) at O(c^3/3) plus LOBPCG on its inverse \
at O(iterations * c^2)",
    scale_ceiling: SPECTRAL_CEILING,
    degradation: "a component whose spectral gap is too tight for LOBPCG's 1500 iterations \
gets the reference's shift-invert retry, which resolves the same matrices to a peak residual \
of 1e-10 or better; past the retry's own dense budget of 1024 nodes the component still misses \
the gate, and the run is then REFUSED with StageError::Param naming it (spectral_stage no \
longer skips a component: its nodes used to collapse onto one point at their packing cell with \
the residual printed nowhere). Never a random layout, and never a partial picture (C12). \
Grid-like and gate-model components converged to 100 000 nodes",
    ponytail: "Ponytail (solver): the block start uses a Weyl sequence and a constant-diagonal \
preconditioner, so LOBPCG converges slower than the reference's random start; failing input: a \
single path or cycle whose gap O(1/n^2) no iteration count reaches. Direction: under-reporting \
is impossible (the residual and orthonormality gate decides, not the iteration count) and the \
failure is a refusal, not a wrong picture. Ponytail (shift_invert budget): the retry \
factorises densely and stops at 1024 nodes, so a path of 1025 or more is refused where 800 is \
placed; direction is under-reporting only. Ponytail (sign): inside a degenerate eigenspace the \
chosen sign and rotation are an artifact of the solver, so a symmetric graph may be mirrored \
relative to the reference; cosmetic. Ponytail (scale_ceiling): a per-shape spectral-gap \
threshold measured on the worst spectrum, not a node limit; see SPECTRAL_CEILING.",
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
whose k x k solve misses the residual or orthonormality gate is REFUSED as in spectral, with \
StageError::Param naming it",
    ponytail: "Ponytail (pivots): farthest-point selection starts at index 0 and breaks ties by \
the lowest index, so the pivot set is a heuristic covering, not an optimum; failing input: a \
graph whose farthest node is one of many equally far, which changes the pivots and so the \
picture, not its validity. Ponytail (distance): unreachable pairs count as 0 hops, which pulls \
nodes of different components together inside a block. Ponytail (tie): inside a tied group of \
Gram eigenvalues the basis is canonicalised before projection (pivot_mds::tied), so the drawing \
no longer carries the solver's rotation; over-grouping two eigenvalues that are merely close \
replaces them by a canonical basis of their span, which is a valid answer to the same \
eigenproblem and not a detectable difference. Ponytail (scale_ceiling): the largest size \
measured, not a limit found.",
};

/// `layout.spectral3d`: `_spectral_layout_3d` (`networkx_layouts.py:249-269`) — the same
/// kernel as [`SPECTRAL`] at the reference's `dims = 3`, its cubic component lattice and its
/// `_rescale_positions`.
///
/// **Same ceilings as the 2D id, and the same reasons.** The eigenproblem is the same matrix
/// with one more eigenpair asked for; the extra lattice axis and the rescale are linear.
pub(super) const SPECTRAL_3D: Metadata = Metadata {
    tier: 2,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "SciGraphs networkx_layouts._spectral_layout_3d on scipy 1.16.2 (dense eigh below \
257 nodes per component, LOBPCG above), compared coordinate by coordinate as the largest \
absolute difference over the conformance fixture set; tolerance, not bytes, because the two \
solvers are JAMA tred2/tql2 and LAPACK syevd",
    complexity: "O(c^3) per component of c <= 256 nodes (tred2/tql2); above that LOBPCG, \
O(iterations * (m + n b^2)) with block b = 5 and at most 1500 iterations; when that misses the \
gate and c <= 1024, one dense Cholesky of (L + 1e-3 I) at O(c^3/3) plus LOBPCG on its inverse \
at O(iterations * c^2)",
    scale_ceiling: SPECTRAL_CEILING,
    degradation: "a component whose spectral gap is too tight for LOBPCG's 1500 iterations \
gets the reference's shift-invert retry, which resolves the same matrices to a peak residual \
of 1e-10 or better; past the retry's own dense budget of 1024 nodes the component still misses \
the gate, and the run is then REFUSED with StageError::Param naming it (spectral_stage no \
longer skips a component: its nodes used to collapse onto one point at their packing cell with \
the residual printed nowhere). Never a random layout, and never a partial picture (C12). Below \
four nodes the whole graph is a random layout instead, which is the reference's own guard \
(_spectral_layout_3d:257-258) and not a fallback: that branch draws layout::random's seeded port",
    ponytail: "Ponytail (solver): the block start uses a Weyl sequence and a constant-diagonal \
preconditioner, so LOBPCG converges slower than the reference's random start; failing input: a \
single path or cycle whose gap O(1/n^2) no iteration count reaches. Direction: under-reporting \
is impossible (the residual and orthonormality gate decides, not the iteration count). \
Ponytail (shift_invert budget): the retry factorises densely and stops at 1024 nodes, so a \
path of 1025 or more is refused where 800 is placed; direction is under-reporting only. \
Ponytail (sign): inside a degenerate eigenspace — a path or a regular grid has lambda2 == \
lambda3 — the chosen basis is an artifact of the solver, so such a fixture may stay apart from \
the reference in all three coordinates at once; cosmetic. Ponytail (scale): scale is the \
dispatcher default 5.0 as a constant, not a parameter (basic_3d.rs:53 states the convention). \
Ponytail (scale_ceiling): a per-shape spectral-gap threshold measured on the worst spectrum, \
not a node limit; see SPECTRAL_CEILING.",
};

/// `layout.mds.pivot3d`: `_mds_layout_3d` (`networkx_layouts.py:271-291`) — [`PIVOT_MDS`] at
/// the reference's `dims = 3`, with the same cubic lattice and rescale.
pub(super) const PIVOT_MDS_3D: Metadata = Metadata {
    tier: 2,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "SciGraphs networkx_layouts._mds_layout_3d on scipy 1.16.2 and numpy 2.3.3, \
compared coordinate by coordinate as the largest absolute difference over the conformance \
fixture set; tolerance for f32 output and BLAS reduction order",
    complexity: "O(k (n + m)) time and O(n k) memory, k = min(100, n); the k x k eigenproblem \
is dense",
    scale_ceiling: PIVOT_MDS_CEILING,
    degradation: "past the measured ceiling nothing changes in kind: time and the n x k \
distance matrix grow linearly and wasm32 traps when it cannot allocate; a component whose k x k \
solve misses the residual or orthonormality gate is REFUSED as in spectral, and a graph below \
four nodes is a random layout by the reference's own guard (_mds_layout_3d:283-284)",
    ponytail: "Ponytail (pivots): farthest-point selection starts at index 0 and breaks ties by \
the lowest index, so the pivot set is a heuristic covering, not an optimum; failing input: a \
graph whose farthest node is one of many equally far, which changes the pivots and so the \
picture, not its validity. Ponytail (tie): inside a tied group of Gram eigenvalues the basis is \
canonicalised before projection (pivot_mds::tied), so a symmetric input no longer differs from \
the reference by a rotation of a degenerate eigenspace; over-grouping two merely-close \
eigenvalues replaces them by a canonical basis of their span, a valid answer to the same \
eigenproblem. Ponytail (scale): scale is the dispatcher default 5.0 as a constant. \
Ponytail (scale_ceiling): the largest size measured, not a limit found.",
};

/// The `layout.spectral3d` entry, appended to `LAYOUTS`. Separate ids rather than a `dims`
/// parameter, because the reference's own entries are separate (`_spectral_layout_3d`,
/// `_mds_layout_3d`) and a `layout.spectral` that drew a volume would break the bytes its own
/// conformance row and the differential in `harness/oracle-spectral.py` are pinned on.
pub(super) const SPECTRAL_3D_LAYOUT: Capability = Capability {
    id: "layout.spectral3d",
    run: spectral_stage::spectral_3d,
    params: &LayoutParams::NONE,
    meta: SPECTRAL_3D,
};

/// The `layout.mds.pivot3d` entry, appended to `LAYOUTS` after [`SPECTRAL_3D_LAYOUT`].
pub(super) const PIVOT_MDS_3D_LAYOUT: Capability = Capability {
    id: "layout.mds.pivot3d",
    run: spectral_stage::pivot_mds_3d,
    params: &LayoutParams::NONE,
    meta: PIVOT_MDS_3D,
};
