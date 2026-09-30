//! The negative controls' knobs and the setting the native arm runs with: every
//! variable is read strictly and at most one may be set.

use super::knobs;

pub(super) mod setting;
pub(super) use setting::{Setting, env_setting};

/// A negative control (`prompt.md` §7.2): a variable that perturbs the native arm only,
/// so a wired mutation surfaces as exactly the cross-target divergence the gate must
/// catch. The grid ignores weights, so the reference degree cannot reach `layout.grid`,
/// and the grid's spacing is what backs that stage. Treemap reads node weight, so the
/// reference degree backs it too. The layered drawing ignores weights too; its layer
/// spacing backs `layout.dag.sugiyama`.
///
/// **Every stage the gate hashes has a control of its own, and none of them is
/// [`Knob::NodeCount`].** Node count perturbs the gate's one shared model, so it moves
/// every stage that is a function of the topology at all: it backs the stages nothing
/// else reaches (spectral, pivot MDS), but a control that moves eleven stages at once
/// cannot say *which* stage a divergence came from, which is the whole point of hashing
/// them one at a time. So the four Phase 3 layouts and Barnes-Hut each have a control
/// filed under their own stage id, and the test
/// `each_p3_layout_has_its_own_negative_control_that_moves_only_its_stage` is what keeps
/// them honest.
///
/// **What each of the four perturbs, and why it is not one thing.** Circle packing is
/// the only one that publishes parameters ([`graph_core::layout::circle_packing::
/// CirclePackingParams`]), so [`Knob::PackingScale`] moves a real parameter of that
/// layout. The other three take none by design — `layout::tidy_tree`,
/// `layout::treemap` and `layout::circular` pin their own conventions and say in their
/// own module docs that adding a `Params` to gain a knob would be the tail wagging the
/// dog — so their controls re-draw *that one stage's* model with one more node instead
/// ([`Knob::TreeTidyNodes`], [`Knob::TreemapNodes`], [`Knob::CircularNodes`]). Same
/// probe as node count, scoped to one stage: it is the honest way to move a layout that
/// has no parameter to move, and it is what makes the divergence *name* the stage.
///
/// **The fifteen ANALYSIS and POST controls are the same probe again**, and for the same
/// reason: no analysis and no POST capability takes a parameter, being a pure function
/// of the gate's model at fixed conventions, so each of them re-draws *its own* model
/// with one more node through [`Setting::stage_nodes`] and moves that stage alone. They
/// are tabulated in [`knobs::ANALYSIS_POST_STAGES`], and
/// `the_analysis_and_post_controls_are_the_knobs_table` holds this enum's arms to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Knob {
    /// `GM_MUTATE_REFERENCE_DEGREE`: the degree the topology's weights are taken against.
    ReferenceDegree,
    /// `GM_MUTATE_GRID_SPACING`: the grid's spacing.
    GridSpacing,
    /// `GM_MUTATE_SUGIYAMA_LAYER_SPACING`: the layered drawing's Y step per layer.
    SugiyamaLayerSpacing,
    /// `GM_MUTATE_NODE_COUNT`: nodes added to the model, native arm only.
    NodeCount,
    /// `GM_MUTATE_FORCE_THETA`: Barnes-Hut's opening angle, native arm only.
    ///
    /// Its own control, and the only one that reaches `layout.force.barnes_hut`
    /// without touching anything else: theta is read by the quadtree's opening test
    /// alone, so perturbing it re-aggregates that stage's many-body force and leaves
    /// every other stage — including `layout.forceatlas2`, which shares no code
    /// with it — byte-identical. A shared knob would back both force stages at once
    /// and prove nothing about either.
    ForceTheta,
    /// `GM_MUTATE_FA2_SCALING_RATIO`: ForceAtlas2's repulsion scale, native arm only.
    ///
    /// Its own control for the same reason, on the other side: `scaling_ratio` is
    /// read by `Fa2State::repulsion` alone.
    Fa2ScalingRatio,
    /// `GM_MUTATE_TREE_TIDY_NODES`: nodes added to `layout.tree.tidy`'s model alone.
    ///
    /// The tidy tree takes no parameters (its module pins every d3 convention), so its
    /// own control perturbs the one thing it does read — the model — for that stage only.
    TreeTidyNodes,
    /// `GM_MUTATE_TREEMAP_NODES`: nodes added to `layout.treemap.squarified`'s model
    /// alone. Squarify takes no parameters either; a new node is a new box, and the
    /// layout's stage is the only one that sees it.
    TreemapNodes,
    /// `GM_MUTATE_CIRCULAR_NODES`: nodes added to `layout.circular.radial`'s model
    /// alone. Rings come from BFS depth over the hierarchy, so one more node changes
    /// this stage's ring counts and slots and nothing else's.
    CircularNodes,
    /// `GM_MUTATE_PACKING_SCALE`: the packing's `CirclePackingParams::scale`, native arm
    /// only.
    ///
    /// A real parameter rather than a re-drawn model, because circle packing is the one
    /// of the four that publishes `run_with`; the scale is read by the final centring and
    /// so changes every circle's centre and radius.
    PackingScale,
    /// `GM_MUTATE_ANALYSIS_COMPONENTS_WEAK`: weak components, native arm only.
    ///
    /// The first of the fifteen ANALYSIS and POST controls, which share one shape and are
    /// tabulated in [`knobs::ANALYSIS_POST_STAGES`] — that table holds each one's stage
    /// id (a graph-core constant), its variable and its record, and
    /// `the_analysis_and_post_controls_are_the_knobs_table` holds this enum's arms to it.
    ///
    /// **None of the fifteen moves a parameter**, because no ANALYSIS or POST stage takes
    /// one: each re-draws *its own* model with one more node, through
    /// [`Setting::stage_nodes`], and moves that stage alone.
    AnalysisComponentsWeak,
    /// `GM_MUTATE_ANALYSIS_COMPONENTS_STRONG`: strong components, native arm only.
    AnalysisComponentsStrong,
    /// `GM_MUTATE_ANALYSIS_COMMUNITIES_LOUVAIN`: Louvain communities, native arm only.
    AnalysisCommunitiesLouvain,
    /// `GM_MUTATE_ANALYSIS_CENTRALITY_DEGREE`: degree centrality, native arm only.
    AnalysisCentralityDegree,
    /// `GM_MUTATE_ANALYSIS_CENTRALITY_CLOSENESS`: closeness centrality, native arm only.
    AnalysisCentralityCloseness,
    /// `GM_MUTATE_ANALYSIS_CENTRALITY_BETWEENNESS`: betweenness, native arm only.
    AnalysisCentralityBetweenness,
    /// `GM_MUTATE_ANALYSIS_CENTRALITY_EIGENVECTOR`: eigenvector, native arm only.
    AnalysisCentralityEigenvector,
    /// `GM_MUTATE_ANALYSIS_DEPTH_BFS`: BFS depth, native arm only.
    AnalysisDepthBfs,
    /// `GM_MUTATE_POST_BUNDLE_FDEB`: FDEB bundling, native arm only.
    PostBundleFdeb,
    /// `GM_MUTATE_POST_BUNDLE_MINGLE`: MINGLE bundling, native arm only.
    PostBundleMingle,
    /// `GM_MUTATE_POST_ROUTE_GRID`: grid routing, native arm only.
    PostRouteGrid,
    /// `GM_MUTATE_POST_STYLE_STRAIGHT`: straight edges, native arm only.
    PostStyleStraight,
    /// `GM_MUTATE_POST_STYLE_ORTHOGONAL`: orthogonal edges, native arm only.
    PostStyleOrthogonal,
    /// `GM_MUTATE_POST_STYLE_QUADRATIC`: quadratic bezier edges, native arm only.
    PostStyleQuadratic,
    /// `GM_MUTATE_POST_STYLE_BEZIER`: cubic bezier edges, native arm only.
    PostStyleBezier,
    /// `GM_MUTATE_SPLIT_SUM`: **native arms only, and the threaded ones above all.**
    ///
    /// Phase 11's own control, and the one the phase prompt names: it makes a gathered
    /// pass's merge read a *neighbouring* node's delta — the shape a wrong partition of
    /// the outputs would take — so the threaded arms must diverge from the scalar one. It
    /// is the control that proves the threaded arms are actually reading their own
    /// results: a threaded arm that ignored the merge entirely would agree with a mutated
    /// one.
    ///
    /// The variable takes the **pass** whose merge is split (`charge`, `collide`, `link`),
    /// because each kernel needs its own control to be shown to be compared: a knob that
    /// only ever split the charge merge would leave the other two kernels' equality
    /// resting on nothing. `1`/`true` means all three, `0`/`false` none.
    ///
    /// The perturbation lives in that pass's merge (`barnes_hut::charge`,
    /// `barnes_hut::collide`, `barnes_hut::link`), and the setting it reads is carried on
    /// the [`Setting`] so a knob cannot change behaviour without being declared here — the
    /// same discipline every other knob obeys.
    SplitSum,
}

impl Knob {
    /// Every knob: the ten that move a parameter, then the fifteen ANALYSIS and POST
    /// stage controls in [`knobs::ANALYSIS_POST_STAGES`] order, then the compute-tier
    /// control last.
    ///
    /// **A `const`, because `capabilities::verdict::Evidence::load` walks it** to collect
    /// one control record each — a ledger read cannot be a function call per row. So the
    /// fifteen are spelled as arms here and held against that one table by
    /// `the_analysis_and_post_controls_are_the_knobs_table`, which fails on any arm whose
    /// variable, record or stage the table disagrees with.
    pub const ALL: [Self; 26] = [
        Self::ReferenceDegree,
        Self::GridSpacing,
        Self::SugiyamaLayerSpacing,
        Self::NodeCount,
        Self::ForceTheta,
        Self::Fa2ScalingRatio,
        Self::TreeTidyNodes,
        Self::TreemapNodes,
        Self::CircularNodes,
        Self::PackingScale,
        Self::AnalysisComponentsWeak,
        Self::AnalysisComponentsStrong,
        Self::AnalysisCommunitiesLouvain,
        Self::AnalysisCentralityDegree,
        Self::AnalysisCentralityCloseness,
        Self::AnalysisCentralityBetweenness,
        Self::AnalysisCentralityEigenvector,
        Self::AnalysisDepthBfs,
        Self::PostBundleFdeb,
        Self::PostBundleMingle,
        Self::PostRouteGrid,
        Self::PostStyleStraight,
        Self::PostStyleOrthogonal,
        Self::PostStyleQuadratic,
        Self::PostStyleBezier,
        Self::SplitSum,
    ];

    /// The variable that sets it.
    pub const fn env(self) -> &'static str {
        match self {
            Self::ReferenceDegree => "GM_MUTATE_REFERENCE_DEGREE",
            Self::GridSpacing => "GM_MUTATE_GRID_SPACING",
            Self::SugiyamaLayerSpacing => "GM_MUTATE_SUGIYAMA_LAYER_SPACING",
            Self::NodeCount => "GM_MUTATE_NODE_COUNT",
            Self::ForceTheta => "GM_MUTATE_FORCE_THETA",
            Self::Fa2ScalingRatio => "GM_MUTATE_FA2_SCALING_RATIO",
            Self::TreeTidyNodes => "GM_MUTATE_TREE_TIDY_NODES",
            Self::TreemapNodes => "GM_MUTATE_TREEMAP_NODES",
            Self::CircularNodes => "GM_MUTATE_CIRCULAR_NODES",
            Self::PackingScale => "GM_MUTATE_PACKING_SCALE",
            Self::AnalysisComponentsWeak => "GM_MUTATE_ANALYSIS_COMPONENTS_WEAK",
            Self::AnalysisComponentsStrong => "GM_MUTATE_ANALYSIS_COMPONENTS_STRONG",
            Self::AnalysisCommunitiesLouvain => "GM_MUTATE_ANALYSIS_COMMUNITIES_LOUVAIN",
            Self::AnalysisCentralityDegree => "GM_MUTATE_ANALYSIS_CENTRALITY_DEGREE",
            Self::AnalysisCentralityCloseness => "GM_MUTATE_ANALYSIS_CENTRALITY_CLOSENESS",
            Self::AnalysisCentralityBetweenness => "GM_MUTATE_ANALYSIS_CENTRALITY_BETWEENNESS",
            Self::AnalysisCentralityEigenvector => "GM_MUTATE_ANALYSIS_CENTRALITY_EIGENVECTOR",
            Self::AnalysisDepthBfs => "GM_MUTATE_ANALYSIS_DEPTH_BFS",
            Self::PostBundleFdeb => "GM_MUTATE_POST_BUNDLE_FDEB",
            Self::PostBundleMingle => "GM_MUTATE_POST_BUNDLE_MINGLE",
            Self::PostRouteGrid => "GM_MUTATE_POST_ROUTE_GRID",
            Self::PostStyleStraight => "GM_MUTATE_POST_STYLE_STRAIGHT",
            Self::PostStyleOrthogonal => "GM_MUTATE_POST_STYLE_ORTHOGONAL",
            Self::PostStyleQuadratic => "GM_MUTATE_POST_STYLE_QUADRATIC",
            Self::PostStyleBezier => "GM_MUTATE_POST_STYLE_BEZIER",
            Self::SplitSum => "GM_MUTATE_SPLIT_SUM",
        }
    }

    /// The record its run writes.
    pub const fn record(self) -> &'static str {
        match self {
            Self::ReferenceDegree => "hashgate-control-reference-degree",
            Self::GridSpacing => "hashgate-control-grid-spacing",
            Self::SugiyamaLayerSpacing => "hashgate-control-sugiyama-layer-spacing",
            Self::NodeCount => "hashgate-control-node-count",
            Self::ForceTheta => "hashgate-control-force-theta",
            Self::Fa2ScalingRatio => "hashgate-control-fa2-scaling-ratio",
            Self::TreeTidyNodes => "hashgate-control-tree-tidy-nodes",
            Self::TreemapNodes => "hashgate-control-treemap-nodes",
            Self::CircularNodes => "hashgate-control-circular-nodes",
            Self::PackingScale => "hashgate-control-packing-scale",
            Self::AnalysisComponentsWeak => "hashgate-control-analysis-components-weak",
            Self::AnalysisComponentsStrong => "hashgate-control-analysis-components-strong",
            Self::AnalysisCommunitiesLouvain => "hashgate-control-analysis-communities-louvain",
            Self::AnalysisCentralityDegree => "hashgate-control-analysis-centrality-degree",
            Self::AnalysisCentralityCloseness => "hashgate-control-analysis-centrality-closeness",
            Self::AnalysisCentralityBetweenness => {
                "hashgate-control-analysis-centrality-betweenness"
            }
            Self::AnalysisCentralityEigenvector => {
                "hashgate-control-analysis-centrality-eigenvector"
            }
            Self::AnalysisDepthBfs => "hashgate-control-analysis-depth-bfs",
            Self::PostBundleFdeb => "hashgate-control-post-bundle-fdeb",
            Self::PostBundleMingle => "hashgate-control-post-bundle-mingle",
            Self::PostRouteGrid => "hashgate-control-post-route-grid",
            Self::PostStyleStraight => "hashgate-control-post-style-straight",
            Self::PostStyleOrthogonal => "hashgate-control-post-style-orthogonal",
            Self::PostStyleQuadratic => "hashgate-control-post-style-quadratic",
            Self::PostStyleBezier => "hashgate-control-post-style-bezier",
            Self::SplitSum => "hashgate-control-split-sum",
        }
    }
}

/// The [`knobs::Stage`] `knob` perturbs — a function of its *variable*, not its arm index.
///
/// Resolved by matching the variable name against the one table, so a control cannot be
/// filed under a stage the table does not agree with: a variable the table does not carry
/// is a programming error, not a runtime setting, and it panics here rather than quietly
/// perturbing whichever stage happened to sit at that arm's position.
pub(super) fn stage_of(knob: Knob) -> knobs::Stage {
    let env = knob.env();
    knobs::ANALYSIS_POST_STAGES
        .iter()
        .copied()
        .find(|row| row.env == env)
        .unwrap_or_else(|| panic!("{env} is one of the fifteen ANALYSIS and POST controls"))
}
