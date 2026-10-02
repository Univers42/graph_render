//! [`super::Knob`]'s three lists — every arm, the variable each is set by, and the stage
//! a per-stage control perturbs.
//!
//! Split out of `knob.rs` by the house's 300-line limit, and because these three are pure
//! bookkeeping: the enum above states *what* a control is, and this module is the one
//! place the names live, so `Knob::env`, `Knob::ALL` and [`stage_of`] are three thin
//! delegations rather than three lists that could disagree. The igraph group's names come
//! from [`super::igraph`] rather than being spelled a sixth time.

use super::{Knob, igraph, knobs, three_d};

/// Every knob, in the order the gate reads them: the arms that move a parameter or
/// re-draw one layout's model, then the fifteen ANALYSIS and POST stage controls in
/// [`knobs::ANALYSIS_POST_STAGES`] order, then the six igraph layout controls in
/// [`knobs::IGRAPH_LAYOUT_STAGES`] order, then the two compute-tier controls, then the
/// live session's own.
///
/// **A `const`, because `capabilities::verdict::Evidence::load` walks it** to collect one
/// control record each — a ledger read cannot be a function call per row. `Knob::ALL` is
/// this array, re-exported so the name every caller already used keeps working.
pub const ALL: [Knob; 45] = [
    Knob::ReferenceDegree,
    Knob::GridSpacing,
    Knob::SugiyamaLayerSpacing,
    Knob::NodeCount,
    Knob::ForceTheta,
    Knob::Fa2ScalingRatio,
    Knob::TreeTidyNodes,
    Knob::TreemapNodes,
    Knob::CircularNodes,
    Knob::TwopiNodes,
    Knob::NeatoEpsilon,
    Knob::PatchworkNodes,
    Knob::SpringIterations,
    Knob::CircularHierarchyNodes,
    Knob::PackingScale,
    Knob::AnalysisComponentsWeak,
    Knob::AnalysisComponentsStrong,
    Knob::AnalysisCommunitiesLouvain,
    Knob::AnalysisCentralityDegree,
    Knob::AnalysisCentralityCloseness,
    Knob::AnalysisCentralityBetweenness,
    Knob::AnalysisCentralityEigenvector,
    Knob::AnalysisDepthBfs,
    Knob::PostBundleFdeb,
    Knob::PostBundleMingle,
    Knob::PostRouteGrid,
    Knob::PostStyleStraight,
    Knob::PostStyleOrthogonal,
    Knob::PostStyleQuadratic,
    Knob::PostStyleBezier,
    Knob::IgraphFruchtermanReingoldNodes,
    Knob::IgraphKamadaKawaiNodes,
    Knob::IgraphGraphoptNodes,
    Knob::IgraphDavidsonHarelNodes,
    Knob::IgraphLglNodes,
    Knob::IgraphDrlNodes,
    Knob::Basic3dSphereNodes,
    Knob::Basic3dHelixNodes,
    Knob::Basic3dCubeNodes,
    Knob::Hierarchical3dNodes,
    Knob::Spring3dNodes,
    Knob::PackingOsageNodes,
    Knob::SplitSum,
    Knob::SplitRescale,
    Knob::ForceSessionGravity,
];

/// The variable that sets `knob`.
pub const fn env(knob: Knob) -> &'static str {
    match knob {
        Knob::ReferenceDegree => "GM_MUTATE_REFERENCE_DEGREE",
        Knob::GridSpacing => "GM_MUTATE_GRID_SPACING",
        Knob::SugiyamaLayerSpacing => "GM_MUTATE_SUGIYAMA_LAYER_SPACING",
        Knob::NodeCount => "GM_MUTATE_NODE_COUNT",
        Knob::ForceTheta => "GM_MUTATE_FORCE_THETA",
        Knob::Fa2ScalingRatio => "GM_MUTATE_FA2_SCALING_RATIO",
        Knob::TreeTidyNodes => "GM_MUTATE_TREE_TIDY_NODES",
        Knob::TreemapNodes => "GM_MUTATE_TREEMAP_NODES",
        Knob::CircularNodes => "GM_MUTATE_CIRCULAR_NODES",
        Knob::TwopiNodes => "GM_MUTATE_TWOPI_NODES",
        Knob::NeatoEpsilon => "GM_MUTATE_NEATO_EPSILON",
        Knob::PatchworkNodes => "GM_MUTATE_PATCHWORK_NODES",
        Knob::SpringIterations => "GM_MUTATE_SPRING_ITERATIONS",
        Knob::CircularHierarchyNodes => "GM_MUTATE_CIRCULAR_HIERARCHY_NODES",
        Knob::PackingScale => "GM_MUTATE_PACKING_SCALE",
        Knob::AnalysisComponentsWeak => "GM_MUTATE_ANALYSIS_COMPONENTS_WEAK",
        Knob::AnalysisComponentsStrong => "GM_MUTATE_ANALYSIS_COMPONENTS_STRONG",
        Knob::AnalysisCommunitiesLouvain => "GM_MUTATE_ANALYSIS_COMMUNITIES_LOUVAIN",
        Knob::AnalysisCentralityDegree => "GM_MUTATE_ANALYSIS_CENTRALITY_DEGREE",
        Knob::AnalysisCentralityCloseness => "GM_MUTATE_ANALYSIS_CENTRALITY_CLOSENESS",
        Knob::AnalysisCentralityBetweenness => "GM_MUTATE_ANALYSIS_CENTRALITY_BETWEENNESS",
        Knob::AnalysisCentralityEigenvector => "GM_MUTATE_ANALYSIS_CENTRALITY_EIGENVECTOR",
        Knob::AnalysisDepthBfs => "GM_MUTATE_ANALYSIS_DEPTH_BFS",
        Knob::PostBundleFdeb => "GM_MUTATE_POST_BUNDLE_FDEB",
        Knob::PostBundleMingle => "GM_MUTATE_POST_BUNDLE_MINGLE",
        Knob::PostRouteGrid => "GM_MUTATE_POST_ROUTE_GRID",
        Knob::PostStyleStraight => "GM_MUTATE_POST_STYLE_STRAIGHT",
        Knob::PostStyleOrthogonal => "GM_MUTATE_POST_STYLE_ORTHOGONAL",
        Knob::PostStyleQuadratic => "GM_MUTATE_POST_STYLE_QUADRATIC",
        Knob::PostStyleBezier => "GM_MUTATE_POST_STYLE_BEZIER",
        Knob::IgraphFruchtermanReingoldNodes => igraph::ENV[0],
        Knob::IgraphKamadaKawaiNodes => igraph::ENV[1],
        Knob::IgraphGraphoptNodes => igraph::ENV[2],
        Knob::IgraphDavidsonHarelNodes => igraph::ENV[3],
        Knob::IgraphLglNodes => igraph::ENV[4],
        Knob::IgraphDrlNodes => igraph::ENV[5],
        Knob::Basic3dSphereNodes => three_d::ENV[0],
        Knob::Basic3dHelixNodes => three_d::ENV[1],
        Knob::Basic3dCubeNodes => three_d::ENV[2],
        Knob::Hierarchical3dNodes => three_d::ENV[3],
        Knob::Spring3dNodes => three_d::ENV[4],
        Knob::PackingOsageNodes => knobs::OSAGE_LAYOUT_STAGES[0].env,
        Knob::SplitSum => "GM_MUTATE_SPLIT_SUM",
        Knob::SplitRescale => "GM_MUTATE_SPLIT_RESCALE",
        Knob::ForceSessionGravity => "GM_MUTATE_FORCE_SESSION_GRAVITY",
    }
}

/// The [`knobs::Stage`] `knob` perturbs — a function of its *variable*, not its arm index.
///
/// Resolved by matching the variable name against the one table, so a control cannot be
/// filed under a stage the table does not agree with: a variable the table does not carry
/// is a programming error, not a runtime setting, and it panics here rather than quietly
/// perturbing whichever stage happened to sit at that arm's position.
pub(in crate::hashgate) fn stage_of(knob: Knob) -> knobs::Stage {
    let name = env(knob);
    knobs::all()
        .find(|row| row.env == name)
        .unwrap_or_else(|| panic!("{name} is one of the per-stage controls"))
}
