//! The layout registry (`prompt.md` §8): each layout's capability id, its implementation
//! and the metadata the ledger publishes for it. Every [`Metadata`] field is required and
//! none is an `Option`, so a layout cannot be registered without declaring its tier,
//! stage, geometry, oracle, complexity, `scale_ceiling`, `degradation` and `ponytail`.
//! graph-cli's `capabilities` ledger takes its layout rows from [`LAYOUTS`], and its
//! `hashgate` hashes every layout listed here.

use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::basic_3d;
use crate::layout::force::spring::Spring;
use crate::layout::force::spring::{ID_3D as SPRING_3D_ID, Spring3D};
use crate::layout::force::{
    BarnesHut, DavidsonHarel, Drl, FruchtermanReingold, Graphopt, KamadaKawai, Lgl, ParticleMesh,
    YifanHu,
};
use crate::layout::forceatlas2::{ForceAtlas2, ForceAtlas2BarnesHut};
use crate::layout::graphviz::circo;
use crate::layout::graphviz::fdp;
use crate::layout::graphviz::neato;
use crate::layout::graphviz::osage;
use crate::layout::graphviz::patchwork;
use crate::layout::graphviz::sfdp;
use crate::layout::grid::Grid;
use crate::layout::hierarchical_3d;
use crate::layout::radial::twopi;
use crate::layout::sugiyama::Sugiyama;
use crate::layout::{
    bipartite, circle_packing, circular, random, spectral_stage, spiral, tidy_tree, treemap,
};
use crate::stage::{Stage, StageError};

mod arms_3d;
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
mod radial;
mod spectral;
mod three_d;
use arms_3d::{DRL_3D, FA2_3D, FRUCHTERMAN_REINGOLD_3D, KAMADA_KAWAI_3D, YIFAN_HU_2Z};
pub use capability::{Capability, Metadata};
use closed_form::{BIPARTITE, RANDOM, RING, SPIRAL};
use force::{BARNES_HUT, FA2, PARTICLE_MESH, SPRING, YIFAN_HU};
pub use force::{FA2_CEILING, FORCE_CEILING, SPRING_CEILING};
use forceatlas2_bh::FA2_BH;
pub use forceatlas2_bh::FA2_BH_CEILING;
use graphviz_circo::CIRCO;
pub use graphviz_circo::GRAPHVIZ_CIRCO_CEILING;
use graphviz_fdp::FDP;
pub use graphviz_fdp::FDP_CEILING;
use graphviz_neato::NEATO;
pub use graphviz_neato::NEATO_CEILING;
use graphviz_osage::OSAGE;
pub use graphviz_osage::OSAGE_CEILING;
use graphviz_patchwork::PATCHWORK;
pub use graphviz_patchwork::PATCHWORK_CEILING;
use graphviz_sfdp::SFDP;
pub use graphviz_sfdp::SFDP_CEILING;
use grid::{GRID, PACKING, SUGIYAMA};
pub use grid::{GRID_CEILING, PACKING_CEILING, SUGIYAMA_CEILING};
pub use hierarchy::HIERARCHY_LAYOUT_CEILING;
use hierarchy::{CIRCULAR, CIRCULAR_HIERARCHY, TIDY_TREE, TREEMAP};
pub use radial::RADIAL_CEILING;
use radial::TWOPI;
use spectral::{PIVOT_MDS, SPECTRAL};
pub use spectral::{PIVOT_MDS_CEILING, SPECTRAL_CEILING};
pub use three_d::BASIC_3D_CEILING;
use three_d::{BIPARTITE_3D, CUBE, HELIX, HIERARCHICAL_3D, SPHERE, SPIRAL_3D, SPRING_3D};

mod layouts;
pub use layouts::LAYOUTS;

/// The layout registered under `id`.
pub fn find(id: &str) -> Option<&'static Capability> {
    LAYOUTS.iter().find(|layout| layout.id == id)
}

fn run_default<S: Stage>(topology: &Topology) -> Result<Geometry, StageError> {
    S::run(topology, &S::Params::default())
}

#[cfg(test)]
mod tests;
