//! The fifteen per-stage negative controls for the ANALYSIS and POST stages, and the
//! `GM_MUTATE_*` variable each one is set by.
//!
//! Split out of `knob.rs` by the house's 300-line limit, and because these fifteen are
//! one thing the ten in `knob.rs` are not: they share a single shape. A stage that takes
//! a parameter backs its control with a real parameter; a stage that takes none — and
//! every analysis and every POST capability takes none, being a pure function of the
//! gate's model at fixed conventions — backs its control by re-drawing **its own** model
//! with [`Setting::stage_nodes`], one more node than the gate's own, for that stage
//! alone.
//!
//! Re-drawing is the honest probe for a stage with no parameter to move, and it is what
//! makes a divergence *name* the stage: `GM_MUTATE_NODE_COUNT` would perturb the gate's
//! one shared model and move every stage at once, which proves nothing about any of them.
//! The test `each_analysis_and_post_stage_has_its_own_control_that_moves_only_its_stage`
//! is what keeps that claim true for all fifteen.

use graph_core::analysis::{centrality, communities, components, depth};
use graph_core::post::routed;
use graph_core::post::styles::Style;
use graph_core::post::{fdeb, mingle};

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
/// which is exactly what [`Setting::stage_nodes`] and [`stage_bytes_from_own_model`] do for
/// a layout id.
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

/// Every per-stage control this module tables, the fifteen ANALYSIS and POST rows then the
/// six igraph layout rows — one search list, so [`super::knob::stage_of`] resolves both
/// families through the same table lookup and neither can drift from the other's shape.
pub fn all() -> impl Iterator<Item = Stage> {
    ANALYSIS_POST_STAGES
        .iter()
        .copied()
        .chain(IGRAPH_LAYOUT_STAGES.iter().copied())
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
/// Zero is refused by the caller (`knob::nodes`), not here: the rule is one rule for all
/// twenty-five controls, and a second copy of it beside these fifteen would be a second
/// place for it to drift.
pub fn apply(stage: Stage, count: u32, setting: &mut Setting) {
    setting.stage_nodes = Some((stage.id, count));
}
