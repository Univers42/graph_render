//! The negative controls' knobs and the setting the native arm runs with: every
//! variable is read strictly and at most one may be set.

use graph_core::layout::circle_packing::CirclePackingParams;
use graph_core::layout::force::ForceParams;
use graph_core::layout::forceatlas2::Fa2Params;
use graph_core::layout::{circular, tidy_tree, treemap};
use graph_core::{GridParams, REFERENCE_DEGREE, SugiyamaParams};
use std::env::VarError;

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
}

impl Knob {
    /// Every knob.
    pub const ALL: [Self; 10] = [
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
        }
    }
}

/// What the native arm runs with: the compiled-in defaults, or one knob's perturbation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Setting {
    pub(super) reference_degree: u32,
    pub(super) grid: GridParams,
    pub(super) sugiyama: SugiyamaParams,
    /// Extra nodes added to the gate's model, native arm only ([`Knob::NodeCount`]).
    pub(super) extra_nodes: u32,
    /// Barnes-Hut's parameters, native arm only ([`Knob::ForceTheta`] perturbs them).
    pub(super) force: ForceParams,
    /// ForceAtlas2's parameters, native arm only ([`Knob::Fa2ScalingRatio`] perturbs).
    pub(super) fa2: Fa2Params,
    /// Circle packing's parameters, native arm only ([`Knob::PackingScale`] perturbs).
    pub(super) packing: CirclePackingParams,
    /// The one Phase 3 stage whose own model a control re-draws, native arm only
    /// ([`Knob::TreeTidyNodes`], [`Knob::TreemapNodes`], [`Knob::CircularNodes`]).
    ///
    /// A stage id, never a node count: which stage the extra nodes are *for* is the whole
    /// claim, and a bare `u32` would let the same perturbation reach the shared model
    /// again — which is [`Setting::extra_nodes`], and moves every stage at once.
    pub(super) stage_nodes: Option<(&'static str, u32)>,
    pub(super) control: Option<Knob>,
}

/// Reads the knobs through `read`. At most one may be set, and a set one must parse:
/// a typo falling back to the default would let the control pass as green. A spacing the
/// grid refuses is left for the grid to refuse, so the rule lives in one place.
pub(super) fn setting(read: impl Fn(&str) -> Result<String, VarError>) -> Result<Setting, String> {
    let mut setting = Setting {
        reference_degree: REFERENCE_DEGREE,
        grid: GridParams::default(),
        sugiyama: SugiyamaParams::default(),
        extra_nodes: 0,
        force: ForceParams::default(),
        fa2: Fa2Params::default(),
        packing: CirclePackingParams::default(),
        stage_nodes: None,
        control: None,
    };
    for knob in Knob::ALL {
        let text = match read(knob.env()) {
            Err(VarError::NotPresent) => continue,
            Err(err) => return Err(format!("{}: {err}", knob.env())),
            Ok(text) => text,
        };
        if let Some(other) = setting.control {
            let (a, b) = (other.env(), knob.env());
            return Err(format!("{a} and {b} are both set: one control at a time"));
        }
        setting.control = Some(knob);
        apply(knob, text.trim(), &mut setting)?;
    }
    Ok(setting)
}

/// The one knob's perturbation, written into `setting`. Split out of [`setting`] by the
/// house's 40-line-per-function cap, and the place a new knob adds its single line: every
/// arm parses the *same* way, so a typo is refused whatever the knob perturbs.
fn apply(knob: Knob, text: &str, setting: &mut Setting) -> Result<(), String> {
    let bad = |e: &dyn std::fmt::Display| format!("{}={text:?}: {e}", knob.env());
    match knob {
        Knob::ReferenceDegree => {
            setting.reference_degree = text.parse().map_err(|e| bad(&e))?;
        }
        Knob::GridSpacing => setting.grid.spacing = text.parse().map_err(|e| bad(&e))?,
        Knob::SugiyamaLayerSpacing => {
            setting.sugiyama.layer_spacing = text.parse().map_err(|e| bad(&e))?;
        }
        Knob::NodeCount => setting.extra_nodes = text.parse().map_err(|e| bad(&e))?,
        Knob::ForceTheta => setting.force.theta = text.parse().map_err(|e| bad(&e))?,
        Knob::Fa2ScalingRatio => setting.fa2.scaling_ratio = text.parse().map_err(|e| bad(&e))?,
        Knob::TreeTidyNodes => {
            setting.stage_nodes = Some((tidy_tree::ID, nodes(text, knob)?));
        }
        Knob::TreemapNodes => {
            setting.stage_nodes = Some((treemap::ID, nodes(text, knob)?));
        }
        Knob::CircularNodes => {
            setting.stage_nodes = Some((circular::ID, nodes(text, knob)?));
        }
        Knob::PackingScale => setting.packing.scale = text.parse().map_err(|e| bad(&e))?,
    }
    Ok(())
}

/// Nodes added to one stage's own model. Zero is refused: a control that perturbs by
/// nothing passes vacuously, which is the one failure mode a negative control must not
/// have (`cli_force.rs`'s `--seeds 2` note is the same lesson at the other end of the
/// seed range).
fn nodes(text: &str, knob: Knob) -> Result<u32, String> {
    let count: u32 = text
        .parse()
        .map_err(|e| format!("{}={text:?}: {e}", knob.env()))?;
    if count == 0 {
        return Err(format!(
            "{}={text:?}: a control that adds no node perturbs nothing",
            knob.env()
        ));
    }
    Ok(count)
}

pub(super) fn env_setting() -> Result<Setting, String> {
    setting(|name| std::env::var(name))
}
