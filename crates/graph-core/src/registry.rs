//! The layout registry (`prompt.md` §8): each layout's capability id, its implementation
//! and the metadata the ledger publishes for it. Every [`Metadata`] field is required and
//! none is an `Option`, so a layout cannot be registered without declaring its tier,
//! stage, geometry, oracle, complexity, `scale_ceiling`, `degradation` and `ponytail`.
//! graph-cli's `capabilities` ledger takes its layout rows from [`LAYOUTS`], and its
//! `hashgate` hashes every layout listed here.

use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::grid::Grid;
use crate::layout::{circle_packing, circular, spectral_stage, tidy_tree, treemap};
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// What the ledger says about a layout. Every field is required.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Metadata {
    /// Delivery tier.
    pub tier: u8,
    /// Pipeline stage.
    pub stage: &'static str,
    /// The node geometry kind it emits.
    pub nodes: NodeGeometryKind,
    /// The edge geometry kind it emits.
    pub edges: EdgeGeometryKind,
    /// The reference it is checked against.
    pub oracle: &'static str,
    /// Time complexity, stated and held.
    pub complexity: &'static str,
    /// Node count past which it stops being usable.
    pub scale_ceiling: u64,
    /// What happens past the ceiling.
    pub degradation: &'static str,
    /// Its Ponytail marker, or the reason none is owed.
    pub ponytail: &'static str,
}

/// One registered layout.
#[derive(Debug, Clone, Copy)]
pub struct Capability {
    /// Its capability id, which is also its hash-gate stage.
    pub id: &'static str,
    /// The layout at its default parameters: the run a hashed snapshot is pinned to.
    pub run: fn(&Topology) -> Result<Geometry, StageError>,
    /// Its ledger metadata.
    pub meta: Metadata,
}

/// Node count past which `layout.grid` stops being usable, and why it is this one.
///
/// Estimated, not measured on the target (`crates/graph-core/tests/memory.rs`,
/// `grid_pipeline_memory_per_node`): natively, the topology stage, the grid and the
/// snapshot's bytes peak at **919 B per node** at 100 000 synthetic nodes and 154 978
/// edges (91.9 MB), input records excluded. wasm32 addresses at most 4 GiB, so
/// 4 GiB / 919 B = 4.67 M nodes, rounded down to two figures. The grid's own `u32` limits bind far later: its lattice is exact
/// up to 2^32 − 1 nodes, and the id tables refuse past 2^32 − 1 bytes of text.
pub const GRID_CEILING: u64 = 4_600_000;

const GRID: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "hand: the conventions worked by hand in graph-core layout/grid.rs, restated in f64 \
and checked per seed by graph-cli roundtrip; no third-party grid is a meaningful oracle \
(SciGraphs' _grid_layout fits its scale and starts at the origin)",
    complexity: "O(n)",
    scale_ceiling: GRID_CEILING,
    degradation: "past the ceiling wasm32 cannot allocate and the module traps (no partial \
result); natively, memory permitting, the snapshot refuses with SnapshotError::Capacity once an \
id table's text would pass 2^32-1 bytes — a refusal, never a wrap or a truncation",
    ponytail: "Ponytail (aspect): cols = ceil(sqrt(n)) is a convention, not a computation; when \
cols does not divide n the last row is ragged and the nodes' centroid sits off the origin (n = 3: \
(-1/6, -1/6)). Direction: cosmetic, never wrong — every node gets its own cell. Escape hatch: a \
layout that centres the last row, under its own id. Ponytail (scale_ceiling): an estimate — \
measured natively on 64-bit and projected onto wasm32's 4 GiB; re-measure with \
crates/graph-core/tests/memory.rs",
};

/// Node count past which the three hierarchy layouts (tidy tree, treemap, circular) stop
/// being usable, and why it is this one.
///
/// Measured, not estimated (`crates/graph-core/tests/memory.rs`,
/// `hierarchy_layout_pipeline_memory_per_node`): each holds within a few percent of the
/// grid's own 919 B/node at 100 000 synthetic nodes and 154 978 edges — tidy tree 933 B,
/// treemap 930 B, circular 922 B/node — because all three add only an O(n) hierarchy
/// repair (`layout/hierarchy.rs`) and O(n) geometry over the same topology and snapshot
/// substrate the grid does. wasm32 addresses at most 4 GiB, so 4 GiB / 933 B = 4.60 M
/// nodes at the heaviest of the three (tidy tree), rounded down to two figures, same as
/// `GRID_CEILING`. The hierarchy repair's own `u32` limit binds far later:
/// `Hierarchy::of` needs `n + 1` rows to fit `u32`, i.e. up to 2^32 − 2 nodes.
pub const HIERARCHY_LAYOUT_CEILING: u64 = 4_600_000;

const TIDY_TREE: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Polyline,
    oracle: "d3-hierarchy@3.1.2 tree() — an exact f64 port of tree.js's Buchheim/Jünger/ \
Leipert/Walker algorithm at d3's own defaults (size([1,1]), the default separation), stated in \
the module doc; byte-compared after Math.fround by harness/oracle-layouts.mjs over >=1000 seeds",
    complexity: "O(n)",
    scale_ceiling: HIERARCHY_LAYOUT_CEILING,
    degradation: "past the ceiling wasm32 cannot allocate and the module traps (no partial \
result); natively, memory permitting, the snapshot refuses with SnapshotError::Capacity once an \
id table's text would pass 2^32-1 bytes — a refusal, never a wrap or a truncation",
    ponytail: "No Ponytail on the algorithm: the port is exact, nothing here is a heuristic, an \
estimate or a fallback, so none is owed (module doc). Ponytail (scale_ceiling): measured, not \
estimated — see HIERARCHY_LAYOUT_CEILING's derivation and \
crates/graph-core/tests/memory.rs::hierarchy_layout_pipeline_memory_per_node.",
};

const TREEMAP: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Box,
    edges: EdgeGeometryKind::Line,
    oracle: "d3-hierarchy@3.1.2 treemap().tile(treemapSquarify) — an exact f64 port of \
squarify.js and hierarchy.sum at d3's own defaults (size([1,1]), no padding, no rounding), \
stated in the module doc; byte-compared after Math.fround by harness/oracle-layouts.mjs over \
>=1000 seeds",
    complexity: "O(n log n)",
    scale_ceiling: HIERARCHY_LAYOUT_CEILING,
    degradation: "past the ceiling wasm32 cannot allocate and the module traps (no partial \
result); natively, memory permitting, the snapshot refuses with SnapshotError::Capacity once an \
id table's text would pass 2^32-1 bytes — a refusal, never a wrap or a truncation",
    ponytail: "a non-positive or non-finite weight clamps to WEIGHT_EPSILON (1e-6) rather than \
vanishing or handing squarify a zero/NaN value — the oracle applies the identical clamp. \
Direction: cosmetic under-representation, a hairline never a wrong containment; escape hatch: \
fix the weight upstream (module doc). Ponytail (scale_ceiling): measured, not estimated — see \
HIERARCHY_LAYOUT_CEILING's derivation and \
crates/graph-core/tests/memory.rs::hierarchy_layout_pipeline_memory_per_node.",
};

const CIRCULAR: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "hand: the ring/angle/radius conventions worked by hand in graph-core \
layout/circular.rs (docs/decisions/circular-conventions.md), restated independently in f64 and \
checked per seed by graph-cli roundtrip; no third-party circular/radial hierarchy layout is a \
meaningful byte-for-byte oracle (SciGraphs' own hierarchical.py normalises differently, per that \
decision doc)",
    complexity: "O(n)",
    scale_ceiling: HIERARCHY_LAYOUT_CEILING,
    degradation: "past the ceiling wasm32 cannot allocate and the module traps (no partial \
result); natively, memory permitting, the snapshot refuses with SnapshotError::Capacity once an \
id table's text would pass 2^32-1 bytes — a refusal, never a wrap or a truncation",
    ponytail: "the radius step and the start angle are conventions pinned by this module, not a \
computation with one right answer (docs/decisions/circular-conventions.md). Failing input: a \
ring holding many nodes at a small radius (a shallow, bushy tree) crowds them close together. \
Direction: cosmetic, never wrong — every node keeps its own ring and a distinct slot, so no two \
real nodes ever collide. Escape hatch: a variant that inflates the radius by ring population, \
under its own id (module doc). Ponytail (scale_ceiling): measured, not estimated — see \
HIERARCHY_LAYOUT_CEILING's derivation and \
crates/graph-core/tests/memory.rs::hierarchy_layout_pipeline_memory_per_node.",
};

/// Node count past which `layout.packing.circle` stops being usable, and why it is this
/// one — a different shape of ceiling than the other three, and much lower.
///
/// Labelled, not a hard memory wall: the exact Collins–Stephenson path (genuinely planar
/// input) is close to linear in `n`, like the other three layouts. But a random or dense
/// graph at synthetic-model density is essentially always non-planar (the planar bound is
/// `m <= 3n - 6`), so the realistic case takes `circle_packing/fallback.rs`'s two O(n^2)
/// relaxation passes. Measured natively, `--release`
/// (`crates/graph-core/tests/memory.rs::circle_packing_pipeline_memory_per_node`): one
/// pipeline call takes 304 ms at n=300, 2.81 s at n=1000, 22.63 s at n=3000 — the O(n^2)
/// shape shows in the timing and in peak memory, which does *not* hold flat per node the
/// way the other three layouts' does (3.2 KB/node at n=300 rising to 24.9 KB/node at
/// n=3000). Fitting that quadratic, a single call already crosses a 1-second budget
/// around n=600. 5,000 is a round, stated cutoff at which a fallback packing already
/// costs tens of seconds even natively; it is not measured directly at that size because
/// doing so is itself impractically slow — the same reason this ceiling exists.
pub const PACKING_CEILING: u64 = 5_000;

const PACKING: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Circle,
    edges: EdgeGeometryKind::Line,
    oracle: "hand + planarity certificate: the Collins-Stephenson exact path is checked against \
its own Euler-formula certificate (planarity::planar_embedding + triangulate_embedding); the \
per-seed hand oracle (graph-cli roundtrip) restates finite radii, no NaN/Inf, and edge tangency \
within tolerance whenever note code 3 is absent — no third-party packer is pinned to this exact \
convention",
    complexity: "O(n) exact path; O(n^2) per relaxation round on the non-planar fallback",
    scale_ceiling: PACKING_CEILING,
    degradation: "past the ceiling the fallback still runs and still returns finite geometry, \
never a refusal or a trap — it simply gets slower at O(n^2), with no built-in cutoff, so a \
caller must apply its own timeout; the exact planar path is unaffected and stays fast at any n \
this crate's u32 index space allows",
    ponytail: "the packing is exact only for planar input. The failing input is any graph with a \
K5 or K3,3 minor (or one whose planar embedding cannot be triangulated into a genuine disk, \
treated the same defensively). Direction: overlap, the dangerous one — the fallback does not \
guarantee tangency or non-overlap either. Escape hatch: read note code 3 off the snapshot; its \
absence is the only trustworthy sign the packing is exact (module doc; full account in \
docs/decisions/planarity-fallback.md). Ponytail (scale_ceiling): labelled, time-bound, not \
measured at the ceiling itself — see PACKING_CEILING's derivation.",
};

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

const SPECTRAL: Metadata = Metadata {
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

const PIVOT_MDS: Metadata = Metadata {
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

/// Every registered layout, in the order the hash gate runs them.
pub static LAYOUTS: [Capability; 7] = [
    Capability {
        id: Grid::ID,
        run: run_default::<Grid>,
        meta: GRID,
    },
    Capability {
        id: "layout.tree.tidy",
        run: tidy_tree::run,
        meta: TIDY_TREE,
    },
    Capability {
        id: "layout.treemap.squarified",
        run: treemap::run,
        meta: TREEMAP,
    },
    Capability {
        id: "layout.circular.radial",
        run: circular::run,
        meta: CIRCULAR,
    },
    Capability {
        id: "layout.packing.circle",
        run: circle_packing::run,
        meta: PACKING,
    },
    Capability {
        id: "layout.spectral",
        run: spectral_stage::spectral,
        meta: SPECTRAL,
    },
    Capability {
        id: "layout.mds.pivot",
        run: spectral_stage::pivot_mds,
        meta: PIVOT_MDS,
    },
];

/// The layout registered under `id`.
pub fn find(id: &str) -> Option<&'static Capability> {
    LAYOUTS.iter().find(|layout| layout.id == id)
}

fn run_default<S: Stage>(topology: &Topology) -> Result<Geometry, StageError> {
    S::run(topology, &S::Params::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::grid::GridParams;
    use crate::stage::{gate_node_count, run_with, seeded_model};
    use crate::weights::REFERENCE_DEGREE;

    #[test]
    fn every_layout_is_a_layout_stage_with_its_metadata_filled() {
        let mut ids: Vec<_> = LAYOUTS.iter().map(|layout| layout.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), LAYOUTS.len(), "unique ids");
        for layout in &LAYOUTS {
            let m = layout.meta;
            assert!(layout.id.starts_with("layout."), "{}", layout.id);
            assert_eq!(m.stage, "layout");
            assert!(m.scale_ceiling > 0, "{}", layout.id);
            for text in [m.oracle, m.complexity, m.degradation, m.ponytail] {
                assert!(!text.trim().is_empty(), "{}", layout.id);
            }
        }
    }

    #[test]
    fn a_registered_layout_emits_the_kinds_it_declares_at_its_default_parameters() {
        let (nodes, edges) = seeded_model(5, gate_node_count(5), REFERENCE_DEGREE);
        for layout in &LAYOUTS {
            let run = run_with(&nodes, &edges, layout.id, layout.run).expect("runs");
            let header = run.snapshot.header();
            assert_eq!(
                (header.node_kind, header.edge_kind),
                (layout.meta.nodes, layout.meta.edges)
            );
        }
        let grid = find("layout.grid").expect("registered");
        let by_hand = run_with(&nodes, &edges, "layout.grid", |t| {
            Grid::run(t, &GridParams::default())
        });
        assert_eq!(run_with(&nodes, &edges, grid.id, grid.run), by_hand);
        assert!(find("layout.none").is_none());
    }
}
