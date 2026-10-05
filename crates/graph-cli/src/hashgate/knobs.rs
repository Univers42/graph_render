//! The per-stage negative controls that back a control with a re-drawn model rather than a
//! real parameter, and the `GM_MUTATE_*` variable each one is set by: the fifteen ANALYSIS
//! and POST stages, the six igraph layouts, the seven natively 3D layouts, the one Graphviz
//! packing layout and the one Graphviz `dot` layout.
//!
//! Split out of `knob.rs` by the house's 300-line limit, and because these are one thing
//! the controls in `knob.rs` are not: they share a single shape. A stage that takes a
//! parameter backs its control with a real parameter; a stage that takes none — and
//! every analysis, every POST capability, every igraph layout, every natively 3D layout,
//! `layout.packing.osage` and `layout.dag.dot` take none, being a pure function of the gate's
//! model at fixed conventions — backs its control by re-drawing **its own** model with
//! [`Setting::stage_nodes`], one more node than the gate's own, for that stage alone.
//!
//! Re-drawing is the honest probe for a stage with no parameter to move, and it is what
//! makes a divergence *name* the stage: `GM_MUTATE_NODE_COUNT` would perturb the gate's
//! one shared model and move every stage at once, which proves nothing about any of them.
//! The test `each_analysis_and_post_stage_has_its_own_control_that_moves_only_its_stage`
//! is what keeps that claim true for every row tabled here.

use graph_core::analysis::{centrality, communities, components, depth};
use graph_core::post::routed;
use graph_core::post::styles::Style;
use graph_core::post::{fdeb, mingle};

mod three_d;
pub use three_d::THREE_D_LAYOUT_STAGES;

use super::Setting;
use graph_core::Stage as _;

/// The stage one control perturbs, named by the constant its own module publishes.
///
/// **Every id here is a graph-core constant**, never a spelling in this file: a
/// hand-copied literal is a string that can drift from the registry row the gate hashes
/// under, and a control filed under a drifted id perturbs nothing while passing as
/// green. The test `each_stage_id_is_the_constant_its_own_module_publishes` holds every
/// one of them against the registry the gate walks.
pub const ANALYSIS_POST_STAGES: [Stage; 15] = [
    Stage {
        id: components::WEAK,
        env: "GM_MUTATE_ANALYSIS_COMPONENTS_WEAK",
        record: "hashgate-control-analysis-components-weak",
    },
    Stage {
        id: components::STRONG,
        env: "GM_MUTATE_ANALYSIS_COMPONENTS_STRONG",
        record: "hashgate-control-analysis-components-strong",
    },
    Stage {
        id: communities::LOUVAIN,
        env: "GM_MUTATE_ANALYSIS_COMMUNITIES_LOUVAIN",
        record: "hashgate-control-analysis-communities-louvain",
    },
    Stage {
        id: centrality::DEGREE,
        env: "GM_MUTATE_ANALYSIS_CENTRALITY_DEGREE",
        record: "hashgate-control-analysis-centrality-degree",
    },
    Stage {
        id: centrality::CLOSENESS,
        env: "GM_MUTATE_ANALYSIS_CENTRALITY_CLOSENESS",
        record: "hashgate-control-analysis-centrality-closeness",
    },
    Stage {
        id: centrality::BETWEENNESS,
        env: "GM_MUTATE_ANALYSIS_CENTRALITY_BETWEENNESS",
        record: "hashgate-control-analysis-centrality-betweenness",
    },
    Stage {
        id: centrality::EIGENVECTOR,
        env: "GM_MUTATE_ANALYSIS_CENTRALITY_EIGENVECTOR",
        record: "hashgate-control-analysis-centrality-eigenvector",
    },
    Stage {
        id: depth::BFS,
        env: "GM_MUTATE_ANALYSIS_DEPTH_BFS",
        record: "hashgate-control-analysis-depth-bfs",
    },
    Stage {
        id: fdeb::ID,
        env: "GM_MUTATE_POST_BUNDLE_FDEB",
        record: "hashgate-control-post-bundle-fdeb",
    },
    Stage {
        id: mingle::ID,
        env: "GM_MUTATE_POST_BUNDLE_MINGLE",
        record: "hashgate-control-post-bundle-mingle",
    },
    Stage {
        id: routed::ID,
        env: "GM_MUTATE_POST_ROUTE_GRID",
        record: "hashgate-control-post-route-grid",
    },
    Stage {
        id: Style::Straight.id(),
        env: "GM_MUTATE_POST_STYLE_STRAIGHT",
        record: "hashgate-control-post-style-straight",
    },
    Stage {
        id: Style::Orthogonal.id(),
        env: "GM_MUTATE_POST_STYLE_ORTHOGONAL",
        record: "hashgate-control-post-style-orthogonal",
    },
    Stage {
        id: Style::Quadratic.id(),
        env: "GM_MUTATE_POST_STYLE_QUADRATIC",
        record: "hashgate-control-post-style-quadratic",
    },
    Stage {
        id: Style::Bezier.id(),
        env: "GM_MUTATE_POST_STYLE_BEZIER",
        record: "hashgate-control-post-style-bezier",
    },
];

/// The six igraph-family layouts' per-stage node controls, the same shape as the fifteen
/// above and for the same reason.
///
/// None of the six takes a parameter the gate can move: the gate hashes each from the
/// registry's own `run` closure, at the compiled-in defaults its module pins, so there is
/// no real parameter to perturb. That leaves the honest probe the three Phase 3 layout
/// controls use — re-draw **this** stage's model with one more node, for this stage alone —
/// which is exactly what [`Setting::stage_nodes`] and
/// `super::stages::stage_bytes_from_own_model` do for a layout id.
///
/// A shared control would not do: `GM_MUTATE_NODE_COUNT` grows the gate's one model, so it
/// moves all six at once and names none of them, which is the failure mode the whole
/// per-stage family exists to prevent. So one control per layout, one record per control.
///
/// **Every id here is a graph-core constant** (`<Layout>::ID` through the `Stage` trait),
/// never a spelling in this file, for the reason the fifteen above give.
pub const IGRAPH_LAYOUT_STAGES: [Stage; 6] = [
    Stage {
        id: graph_core::layout::force::FruchtermanReingold::ID,
        env: "GM_MUTATE_FORCE_FRUCHTERMAN_REINGOLD_NODES",
        record: "hashgate-control-force-fruchterman-reingold-nodes",
    },
    Stage {
        id: graph_core::layout::force::KamadaKawai::ID,
        env: "GM_MUTATE_FORCE_KAMADA_KAWAI_NODES",
        record: "hashgate-control-force-kamada-kawai-nodes",
    },
    Stage {
        id: graph_core::layout::force::Graphopt::ID,
        env: "GM_MUTATE_FORCE_GRAPHOPT_NODES",
        record: "hashgate-control-force-graphopt-nodes",
    },
    Stage {
        id: graph_core::layout::force::DavidsonHarel::ID,
        env: "GM_MUTATE_FORCE_DAVIDSON_HAREL_NODES",
        record: "hashgate-control-force-davidson-harel-nodes",
    },
    Stage {
        id: graph_core::layout::force::Lgl::ID,
        env: "GM_MUTATE_FORCE_LGL_NODES",
        record: "hashgate-control-force-lgl-nodes",
    },
    Stage {
        id: graph_core::layout::force::Drl::ID,
        env: "GM_MUTATE_FORCE_DRL_NODES",
        record: "hashgate-control-force-drl-nodes",
    },
];

/// The one Graphviz packing layout's per-stage node control, the same shape as the six above
/// and for the same reason.
///
/// **The re-drawn model is the *only* probe available here**, so it is worth saying why.
/// `osage` publishes no `Params` and has no `impl Stage` — its module says so, and names the
/// escape hatch that would change it (`osage.rs:72`: "a `Params` on the stage, which is a
/// contract change and not this job's"). What it *does* read is the node count and no edge at
/// all, so its model is its entire input and one more node is exactly what moves it. The
/// re-draw is scoped to `stage_nodes`, so `topology`, the other layouts and the transport
/// stage stay byte-identical and the divergence names this stage.
///
/// **This row is what `layout.packing.osage` needs for `Status::Gated`.** `verdict::hash_4way`
/// refuses a gated row whose own stage no control went red on, and the controls that did go red
/// do not reach it: `GM_MUTATE_NODE_COUNT` moves `topology` and every topology-shaped stage,
/// and `GM_MUTATE_PACKING_SCALE` is `layout.packing.circle`'s real parameter — osage packs its
/// own uniform grid and reads no scale, so it stays equal under both. Without a control of its
/// own the row is `implemented` however good its oracle record is.
///
/// **The id is graph-core's own `osage::ID`**, never a spelling here, for the reason the three
/// tables above give.
pub const OSAGE_LAYOUT_STAGES: [Stage; 1] = [Stage {
    id: graph_core::layout::graphviz::osage::ID,
    env: "GM_MUTATE_PACKING_OSAGE_NODES",
    record: "hashgate-control-packing-osage-nodes",
}];

/// The one Graphviz `dot` layout's per-stage node control, the same shape as the two above
/// and for the same reason.
///
/// **The re-drawn model is the *only* probe available here**, and the port says so itself:
/// `dot` publishes no `Params` and has no `impl Stage`, recording that the reference's
/// `rankdir`, `nodesep`, `ranksep`, `ratio`, `nslimit` and `searchsize` are set by neither the
/// ledger row nor the oracle, so a `Params` here would buy a knob with nothing behind it
/// (`dot.rs:169-172`). What it *does* read is the whole `Topology`, so its model is its entire
/// input and one more node moves its ranks, its ordering and its coordinates.
///
/// The re-draw is scoped to `stage_nodes`, so `topology`, the other layouts and the transport
/// stage stay byte-identical and the divergence names this stage.
///
/// **This row is what `layout.dag.dot` needs for `Status::Gated`**, by the same argument the
/// osage row above gives: `verdict::hash_4way` refuses a gated row whose own stage no control
/// went red on, and the controls that do go red do not reach it — `GM_MUTATE_NODE_COUNT` moves
/// `topology` and everything shaped by it, and no parameter control names a `dot` input.
///
/// **The id is graph-core's own `dot::ID`**, never a spelling here, for the reason the tables
/// above give.
pub const DOT_LAYOUT_STAGES: [Stage; 1] = [Stage {
    id: graph_core::layout::graphviz::dot::ID,
    env: "GM_MUTATE_DAG_DOT_NODES",
    record: "hashgate-control-dag-dot-nodes",
}];

/// Every per-stage control this module tables: the fifteen ANALYSIS and POST rows, then the
/// six igraph layout rows, then the seven 3D layout rows, then the one Graphviz packing row and
/// the one Graphviz `dot` row — one search list, so [`super::knob::stage_of`] resolves every
/// family through the same table lookup and none can drift from another's shape.
pub fn all() -> impl Iterator<Item = Stage> {
    ANALYSIS_POST_STAGES
        .iter()
        .copied()
        .chain(IGRAPH_LAYOUT_STAGES.iter().copied())
        .chain(THREE_D_LAYOUT_STAGES.iter().copied())
        .chain(OSAGE_LAYOUT_STAGES.iter().copied())
        .chain(DOT_LAYOUT_STAGES.iter().copied())
}

/// One ANALYSIS or POST stage's negative control: the stage it perturbs, the variable
/// that sets it, and the record its run writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stage {
    /// The gate stage id this control moves — graph-core's constant for that analysis or
    /// capability, never a copy.
    pub id: &'static str,
    /// The `GM_MUTATE_*` variable that sets it.
    pub env: &'static str,
    /// The evidence record a run with this control set writes.
    pub record: &'static str,
}

/// The row for `env`, or `None` for a variable that tables no per-stage control.
///
/// **Exact, by string, never by prefix**: a `GM_MUTATE_POST_STYLE_` that names no one of
/// the four styles is a typo, and resolving it to the nearest row would file the
/// perturbation under a stage nobody asked for — the failure mode
/// [`Setting::stage_nodes`] exists to prevent. A caller that has already been dispatched
/// by the variable it read ([`super::knob::stage_of`]) cannot reach the `None`; a test
/// reaches it to hold the table's own coverage.
#[cfg(test)]
pub fn by_env(env: &str) -> Option<Stage> {
    all().find(|row| row.env == env)
}

/// `stage`'s perturbation written into `setting`: `count` nodes added to *its* model.
///
/// Zero is refused by the caller (`knob::value::nodes`), not here: the rule is one rule for
/// all thirty tabled controls, and a second copy of it here would be a second place for
/// it to drift.
///
/// **The stage id is checked against the gate's own stage list** (RG-41): `knobs::apply`
/// used to store whatever id it was handed, so a row added to this table with a stage the
/// gate does not hash put a `stage_nodes` entry that no stage ever reads — a control that
/// perturbs nothing and still recorded itself as exercised. The one reader that can tell is
/// [`super::stages()`], so the check is here rather than left to every future caller.
pub fn apply(stage: Stage, count: u32, setting: &mut Setting) -> Result<(), String> {
    if !super::stages().contains(&stage.id) {
        return Err(format!(
            "{} perturbs {}, which is not one of the {} stages the gate hashes",
            stage.env,
            stage.id,
            super::stages().len()
        ));
    }
    setting.stage_nodes = Some((stage.id, count));
    Ok(())
}
