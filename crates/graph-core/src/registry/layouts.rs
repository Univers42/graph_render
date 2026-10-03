//! Every registered layout, in the order the hash gate runs them. Split out of
//! `registry.rs` by the house 300-line limit, and **append only**: the wasm module maps a
//! layout by INDEX, so an insertion repoints every index-keyed consumer with no compile
//! error. The comments inside the array carry that reason per block.

use super::capability::Capability;
use super::params;
use super::run_default;
use super::{
    closed_form, force, forceatlas2_bh, graphviz_circo, graphviz_fdp, graphviz_neato,
    graphviz_osage, graphviz_patchwork, graphviz_sfdp, grid, hierarchy, igraph, radial, spectral,
    three_d,
};
use crate::layout::basic_3d;
use crate::layout::force::spring::Spring;
use crate::layout::force::spring::{ID_3D as SPRING_3D_ID, Spring3D};
use crate::layout::force::{
    BarnesHut, DavidsonHarel, Drl, FruchtermanReingold, Graphopt, KamadaKawai, Lgl, ParticleMesh,
    YifanHu,
};
use crate::layout::forceatlas2::{ForceAtlas2, ForceAtlas2BarnesHut};
use crate::layout::graphviz::{circo, fdp, neato, osage, patchwork, sfdp};
use crate::layout::grid::Grid;
use crate::layout::hierarchical_3d;
use crate::layout::radial::twopi;
use crate::layout::sugiyama::Sugiyama;
use crate::layout::{
    bipartite, circle_packing, circular, random, spectral_stage, spiral, tidy_tree, treemap,
};
use crate::stage::Stage;

use closed_form::{BIPARTITE, RANDOM, RING, SPIRAL};
use force::{BARNES_HUT, FA2, PARTICLE_MESH, SPRING, YIFAN_HU};
use forceatlas2_bh::FA2_BH;
use graphviz_circo::CIRCO;
use graphviz_fdp::FDP;
use graphviz_neato::NEATO;
use graphviz_osage::OSAGE;
use graphviz_patchwork::PATCHWORK;
use graphviz_sfdp::SFDP;
use grid::{GRID, PACKING, SUGIYAMA};
use hierarchy::{CIRCULAR, CIRCULAR_HIERARCHY, TIDY_TREE, TREEMAP};
use radial::TWOPI;
use spectral::{PIVOT_MDS, SPECTRAL};
use three_d::{BIPARTITE_3D, CUBE, HELIX, HIERARCHICAL_3D, SPHERE, SPIRAL_3D, SPRING_3D};

/// Every registered layout, in the order the hash gate runs them.
pub static LAYOUTS: [Capability; 41] = [
    Capability {
        id: Grid::ID,
        run: run_default::<Grid>,
        params: &params::GRID,
        meta: GRID,
    },
    Capability {
        id: tidy_tree::ID,
        run: tidy_tree::run,
        params: &params::LayoutParams::NONE,
        meta: TIDY_TREE,
    },
    Capability {
        id: treemap::ID,
        run: treemap::run,
        params: &params::LayoutParams::NONE,
        meta: TREEMAP,
    },
    Capability {
        id: circular::ID,
        run: circular::run,
        params: &params::LayoutParams::NONE,
        meta: CIRCULAR,
    },
    Capability {
        id: circle_packing::ID,
        run: circle_packing::run,
        params: &params::PACKING,
        meta: PACKING,
    },
    Capability {
        id: "layout.spectral",
        run: spectral_stage::spectral,
        params: &params::LayoutParams::NONE,
        meta: SPECTRAL,
    },
    Capability {
        id: "layout.mds.pivot",
        run: spectral_stage::pivot_mds,
        params: &params::LayoutParams::NONE,
        meta: PIVOT_MDS,
    },
    Capability {
        id: BarnesHut::ID,
        run: run_default::<BarnesHut>,
        params: &params::LayoutParams::NONE,
        meta: BARNES_HUT,
    },
    Capability {
        id: ForceAtlas2::ID,
        run: run_default::<ForceAtlas2>,
        params: &params::FORCEATLAS2,
        meta: FA2,
    },
    Capability {
        id: Sugiyama::ID,
        run: run_default::<Sugiyama>,
        params: &params::SUGIYAMA,
        meta: SUGIYAMA,
    },
    Capability {
        id: random::ID,
        run: random::run,
        params: &params::LayoutParams::NONE,
        meta: RANDOM,
    },
    Capability {
        id: circular::ring::ID,
        run: circular::ring::run,
        params: &params::LayoutParams::NONE,
        meta: RING,
    },
    Capability {
        id: spiral::ID,
        run: spiral::run,
        params: &params::LayoutParams::NONE,
        meta: SPIRAL,
    },
    Capability {
        id: bipartite::ID,
        run: bipartite::run,
        params: &params::LayoutParams::NONE,
        meta: BIPARTITE,
    },
    Capability {
        id: YifanHu::ID,
        run: run_default::<YifanHu>,
        params: &params::LayoutParams::NONE,
        meta: YIFAN_HU,
    },
    Capability {
        id: FruchtermanReingold::ID,
        run: run_default::<FruchtermanReingold>,
        params: &params::FRUCHTERMAN_REINGOLD,
        meta: igraph::FRUCHTERMAN_REINGOLD,
    },
    Capability {
        id: KamadaKawai::ID,
        run: run_default::<KamadaKawai>,
        params: &params::KAMADA_KAWAI,
        meta: igraph::KAMADA_KAWAI,
    },
    Capability {
        id: Graphopt::ID,
        run: run_default::<Graphopt>,
        params: &params::GRAPHOPT,
        meta: igraph::GRAPHOPT,
    },
    Capability {
        id: DavidsonHarel::ID,
        run: run_default::<DavidsonHarel>,
        params: &params::DAVIDSON_HAREL,
        meta: igraph::DAVIDSON_HAREL,
    },
    Capability {
        id: Lgl::ID,
        run: run_default::<Lgl>,
        params: &params::LGL,
        meta: igraph::LGL,
    },
    Capability {
        id: Drl::ID,
        run: run_default::<Drl>,
        params: &params::DRL,
        meta: igraph::DRL,
    },
    Capability {
        id: twopi::ID,
        run: twopi::run,
        params: &params::LayoutParams::NONE,
        meta: TWOPI,
    },
    Capability {
        id: osage::ID,
        run: osage::run,
        params: &params::LayoutParams::NONE,
        meta: OSAGE,
    },
    Capability {
        id: Spring::ID,
        run: run_default::<Spring>,
        params: &params::SPRING,
        meta: SPRING,
    },
    Capability {
        id: circular::hierarchy::ID,
        run: circular::hierarchy::run,
        params: &params::LayoutParams::NONE,
        meta: CIRCULAR_HIERARCHY,
    },
    Capability {
        id: circo::ID,
        run: circo::run,
        params: &params::LayoutParams::NONE,
        meta: CIRCO,
    },
    Capability {
        id: patchwork::ID,
        run: patchwork::run,
        params: &params::LayoutParams::NONE,
        meta: PATCHWORK,
    },
    Capability {
        id: neato::ID,
        run: neato::run,
        params: &params::LayoutParams::NONE,
        meta: NEATO,
    },
    Capability {
        id: fdp::ID,
        run: fdp::run,
        params: &params::LayoutParams::NONE,
        meta: FDP,
    },
    // ---- p12-t3, the last five SciGraphs layouts, all natively 3D. APPENDED, never
    // inserted: `graph-wasm/src/exports/build.rs:26,35,159` maps layouts by INDEX, and
    // `bench/campaign.rs:128`'s `DEFAULT_ARM` is `LAYOUTS[3]`, so inserting before index 3
    // would repoint the default crossover arm with no compile error. Nothing above this
    // line moved, and
    // `registry::tests::the_index_keyed_front_of_layouts_still_holds_the_ids_their_callers_name`
    // fails if it ever does.
    Capability {
        id: basic_3d::sphere::ID,
        run: basic_3d::sphere,
        params: &params::LayoutParams::NONE,
        meta: SPHERE,
    },
    Capability {
        id: basic_3d::helix::ID,
        run: basic_3d::helix,
        params: &params::LayoutParams::NONE,
        meta: HELIX,
    },
    Capability {
        id: basic_3d::cube::ID,
        run: basic_3d::cube,
        params: &params::LayoutParams::NONE,
        meta: CUBE,
    },
    Capability {
        id: hierarchical_3d::ID,
        run: hierarchical_3d::run,
        params: &params::LayoutParams::NONE,
        meta: HIERARCHICAL_3D,
    },
    Capability {
        id: SPRING_3D_ID,
        run: run_default::<Spring3D>,
        params: &params::SPRING_3D,
        meta: SPRING_3D,
    },
    Capability {
        id: sfdp::ID,
        run: sfdp::run,
        params: &params::LayoutParams::NONE,
        meta: SFDP,
    },
    Capability {
        id: ForceAtlas2BarnesHut::ID,
        run: run_default::<ForceAtlas2BarnesHut>,
        params: &params::FORCEATLAS2_BARNES_HUT,
        meta: FA2_BH,
    },
    // APPENDED, never inserted, for the reason the block above gives: layouts are mapped by
    // INDEX in `graph-wasm/src/exports/build.rs:26,35,159` and `bench/campaign.rs:128` pins
    // `LAYOUTS[3]`. `layout.bipartite_3d` reads the graph where the three above it read a
    // node count, which is why its id is outside the `layout.basic3d.*` namespace those
    // three publish.
    Capability {
        id: basic_3d::bipartite_3d::ID,
        run: basic_3d::bipartite_3d,
        params: &params::LayoutParams::NONE,
        meta: BIPARTITE_3D,
    },
    // ---- sg-spiral3d: SciGraphs' SPIRAL_3D, the conical 3D spiral of `basic.py:36-63`.
    Capability {
        id: basic_3d::spiral::ID,
        run: basic_3d::spiral,
        params: &params::LayoutParams::NONE,
        meta: SPIRAL_3D,
    },
    // perf-p2: appended after the entries above, for the same reason.
    Capability {
        id: ParticleMesh::ID,
        run: run_default::<ParticleMesh>,
        params: &params::LayoutParams::NONE,
        meta: PARTICLE_MESH,
    },
    // ---- sg-spectral-mds: the 3D arms of the spectral family, spelled in `registry/spectral.rs`.
    spectral::SPECTRAL_3D_LAYOUT,
    spectral::PIVOT_MDS_3D_LAYOUT,
];
