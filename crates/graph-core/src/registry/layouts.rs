//! The layout table itself, split out of `registry.rs` for the house's 300-line limit.
//!
//! **A pure move.** Every `Capability` row, its order and its `run` pointer are unchanged;
//! only the file they are written in. The order is load-bearing and is documented where
//! the rows are: `graph-wasm/src/exports/build.rs:23,32,166` maps layouts by INDEX, and
//! `bench/campaign.rs:128` pins `LAYOUTS[3]`, so an inserted row repoints an index-keyed
//! consumer with no compile error. Nothing here may be reordered.

use super::*;

/// Every registered layout, in the order the hash gate runs them.
pub static LAYOUTS: [Capability; 44] = [
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
        id: crate::layout::force::yifan_hu::ID_2Z,
        run: arms_3d::run_yifan_2z,
        meta: YIFAN_HU_2Z,
    },
    Capability {
        id: FruchtermanReingold::ID,
        run: run_default::<FruchtermanReingold>,
        meta: igraph::FRUCHTERMAN_REINGOLD,
    },
    Capability {
        id: crate::layout::force::fruchterman_reingold::ID_3D,
        run: arms_3d::run_fr_3d,
        meta: FRUCHTERMAN_REINGOLD_3D,
    },
    Capability {
        id: KamadaKawai::ID,
        run: run_default::<KamadaKawai>,
        meta: igraph::KAMADA_KAWAI,
    },
    Capability {
        id: crate::layout::force::kamada_kawai::ID_3D,
        run: arms_3d::run_kk_3d,
        meta: KAMADA_KAWAI_3D,
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
        id: crate::layout::force::drl::ID_3D,
        run: arms_3d::run_drl_3d,
        meta: DRL_3D,
    },
    Capability {
        id: crate::layout::forceatlas2::ID_3D,
        run: arms_3d::run_fa2_3d,
        meta: FA2_3D,
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
    // would repoint the default crossover arm with no compile error. p12-t4b's five 3D arms
    // are the one exception, and they are all after index 3: each sits beside the 2D layout
    // it arms, so the pairs stay readable in `hashgate`'s order.
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
    Capability {
        id: sfdp::ID,
        run: sfdp::run,
        meta: SFDP,
    },
    Capability {
        id: ForceAtlas2BarnesHut::ID,
        run: run_default::<ForceAtlas2BarnesHut>,
        meta: FA2_BH,
    },
    // APPENDED, never inserted, for the reason the block above gives: layouts are mapped by
    // INDEX in `graph-wasm/src/exports/build.rs:23,32,166` and `bench/campaign.rs:128` pins
    // `LAYOUTS[3]`. `layout.bipartite_3d` reads the graph where the three above it read a
    // node count, which is why its id is outside the `layout.basic3d.*` namespace those
    // three publish.
    Capability {
        id: basic_3d::bipartite_3d::ID,
        run: basic_3d::bipartite_3d,
        meta: BIPARTITE_3D,
    },
    // ---- sg-spiral3d: SciGraphs' SPIRAL_3D, the conical 3D spiral of `basic.py:36-63`.
    // APPENDED for the same reason as the block above it: inserting would repoint every
    // index-keyed consumer with no compile error.
    Capability {
        id: basic_3d::spiral::ID,
        run: basic_3d::spiral,
        meta: SPIRAL_3D,
    },
    // perf-p2: appended after the entries above, for the same reason.
    Capability {
        id: ParticleMesh::ID,
        run: run_default::<ParticleMesh>,
        meta: PARTICLE_MESH,
    },
];
