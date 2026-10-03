//! The two types every registry entry is made of, split out of `registry.rs` for the house
//! 300-line limit.

use super::LayoutParams;
use crate::index::Topology;
use crate::layout::Geometry;
use crate::stage::StageError;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// What the ledger says about a layout. Every field is required.
///
/// `Eq`, which is why the parameter list is on [`Capability`] and not here: a spec carries
/// `f64` bounds, and a `&'static [ParamSpec]` cannot be `Eq` — a field here would drop
/// `Eq` from every layout row and from the POST styles' factory
/// (`crate::post::styles::ledger::meta`) for no gain.
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
    /// The parameters it publishes, and the run at a buffer's values. Empty for a layout
    /// that takes none (`LayoutParams::NONE`), which is an answer and not a gap.
    pub params: &'static LayoutParams,
    /// Its ledger metadata.
    pub meta: Metadata,
}
