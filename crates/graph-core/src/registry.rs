//! The layout registry (`prompt.md` §8): each layout's capability id, its implementation
//! and the metadata the ledger publishes for it. Every [`Metadata`] field is required and
//! none is an `Option`, so a layout cannot be registered without declaring its tier,
//! stage, geometry, oracle, complexity, `scale_ceiling`, `degradation` and `ponytail`.
//! graph-cli's `capabilities` ledger takes its layout rows from [`LAYOUTS`], and its
//! `hashgate` hashes every layout listed here.

use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::force::{BarnesHut, YifanHu};
use crate::layout::forceatlas2::ForceAtlas2;
use crate::layout::grid::Grid;
use crate::layout::sugiyama::Sugiyama;
use crate::layout::{
    bipartite, circle_packing, circular, random, spectral_stage, spiral, tidy_tree, treemap,
};
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

mod closed_form;
mod force;
mod hierarchy;
mod spectral;
use closed_form::{BIPARTITE, RANDOM, RING, SPIRAL};
use force::{BARNES_HUT, FA2, YIFAN_HU};
pub use force::{FA2_CEILING, FORCE_CEILING};
pub use hierarchy::HIERARCHY_LAYOUT_CEILING;
use hierarchy::{CIRCULAR, TIDY_TREE, TREEMAP};
use spectral::{PIVOT_MDS, SPECTRAL};
pub use spectral::{PIVOT_MDS_CEILING, SPECTRAL_CEILING};

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

/// Layered-vertex count past which `layout.dag.sugiyama` routes no more long arcs: the
/// reference's own `_DUMMY_BUDGET` (`SciGraphs/.../hierarchical.py:7`).
pub const SUGIYAMA_CEILING: u64 = 200_000;

const SUGIYAMA: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Polyline,
    oracle: "dagre-d3-es 7.0.14 crossing counts (harness/oracle-layouts.mjs --dag, margin frozen \
in docs/measurements/phase05-crossings.md) and SciGraphs hierarchical.py; per-seed structural \
invariants (acyclic after FAS, monotone layers, contiguous dummy chains) checked by graph-cli \
roundtrip",
    complexity: "O(n+m) per phase; crossing reduction is a heuristic (median + transpose local \
search), not a minimiser",
    scale_ceiling: SUGIYAMA_CEILING,
    degradation: "past the dummy budget (200000) long arcs are left straight and unrouted and \
each is reported as note 4 dag.dummy_budget_exceeded; above 150000 layered vertices the transpose \
rounds drop to 0, so crossings rise while the drawing stays valid",
    ponytail: "Ponytail (crossing reduction): median + transpose is a local search; a graph \
whose optimal order it cannot reach draws more crossings than optimal — cosmetic, never \
incorrect. Ponytail (dummy budget): an unrouted long arc is a straight line that may pass \
through nodes — visually wrong, the dangerous direction; escape hatch: read note 4 in the \
snapshot. Ponytail (FAS): greedy, not minimum; extra reversed edges (note 5) are cosmetic",
};

/// Every registered layout, in the order the hash gate runs them.
pub static LAYOUTS: [Capability; 15] = [
    Capability {
        id: Grid::ID,
        run: run_default::<Grid>,
        meta: GRID,
    },
    Capability {
        id: tidy_tree::ID,
        run: tidy_tree::run,
        meta: TIDY_TREE,
    },
    Capability {
        id: treemap::ID,
        run: treemap::run,
        meta: TREEMAP,
    },
    Capability {
        id: circular::ID,
        run: circular::run,
        meta: CIRCULAR,
    },
    Capability {
        id: circle_packing::ID,
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
    Capability {
        id: BarnesHut::ID,
        run: run_default::<BarnesHut>,
        meta: BARNES_HUT,
    },
    Capability {
        id: ForceAtlas2::ID,
        run: run_default::<ForceAtlas2>,
        meta: FA2,
    },
    Capability {
        id: Sugiyama::ID,
        run: run_default::<Sugiyama>,
        meta: SUGIYAMA,
    },
    Capability {
        id: random::ID,
        run: random::run,
        meta: RANDOM,
    },
    Capability {
        id: circular::ring::ID,
        run: circular::ring::run,
        meta: RING,
    },
    Capability {
        id: spiral::ID,
        run: spiral::run,
        meta: SPIRAL,
    },
    Capability {
        id: bipartite::ID,
        run: bipartite::run,
        meta: BIPARTITE,
    },
    Capability {
        id: YifanHu::ID,
        run: run_default::<YifanHu>,
        meta: YIFAN_HU,
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
mod tests;
