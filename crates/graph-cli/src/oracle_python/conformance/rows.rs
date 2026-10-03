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
        gaps: &[G_RANDOM_ITER],
    },
    Row {
        name: "GRID",
        motor: Some("layout.grid"),
        reference: Reference::Scigraphs,
        gaps: &[G_GRID_ITER],
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
        // **Not `layout.spectral`.** That id is this reference's *own* kernel at `dims = 2`,
        // which is what the 2D differential (`harness/oracle-spectral.py`) pins. SciGraphs'
        // `SPECTRAL_3D` is `_spectral_layout_3d` (`networkx_layouts.py:249-269`): three
        // coordinates, the cubic component lattice and `_rescale_positions`. Running the 2D
        // id here drew a plane against a volume — grey a line, green a cluster.
        name: "SPECTRAL_3D",
        motor: Some("layout.spectral3d"),
        reference: Reference::Scigraphs,
        gaps: &[G_NO_ITERATIONS, G_SNAPSHOT_SCALE],
    },
    Row {
        name: "SPIRAL_3D",
        motor: Some("layout.basic3d.spiral"),
        reference: Reference::Scigraphs,
        gaps: &[G_BASIC3D_SCALE],
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
        gaps: &[G_BASIC3D_SCALE],
    },
    Row {
        name: "HIERARCHICAL_3D",
        motor: Some("layout.hierarchical3d"),
        reference: Reference::Scigraphs,
        gaps: &[G_NO_ITERATIONS, G_SNAPSHOT_SCALE],
    },
    Row {
        name: "BIPARTITE_3D",
        // **Not `layout.bipartite`.** That id is networkx's `bipartite_layout`: two vertical
        // columns in a rescaled unit box, a planar `Geometry` with no z at all. This row's
        // reference is two horizontal rings at `z = +/- scale*0.6/2`, so the motor arm has
        // to be a z-bearing layout or the comparison is between two different drawings.
        motor: Some("layout.bipartite_3d"),
        reference: Reference::Scigraphs,
        // `G_BASIC3D_SCALE` and not `G_SNAPSHOT_SCALE`: both name the same number at the
        // same line (`basic_3d.rs:43`), and this row now reads its scale through
        // `basic_3d::SCALE` like `SPHERE`, `HELIX` and `CUBE` do. `G_NO_ITERATIONS` still
        // holds — the colouring and the placement are both closed forms.
        gaps: &[G_NO_ITERATIONS, G_BASIC3D_SCALE],
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
        // **Not `layout.mds.pivot`**, for the same reason as `SPECTRAL_3D`: SciGraphs'
        // `MDS_3D` is `_mds_layout_3d` (`:271-291`), the three-coordinate entry, and the 2D id
        // is the one `harness/oracle-spectral.py` pins.
        name: "MDS_3D",
        motor: Some("layout.mds.pivot3d"),
        reference: Reference::Scigraphs,
        gaps: &[G_NO_ITERATIONS, G_SNAPSHOT_SCALE],
    },
    Row {
        name: "YIFAN_HU",
        motor: Some("layout.force.yifan_hu"),
        reference: Reference::Graphviz("sfdp"),
        gaps: &[G_GV_UTILS],
    },
    Row {
        name: "GRAPHVIZ_DOT",
        motor: None,
        reference: Reference::Graphviz("dot"),
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
        // No gaps. `scale` used to be one (`G_SCALE_FIXED_LAYER`) and is not any more:
        // `sugiyama::run_scaled` takes it as a parameter, so the arm hands the reference's
        // `scale = 5.0` to the motor too (`hierarchical.py:638`). `iterations` used to be
        // one (`G_NO_ITERATIONS`) and was a false record: `_sugiyama_layout(G, scale)`
        // takes no count at all and `dispatcher.py:142-143` passes it only `scale`, so there
        // is no budget the reference ran with for this layout to have failed to bound.
        gaps: &[],
    },
    Row {
        name: "CIRCULAR_HIERARCHY",
        motor: Some("layout.circular.hierarchy"),
        reference: Reference::Scigraphs,
        gaps: &[G_NO_ITERATIONS, G_SNAPSHOT_SCALE],
    },
];
