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
    BarnesHut, DavidsonHarel, Drl, FruchtermanReingold, Graphopt, KamadaKawai, Lgl, YifanHu,
};
use crate::layout::forceatlas2::ForceAtlas2;
use crate::layout::graphviz::circo;
use crate::layout::graphviz::fdp;
use crate::layout::graphviz::neato;
use crate::layout::graphviz::osage;
use crate::layout::graphviz::patchwork;
use crate::layout::grid::Grid;
use crate::layout::hierarchical_3d;
use crate::layout::radial::twopi;
use crate::layout::sugiyama::Sugiyama;
use crate::layout::{
    bipartite, circle_packing, circular, random, spectral_stage, spiral, tidy_tree, treemap,
};
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

mod closed_form;
mod force;
mod graphviz_circo;
mod graphviz_fdp;
mod graphviz_neato;
mod graphviz_osage;
mod graphviz_patchwork;
mod grid;
mod hierarchy;
mod igraph;
mod radial;
mod spectral;
mod three_d;
use closed_form::{BIPARTITE, RANDOM, RING, SPIRAL};
use force::{BARNES_HUT, FA2, SPRING, YIFAN_HU};
pub use force::{FA2_CEILING, FORCE_CEILING, SPRING_CEILING};
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
use grid::{GRID, PACKING, SUGIYAMA};
pub use grid::{GRID_CEILING, PACKING_CEILING, SUGIYAMA_CEILING};
pub use hierarchy::HIERARCHY_LAYOUT_CEILING;
use hierarchy::{CIRCULAR, CIRCULAR_HIERARCHY, TIDY_TREE, TREEMAP};
pub use radial::RADIAL_CEILING;
use radial::TWOPI;
use spectral::{PIVOT_MDS, SPECTRAL};
pub use spectral::{PIVOT_MDS_CEILING, SPECTRAL_CEILING};
pub use three_d::BASIC_3D_CEILING;
use three_d::{CUBE, HELIX, HIERARCHICAL_3D, SPHERE, SPRING_3D};

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

/// Every registered layout, in the order the hash gate runs them.
pub static LAYOUTS: [Capability; 34] = [
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
    Capability {
        id: FruchtermanReingold::ID,
        run: run_default::<FruchtermanReingold>,
        meta: igraph::FRUCHTERMAN_REINGOLD,
    },
    Capability {
        id: KamadaKawai::ID,
        run: run_default::<KamadaKawai>,
        meta: igraph::KAMADA_KAWAI,
    },
    Capability {
        id: Graphopt::ID,
        run: run_default::<Graphopt>,
        meta: igraph::GRAPHOPT,
    },
    Capability {
        id: DavidsonHarel::ID,
        run: run_default::<DavidsonHarel>,
        meta: igraph::DAVIDSON_HAREL,
    },
    Capability {
        id: Lgl::ID,
        run: run_default::<Lgl>,
        meta: igraph::LGL,
    },
    Capability {
        id: Drl::ID,
        run: run_default::<Drl>,
        meta: igraph::DRL,
    },
    Capability {
        id: twopi::ID,
        run: twopi::run,
        meta: TWOPI,
    },
    Capability {
        id: osage::ID,
        run: osage::run,
        meta: OSAGE,
    },
    Capability {
        id: Spring::ID,
        run: run_default::<Spring>,
        meta: SPRING,
    },
    Capability {
        id: circular::hierarchy::ID,
        run: circular::hierarchy::run,
        meta: CIRCULAR_HIERARCHY,
    },
    Capability {
        id: circo::ID,
        run: circo::run,
        meta: CIRCO,
    },
    Capability {
        id: patchwork::ID,
        run: patchwork::run,
        meta: PATCHWORK,
    },
    Capability {
        id: neato::ID,
        run: neato::run,
        meta: NEATO,
    },
    Capability {
        id: fdp::ID,
        run: fdp::run,
        meta: FDP,
    },
    // ---- p12-t3, the last five SciGraphs layouts, all natively 3D. APPENDED, never
    // inserted: `graph-wasm/src/exports/build.rs:23,32,166` maps layouts by INDEX, and
    // `bench/campaign.rs:128`'s `DEFAULT_ARM` is `LAYOUTS[3]`, so inserting before index 3
    // would repoint the default crossover arm with no compile error. Nothing above this
    // line moved.
    Capability {
        id: basic_3d::sphere::ID,
        run: basic_3d::sphere,
        meta: SPHERE,
    },
    Capability {
        id: basic_3d::helix::ID,
        run: basic_3d::helix,
        meta: HELIX,
    },
    Capability {
        id: basic_3d::cube::ID,
        run: basic_3d::cube,
        meta: CUBE,
    },
    Capability {
        id: hierarchical_3d::ID,
        run: hierarchical_3d::run,
        meta: HIERARCHICAL_3D,
    },
    Capability {
        id: SPRING_3D_ID,
        run: run_default::<Spring3D>,
        meta: SPRING_3D,
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
