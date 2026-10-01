//! The negative controls' knobs and the setting the native arm runs with: every
//! variable is read strictly and at most one may be set.

use super::knobs;

pub(super) mod arms;
pub(super) mod igraph;
pub(super) mod records;
pub(crate) mod setting;
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
/// analysis, no POST capability and none of the six igraph-family layouts takes a
/// parameter the gate can move, being a pure function of the gate's model at fixed
/// conventions, so each re-draws *its own* model with one more node through
/// [`Setting::stage_nodes`] and moves that stage alone. They are tabulated in
/// [`knobs`] (the fifteen in [`knobs::ANALYSIS_POST_STAGES`], the six igraph layouts in
/// [`knobs::IGRAPH_LAYOUT_STAGES`]), and
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
    /// `GM_MUTATE_SPRING_ITERATIONS`: the spring layout's iteration budget, native arm
    /// only.
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
    /// [`knobs::IGRAPH_LAYOUT_STAGES`] order. One arm per layout, for the reason
    /// [`knobs::IGRAPH_LAYOUT_STAGES`] gives: none of the six takes a parameter the gate
    /// can move, so its control re-draws **its own** model with one more node. Each arm's
    /// variable and record are `igraph::ENV` and `igraph::RECORD` at the same index, and
    /// its stage is resolved from [`knobs`] by variable name, never by arm position.
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
    /// `GM_MUTATE_SPLIT_RESCALE`: **native arms only, and the threaded ones above all.**
    ///
    /// The closed-form point layouts' sibling of [`Knob::SplitSum`], and the control that
    /// makes the *other* half of Phase 11 provable: `coords`' shared merge. It makes the
    /// `rescale_layout` centroid merge read the **next** node's term into this node's — the
    /// shape a wrong partition of the outputs would take — so the threaded arms of
    /// `layout.grid`, `layout.circular.ring` and `layout.spiral` must diverge from the
    /// scalar one, and the arms of every other stage must not.
    ///
    /// **It reaches the merge, not the gather.** A knob that perturbed a layout's own
    /// arithmetic would move the *scalar* arm too and so would prove only that the stage is
    /// hashed; this one exists to prove the threaded arm **recomputed** the merge. A
    /// threaded arm that reused the scalar column would agree with a mutated one, and
    /// "10-way equal" would be a statement about nothing.
    ///
    /// A `bool` and not a `Split`, because there is exactly one merge to name — the three
    /// Barnes-Hut passes each needed their own variant so a row could prove a *particular*
    /// kernel was compared, and one merge cannot be told apart from itself. It is a
    /// compiled-in parameter, never a `cfg` and never an environment read, for
    /// [`Knob::SplitSum`]'s reason: graph-core reads no clock, no environment and no
    /// hardware, and the host supplies even the mutation.
    ///
    /// **The grid's own control answers a different question.**
    /// [`Knob::GridSpacing`] is a *pass* control: it moves a real parameter, so it moves
    /// the scalar arm and every threaded arm alike, and what it proves is that
    /// `layout.grid` is hashed and compared at all. This one is the *fail* control for the
    /// merge the three layouts share, and it moves only the threaded arms. Both are kept:
    /// one question each, neither standing in for the other.
    SplitRescale,
    /// `GM_MUTATE_FORCE_SESSION_GRAVITY`: the **live** force session's `gravity`, native arm
    /// of `force-gate` only.
    ///
    /// Its own control, and the only one that reaches the live session: no other variable in
    /// this list touches `LiveParams`, because `LiveParams` has no other user on this side —
    /// the frozen stage runs `from_frozen`, which is a parameter set `ForceParams` holds and
    /// [`Knob::ForceTheta`] already reaches through. `gravity` is on top of that the one
    /// parameter the **frozen** set does not have at all (`live_params.rs`), so a control that
    /// moves it cannot possibly move `layout.force.barnes_hut` and take another stage with it.
    ///
    /// It reaches the force gate rather than this gate: the wasm arm builds the seed's model
    /// from `gm_seed_ingest`, whose document is fixed, so the only perturbation a cross-target
    /// comparison here can see is one in the *parameters* — and this is that one. A non-zero
    /// value pulls every node toward the origin, which moves every position in the pair of
    /// columns the gate hashes, on every seed, from the first tick.
    ///
    /// Parsed, not treated as a flag: `=0` must be the honest run and a typo (`=maybe`) an
    /// error rather than a silent no-op — the same rule every other parameter knob obeys, for
    /// the same reason.
    ForceSessionGravity,
}

impl Knob {
    /// Every knob: the fifteen that move a parameter or re-draw one layout's model, then
    /// the fifteen ANALYSIS and POST stage controls in [`knobs::ANALYSIS_POST_STAGES`] order,
    /// then the six igraph layout controls in [`knobs::IGRAPH_LAYOUT_STAGES`] order, then the
    /// two compute-tier controls, then the live session's own. The list itself is
    /// [`arms::ALL`], spelled out there.
    ///
    /// **A `const`, because `capabilities::verdict::Evidence::load` walks it** to collect
    /// one control record each — a ledger read cannot be a function call per row. So the
    /// twenty-one per-stage arms are spelled out there and held against those two tables by
    /// `the_analysis_and_post_controls_are_the_knobs_table`, which fails on any arm whose
    /// variable, record or stage a table disagrees with.
    pub const ALL: [Self; 38] = arms::ALL;

    /// The variable that sets it.
    pub const fn env(self) -> &'static str {
        arms::env(self)
    }

    /// The record its run writes.
    pub const fn record(self) -> &'static str {
        records::record(self)
    }
}

pub(super) use arms::stage_of;
