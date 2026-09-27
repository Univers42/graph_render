//! The layout registry (`prompt.md` §8): each layout's capability id, its implementation
//! and the metadata the ledger publishes for it. Every [`Metadata`] field is required and
//! none is an `Option`, so a layout cannot be registered without declaring its tier,
//! stage, geometry, oracle, complexity, `scale_ceiling`, `degradation` and `ponytail`.
//! graph-cli's `capabilities` ledger takes its layout rows from [`LAYOUTS`], and its
//! `hashgate` hashes every layout listed here.

use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::grid::Grid;
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

/// Every registered layout, in the order the hash gate runs them.
pub static LAYOUTS: [Capability; 1] = [Capability {
    id: Grid::ID,
    run: run_default::<Grid>,
    meta: GRID,
}];

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
