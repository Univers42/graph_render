//! The negative controls' knobs and the setting the native arm runs with: every
//! variable is read strictly and at most one may be set.

use super::knobs;

pub(super) mod arms;
pub(super) mod compute;
pub(crate) mod env;
pub(super) mod igraph;
pub(super) mod records;
pub(crate) mod setting;
pub(super) mod three_d;
pub(super) mod value;
mod wiring;
pub(crate) use setting::{Setting, env_setting};

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
/// them one at a time. So every layout has a control filed under its own stage id, and the
/// three tests named `*_has_its_own_negative_control_that_moves_only_its_stage` are what
/// keep the per-stage ones honest.
///
/// **What each of the four perturbs, and why it is not one thing.** Circle packing is the
/// only one that publishes parameters ([`graph_core::layout::circle_packing::
/// CirclePackingParams`]), so [`Knob::PackingScale`] moves a real parameter of that
/// layout. The other three take none by design — `layout::tidy_tree`, `layout::treemap`
/// and `layout::circular` pin their own conventions — so their controls re-draw *that one
/// stage's* model with one more node instead ([`Knob::TreeTidyNodes`],
/// [`Knob::TreemapNodes`], [`Knob::CircularNodes`]). Same probe as node count, scoped to
/// one stage: it is the honest way to move a layout that has no parameter to move, and it
/// is what makes the divergence *name* the stage.
///
/// **p12-t2's two layouts split those cases one each.**
/// [`Knob::SpringIterations`] moves a real parameter, because
/// `graph_core::layout::force::spring::SpringParams` publishes one, and `iterations` is
/// read by the Fruchterman–Reingold loop and by nothing else.
/// [`Knob::CircularHierarchyNodes`] takes the other branch, because SciGraphs' closed form
/// takes no parameter at all — its `scale` is the dispatcher's own constant — so the only
/// thing a control can move is the graph it draws.
///
/// **The per-stage controls are the same probe again**, and for the same reason: no
/// analysis, no POST capability, none of the six igraph-family layouts and none of the five
/// natively 3D ones takes a parameter the gate can move, being a pure function of the
/// gate's model at fixed conventions, so each re-draws *its own* model with one more node
/// through [`Setting::stage_nodes`] and moves that stage alone. They are tabulated in
/// [`knobs`] (the fifteen in [`knobs::ANALYSIS_POST_STAGES`], the six igraph layouts in
/// [`knobs::IGRAPH_LAYOUT_STAGES`], the five 3D layouts in
/// [`knobs::THREE_D_LAYOUT_STAGES`]), and
/// `the_analysis_and_post_controls_are_the_knobs_table` holds this enum's arms to them.
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
    /// read by ForceAtlas2's repulsion alone, the dense pair loop and the Barnes-Hut
    /// tree walk alike, so it moves both ForceAtlas2 stages and no other.
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
    /// `GM_MUTATE_TWOPI_NODES`: nodes added to `layout.twopi`'s model alone.
    ///
    /// The same probe as the three Phase 3 node controls, for the same reason: `twopi` is a
    /// closed form with no parameter of its own — it pins Graphviz's defaults (`ranksep`
    /// 1 inch, `overlap` unset) and its module doc says so — so its own control perturbs
    /// the one thing it does read, the model, for that stage only. Adding a `Params` to
    /// gain a knob would be the tail wagging the dog.
    TwopiNodes,
    /// `GM_MUTATE_NEATO_EPSILON`: `layout.force.neato`'s stopping tolerance, native arm
    /// only.
    ///
    /// **A real parameter rather than a re-drawn model, unlike [`Self::TwopiNodes`],** and
    /// the difference is the point. `twopi` is closed form and pins every one of the
    /// reference's defaults, so the only thing left to perturb is the model. `neato` is
    /// iterative and its `Epsilon` is a *tolerance on convergence* (`stress.h:25`), so moving
    /// it changes how far the iteration runs and therefore the drawing, without touching the
    /// graph — which makes it a strictly sharper probe: the re-drawn-model controls would
    /// also move any stage whose output happens to depend on the node count, while this one
    /// reaches `layout.force.neato` and nothing else by construction.
    ///
    /// It reaches a *parameter* rather than a stage's model, and that is also why it is
    /// native-arm-only like every other parameter knob here: the wasm arm runs the stage at
    /// the registry's own defaults, so the divergence it shows is the one a wired control is
    /// supposed to surface. A typo (`=maybe`) is refused rather than read as the default, so
    /// the control cannot pass vacuously.
    NeatoEpsilon,
    /// `GM_MUTATE_PATCHWORK_NODES`: nodes added to `layout.treemap.patchwork`'s model
    /// alone.
    ///
    /// The same probe as the four node controls above, for the same reason: `patchwork` is a
    /// closed form with no parameter of its own — it pins Graphviz's default `area` of 1 and
    /// no `inset`, and its module doc says so — so its own control perturbs the one thing it
    /// does read, the model, for that stage only. A new node is a new square in the field,
    /// so it moves the tiling and this stage's bytes and nothing else's.
    PatchworkNodes,
    /// `GM_MUTATE_SPRING_ITERATIONS`: the spring layout's iteration budget, native arm only.
    ///
    /// Its own control because `iterations` is read by the FR loop's `for` and by
    /// nothing else: perturbing it re-runs this stage's force pass and leaves every other
    /// stage — including `layout.forceatlas2` and `layout.force.barnes_hut`, which share
    /// no code with it — byte-identical.
    SpringIterations,
    /// `GM_MUTATE_CIRCULAR_HIERARCHY_NODES`: nodes added to `layout.circular.hierarchy`'s
    /// model alone.
    ///
    /// The re-drawn-model probe, like [`Knob::CircularNodes`] next to it: the SciGraphs
    /// closed form takes no parameter (`SCALE` is the dispatcher's own default), so the
    /// one thing it does read is the graph, and one more node changes its component roots
    /// and every level count while nothing else in the gate moves.
    CircularHierarchyNodes,
    /// `GM_MUTATE_PACKING_SCALE`: the packing's `CirclePackingParams::scale`, native arm
    /// only.
    ///
    /// A real parameter rather than a re-drawn model, because circle packing is the one
    /// of the four that publishes `run_with`; the scale is read by the final centring and
    /// so changes every circle's centre and radius.
    PackingScale,
    /// `GM_MUTATE_LAYOUT_PARAM_DEFAULT`: which of a published layout's parameters has its
    /// **default** perturbed, native arm only (`docs/decisions/layout-params.md`).
    ///
    /// **The control for the schema itself.** Every other parameter knob moves a value
    /// the layout would have been given anyway; this one runs
    /// [`FruchtermanReingold::ID`](crate::layout::force::FruchtermanReingold)'s stage
    /// through `Capability::run_params` at a buffer whose value at one index is one more
    /// than the default the registry publishes, so the drawing is the published default
    /// plus one. The wasm arm cannot see it: it sends `params_len == 0`, which is the
    /// defaults, so the divergence this shows is exactly the one a wired control must
    /// surface — and it is a control over the ABI, not over the algorithm.
    ///
    /// **Why one stage and not all thirteen.** A control that perturbs nothing passes
    /// vacuously and one that perturbs everything names no stage, which is why every other
    /// knob here is filed under exactly one. This one is filed under the first layout in
    /// the registry that publishes anything, and the value is an index into *that* layout's
    /// list. An index past the end is refused rather than clamped, exactly as a node count
    /// of zero is.
    LayoutParamDefault,
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
    /// The six igraph-family layout node controls, in
    /// [`knobs::IGRAPH_LAYOUT_STAGES`] order — see `igraph` for the group and `knobs` for
    /// why each control re-draws **its own** model rather than a shared one.
    ///
    /// `layout.force.fruchterman_reingold`'s own model.
    IgraphFruchtermanReingoldNodes,
    /// `GM_MUTATE_FORCE_KAMADA_KAWAI_NODES`: `layout.force.kamada_kawai`'s own model.
    IgraphKamadaKawaiNodes,
    /// `GM_MUTATE_FORCE_GRAPHOPT_NODES`: `layout.force.graphopt`'s own model.
    IgraphGraphoptNodes,
    /// `GM_MUTATE_FORCE_DAVIDSON_HAREL_NODES`: `layout.force.davidson_harel`'s own model.
    IgraphDavidsonHarelNodes,
    /// `GM_MUTATE_FORCE_LGL_NODES`: `layout.force.lgl`'s own model.
    IgraphLglNodes,
    /// `GM_MUTATE_FORCE_DRL_NODES`: `layout.force.drl`'s own model.
    IgraphDrlNodes,
    /// The seven natively 3D layout node controls, in
    /// [`knobs::THREE_D_LAYOUT_STAGES`] order — the same shape and the same reason as the
    /// six above, and for `sphere`, `helix` and `cube` the *only* shape available: those
    /// three read the node count and no edge, so their model is their whole input.
    ///
    /// `layout.basic3d.sphere`'s own model.
    Basic3dSphereNodes,
    /// `GM_MUTATE_BASIC3D_HELIX_NODES`: `layout.basic3d.helix`'s own model.
    Basic3dHelixNodes,
    /// `GM_MUTATE_BASIC3D_CUBE_NODES`: `layout.basic3d.cube`'s own model.
    Basic3dCubeNodes,
    /// `GM_MUTATE_HIERARCHICAL3D_NODES`: `layout.hierarchical3d`'s own model.
    Hierarchical3dNodes,
    /// `GM_MUTATE_FORCE_SPRING3D_NODES`: `layout.force.spring3d`'s own model.
    ///
    /// A node control and not a second [`Self::SpringIterations`], because the iterations
    /// budget is read by the one `Solver::settle` both dimensions share
    /// (`force/spring.rs:175`, `force/spring3d.rs:45`) — so it moves *both* spring stages
    /// and names neither. This one moves `layout.force.spring3d` alone, which is what makes
    /// the divergence attributable.
    Spring3dNodes,
    /// `GM_MUTATE_BASIC3D_SPIRAL_NODES`: `layout.basic3d.spiral`'s own model.
    ///
    /// The re-drawn-model probe, and like `sphere`/`helix`/`cube` above it is the *only*
    /// shape available: the layout reads the node count and no edge
    /// (`layout/basic_3d.rs`'s module doc), so its model is its whole input. One more node
    /// moves `turns`, the `wanted` linspace and every coordinate after it, which is a
    /// sharper probe here than it is for `sphere`: the arc-length table is rebuilt per call,
    /// so a perturbed node count re-runs the whole 65 536-entry inversion.
    Basic3dSpiralNodes,
    /// `GM_MUTATE_BIPARTITE_3D_NODES`: `layout.bipartite_3d`'s own model.
    ///
    /// The same probe for the same reason, with one difference worth recording: this layout
    /// *does* read the graph (`registry.rs`'s append comment says so), so a re-drawn model
    /// moves it through its edges as well as its node count. That is still the right probe —
    /// it is scoped to this stage alone, which a shared control could not be.
    Bipartite3dNodes,
    /// `GM_MUTATE_PACKING_OSAGE_NODES`: `layout.packing.osage`'s own model.
    ///
    /// The re-drawn-model probe again, and for `osage` it is not merely the available one but
    /// the only one: the layout publishes no `Params` and has no `impl Stage`, and what it
    /// reads is the node count and no edge, so its model is its entire input. See
    /// [`knobs::OSAGE_LAYOUT_STAGES`] for why this row is what `layout.packing.osage` needs
    /// before the ledger can call the capability `gated` rather than `implemented`.
    PackingOsageNodes,
    /// `GM_MUTATE_SPLIT_SUM`: **native arms only, and the threaded ones above all.**
    ///
    /// Names which gathered pass's merge reads a neighbouring node's delta. The full argument
    /// is in [`compute`], under its own heading.
    SplitSum,
    /// `GM_MUTATE_SPLIT_RESCALE`: **native arms only, and the threaded ones above all.**
    ///
    /// Corrupts the closed-form point layouts' shared `coords` merge. The full argument is in
    /// [`compute`], under its own heading.
    SplitRescale,
    /// `GM_MUTATE_OVERLAP_RELAXATION`: the overlap pass's over-relaxation factor.
    ///
    /// **A real parameter, and the only knob that can make the overlap invariant go red.**
    /// Every other POST capability takes no parameters, so its control re-draws its own model
    /// (`stage_nodes`); `post::separate` publishes [`SeparateParams::over_relaxation`], so its
    /// control moves the real thing.
    ///
    /// The control's value is **`0`**, which is legal and is not clamped: it freezes every
    /// displacement, so the pass cannot separate anything and every input overlap survives into
    /// the snapshot. A native arm that freezes a stage the wasm arm runs normally is exactly
    /// the cross-target divergence the gate exists to catch, and it is the perturbation that
    /// turns `graph-cli overlap`'s invariant row red rather than merely moving a hash.
    ///
    /// A re-drawn model could not do this: adding a node changes the input, and the pass
    /// separates it correctly either way, so the invariant would stay green and the control
    /// would prove nothing about the pass's ability to separate at all.
    OverlapRelaxation,
    /// `GM_MUTATE_FORCE_SESSION_GRAVITY`: the **live** force session's `gravity`, native arm
    /// of `force-gate` only.
    ///
    /// The one control that reaches `force-gate` rather than this gate. The full argument is
    /// in [`compute`], under its own heading.
    ForceSessionGravity,
    /// `GM_MUTATE_DROP_DELTA`: the batch of the force gate's **stream** stage that the
    /// native arm skips outright — no `extend`, no `grow` — native arm of `force-gate` only.
    ///
    /// The control for the growth path rather than for the session's parameters: it drops
    /// input the wasm arm still receives, so the two sessions must diverge, and the gate has
    /// to name the first batch where they did. The full argument is in [`compute`], under its
    /// own heading.
    DropDelta,
}

pub(super) use arms::stage_of;
