//! The two types every registry entry is made of, and the two functions over them, split out
//! of `registry.rs` for the house 300-line limit.

use crate::index::Topology;
use crate::layout::Geometry;
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

/// The layout registered under `id`.
pub fn find(id: &str) -> Option<&'static Capability> {
    super::LAYOUTS.iter().find(|layout| layout.id == id)
}

/// A registry entry's `run`: the stage at its default parameters.
pub(super) fn run_default<S: Stage>(topology: &Topology) -> Result<Geometry, StageError> {
    S::run(topology, &S::Params::default())
}
