//! The 32 rows: `dispatcher.py`'s own dispatch order, each name against the motor layout
//! `docs/measurements/scigraphs-coverage.md` maps it to, with the parameters of
//! `apply_graph_layout` the motor has no slot for.
//!
//! **A gap is recorded here rather than normalised in the fixture, because normalising it is
//! the failure this file exists to prevent.** `scale = 5.0` against graph-core's `SCALE` const
//! is the same number today; the day it is not, the row's cause changes and nobody would be
//! told. Every `at` is the line the fixed value is written on, so a repair job opens the file
//! at the number.
//!
//! **Most of the igraph family has no `iterations` gap, and that is a finding, not an
//! omission.** With `props=None` the dispatcher hands `_igraph_davidson_harel(G, iterations,
//! scale)` and igraph's own `maxiter=10` wins (`igraph_layouts.py:461`), likewise graphopt's
//! `niter=500` (`:493`) and LGL's `maxiter=150` (`:423`). The motor's registered defaults are
//! those same numbers, so the arms agree on the budget without either being told. What they
//! cannot agree on is the seed, which is [`G_IGRAPH_SEED`] — and igraph's is not reachable from
//! Python at all.

use super::{Reference, Row};

use super::gaps::*;

/// The 32 names, in `dispatcher.py`'s dispatch order.
///
/// `GRAPHVIZ_DOT` has no motor layout on this tree (the coverage doc's "planned"), so its
/// motor cells read `not run: no motor layout for this name` rather than a zero — a zero
/// would be a claim measured against nothing.
pub const ROWS: [Row; 32] = [
    Row {
        name: "RANDOM",
        motor: Some("layout.random"),
        reference: Reference::Scigraphs,
        gaps: &[G_RANDOM_ITER, G_RANDOM_SCALE, G_RANDOM_SEED],
    },
    Row {
        name: "GRID",
        motor: Some("layout.grid"),
        reference: Reference::Scigraphs,
        gaps: &[G_GRID_ITER, G_GRID_SCALE],
    },
    Row {
        name: "SPRING",
        motor: Some("layout.force.spring"),
        reference: Reference::Scigraphs,
        gaps: &[G_SPRING_SEED],
    },
    Row {
        name: "SPRING_3D",
        motor: Some("layout.force.spring3d"),
        reference: Reference::Scigraphs,
        gaps: &[G_SPRING_SEED],
    },
    Row {
        name: "CIRCLE_PACKING",
        motor: Some("layout.packing.circle"),
        reference: Reference::Scigraphs,
        gaps: &[],
    },
    Row {
        name: "FORCEATLAS2",
        motor: Some("layout.forceatlas2"),
        reference: Reference::Scigraphs,
        gaps: &[G_FORCEATLAS2_SEED, G_SNAPSHOT_SCALE],
    },
    Row {
        name: "IGRAPH_FR",
        motor: Some("layout.force.fruchterman_reingold"),
        reference: Reference::Scigraphs,
        gaps: &[G_IGRAPH_SEED, G_SNAPSHOT_SCALE],
    },
    Row {
        name: "IGRAPH_KK",
        motor: Some("layout.force.kamada_kawai"),
        reference: Reference::Scigraphs,
        gaps: &[G_SNAPSHOT_SCALE],
    },
    Row {
        name: "IGRAPH_DRL",
        motor: Some("layout.force.drl"),
        reference: Reference::Scigraphs,
        gaps: &[G_IGRAPH_SEED, G_SNAPSHOT_SCALE],
    },
    Row {
        name: "IGRAPH_DRL_2D",
        motor: Some("layout.force.drl"),
        reference: Reference::Scigraphs,
        gaps: &[G_IGRAPH_SEED, G_SNAPSHOT_SCALE],
    },
    Row {
        name: "IGRAPH_LGL",
        motor: Some("layout.force.lgl"),
        reference: Reference::Scigraphs,
        gaps: &[G_IGRAPH_SEED, G_SNAPSHOT_SCALE],
    },
    Row {
        name: "SPHERE",
        motor: Some("layout.basic3d.sphere"),
        reference: Reference::Scigraphs,
        gaps: &[G_BASIC3D_SCALE],
    },
    Row {
        name: "SPECTRAL_3D",
        motor: Some("layout.spectral"),
        reference: Reference::Scigraphs,
        gaps: &[G_NO_ITERATIONS, G_SNAPSHOT_SCALE],
    },
    Row {
        name: "SPIRAL_3D",
        motor: Some("layout.spiral"),
        reference: Reference::Scigraphs,
        gaps: &[G_NO_ITERATIONS, G_SNAPSHOT_SCALE],
    },
    Row {
        name: "HELIX",
        motor: Some("layout.basic3d.helix"),
        reference: Reference::Scigraphs,
        gaps: &[G_BASIC3D_SCALE],
    },
    Row {
        name: "CUBE",
        motor: Some("layout.basic3d.cube"),
        reference: Reference::Scigraphs,
        gaps: &[G_BASIC3D_SCALE, G_CUBE_SEED],
    },
    Row {
        name: "HIERARCHICAL_3D",
        motor: Some("layout.hierarchical3d"),
        reference: Reference::Scigraphs,
        gaps: &[G_NO_ITERATIONS, G_SNAPSHOT_SCALE],
    },
    Row {
        name: "BIPARTITE_3D",
        motor: Some("layout.bipartite"),
        reference: Reference::Scigraphs,
        gaps: &[G_NO_ITERATIONS, G_SNAPSHOT_SCALE],
    },
    Row {
        name: "IGRAPH_DH",
        motor: Some("layout.force.davidson_harel"),
        reference: Reference::Scigraphs,
        gaps: &[G_IGRAPH_SEED, G_SNAPSHOT_SCALE],
    },
    Row {
        name: "IGRAPH_GRAPHOPT",
        motor: Some("layout.force.graphopt"),
        reference: Reference::Scigraphs,
        gaps: &[G_IGRAPH_SEED, G_SNAPSHOT_SCALE],
    },
    Row {
        name: "MDS_3D",
        motor: Some("layout.mds.pivot"),
        reference: Reference::Scigraphs,
        gaps: &[G_NO_ITERATIONS, G_SNAPSHOT_SCALE],
    },
    Row {
        name: "YIFAN_HU",
        motor: Some("layout.force.yifan_hu"),
        reference: Reference::Graphviz("sfdp"),
        // The only row of the nine whose `dimension` is not `"2"`: `sfdp_dim` defaults to
        // `"2Z"`, so SciGraphs derives a spectral z this arm has no way to reach.
        gaps: &[G_GV_UTILS, G_GV_Z],
    },
    Row {
        name: "GRAPHVIZ_DOT",
        motor: None,
        reference: Reference::Graphviz("dot"),
        // `_scigraphs_utils_graphviz_layout` is handed `dimension=None` here too, so `dot`'s
        // `graphviz_dim` is the `"2"` default and the z gap belongs to `YIFAN_HU` alone.
        gaps: &[G_GV_UTILS, G_GV_DIRECTED],
    },
    Row {
        name: "GRAPHVIZ_NEATO",
        motor: Some("layout.force.neato"),
        reference: Reference::Graphviz("neato"),
        gaps: &[G_GV_UTILS, G_NEATO_START],
    },
    Row {
        name: "GRAPHVIZ_FDP",
        motor: Some("layout.force.fdp"),
        reference: Reference::Graphviz("fdp"),
        gaps: &[G_GV_UTILS, G_FDP_SEED],
    },
    Row {
        name: "GRAPHVIZ_SFDP",
        motor: Some("layout.force.sfdp"),
        reference: Reference::Graphviz("sfdp"),
        // No seed gap: the arm calls `run_seeded(get_layout_seed())` and the engine is given
        // `-Gstart=get_layout_seed()`, so **both sides are at the same seed and still differ**.
        // That is the finding, and it is why this row's cause is `algorithm`.
        gaps: &[G_GV_UTILS],
    },
    Row {
        name: "GRAPHVIZ_TWOPI",
        motor: Some("layout.twopi"),
        reference: Reference::Graphviz("twopi"),
        gaps: &[G_GV_UTILS, G_TWOPI_ROOT],
    },
    Row {
        name: "GRAPHVIZ_CIRCO",
        motor: Some("layout.circular.circo"),
        reference: Reference::Graphviz("circo"),
        gaps: &[G_GV_UTILS],
    },
    Row {
        name: "GRAPHVIZ_OSAGE",
        motor: Some("layout.packing.osage"),
        reference: Reference::Graphviz("osage"),
        gaps: &[G_GV_UTILS, G_OSAGE_BOX],
    },
    Row {
        name: "GRAPHVIZ_PATCHWORK",
        motor: Some("layout.treemap.patchwork"),
        reference: Reference::Graphviz("patchwork"),
        gaps: &[G_GV_UTILS],
    },
    Row {
        name: "SUGIYAMA",
        motor: Some("layout.dag.sugiyama"),
        reference: Reference::Scigraphs,
        gaps: &[G_NO_ITERATIONS, G_SCALE_FIXED_LAYER],
    },
    Row {
        name: "CIRCULAR_HIERARCHY",
        motor: Some("layout.circular.hierarchy"),
        reference: Reference::Scigraphs,
        gaps: &[G_NO_ITERATIONS, G_SNAPSHOT_SCALE],
    },
];
