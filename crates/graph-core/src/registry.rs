//! The layout registry (`prompt.md` §8): each layout's capability id, its implementation,
//! the parameters it publishes and the metadata the ledger publishes for it. Every
//! [`Metadata`] field is required and none is an `Option`, so a layout cannot be
//! registered without declaring its tier, stage, geometry, oracle, complexity,
//! `scale_ceiling`, `degradation` and `ponytail`.
//! graph-cli's `capabilities` ledger takes its layout rows from [`LAYOUTS`], and its
//! `hashgate` hashes every layout listed here.
//!
//! What a layout publishes is on [`Capability`], not on [`Metadata`]: a spec carries `f64`
//! bounds, `Metadata` derives `Eq`, and a list of them cannot (`registry/capability.rs`).
//! `docs/decisions/layout-params.md` is the decision, its exclusions and their costs.

use crate::index::Topology;
use crate::layout::Geometry;
// The layout types and metas the array names are imported by `registry/layouts.rs`, which
// holds the array. Only the four module ids `registry/tests.rs`'s index-pinning test
// resolves through its `use super::*` are needed here, and only when it is compiled.
#[cfg(test)]
use crate::layout::{circle_packing, circular, tidy_tree, treemap};
use crate::stage::{Stage, StageError};

mod bench_cap;
mod capability;
mod closed_form;
mod force;
mod forceatlas2_bh;
mod graphviz_circo;
mod graphviz_fdp;
mod graphviz_neato;
mod graphviz_osage;
mod graphviz_patchwork;
mod graphviz_sfdp;
mod grid;
mod hierarchy;
mod igraph;
mod layouts;
mod params;
mod radial;
mod spectral;
mod three_d;
mod tunable;
pub use bench_cap::MAX_BENCH_NODES;
pub use capability::{Capability, Metadata};
pub use closed_form::CLOSED_FORM_CEILING;
pub use force::{FA2_CEILING, FORCE_CEILING, SPRING_CEILING};
pub use forceatlas2_bh::FA2_BH_CEILING;
pub use graphviz_circo::GRAPHVIZ_CIRCO_CEILING;
pub use graphviz_fdp::FDP_CEILING;
pub use graphviz_neato::NEATO_CEILING;
pub use graphviz_osage::OSAGE_CEILING;
pub use graphviz_patchwork::PATCHWORK_CEILING;
pub use graphviz_sfdp::SFDP_CEILING;
pub use grid::{GRID_CEILING, PACKING_CEILING, SUGIYAMA_CEILING};
pub use hierarchy::HIERARCHY_LAYOUT_CEILING;
pub use layouts::LAYOUTS;
pub use params::{LayoutParams, Tunable};
pub use radial::RADIAL_CEILING;
pub use spectral::{PIVOT_MDS_CEILING, SPECTRAL_CEILING};
pub use three_d::BASIC_3D_CEILING;

/// The layout registered under `id`.
pub fn find(id: &str) -> Option<&'static Capability> {
    LAYOUTS.iter().find(|layout| layout.id == id)
}

fn run_default<S: Stage>(topology: &Topology) -> Result<Geometry, StageError> {
    S::run(topology, &S::Params::default())
}

#[cfg(test)]
mod tests;
