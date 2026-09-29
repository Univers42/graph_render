//! The negative controls' knobs and the setting the native arm runs with: every
//! variable is read strictly and at most one may be set.

use graph_core::layout::force::{ForceParams, Split};
use graph_core::layout::forceatlas2::Fa2Params;
use graph_core::{GridParams, REFERENCE_DEGREE, SugiyamaParams};
use std::env::VarError;

/// A negative control (`prompt.md` §7.2): a variable that perturbs the native arm only,
/// so a wired mutation surfaces as exactly the cross-target divergence the gate must
/// catch. The grid ignores weights, so the reference degree cannot reach `layout.grid`,
/// and the grid's spacing is what backs that stage. Treemap reads node weight, so the
/// reference degree backs it too. Tidy tree, circular and packing take no parameters
/// (`layout::tidy_tree`/`circular` are pinned with none, and adding one to gain a knob
/// would be the tail wagging the dog) and read only the topology, so [`Knob::NodeCount`]
/// perturbs that instead: one more node changes every stage that is a function of the
/// topology at all, backing every stage no other knob reaches. The layered drawing
/// ignores weights too; its layer spacing backs `layout.dag.sugiyama`.
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
    /// Every knob.
    pub const ALL: [Self; 7] = [
        Self::ReferenceDegree,
        Self::GridSpacing,
        Self::SugiyamaLayerSpacing,
        Self::NodeCount,
        Self::ForceTheta,
        Self::Fa2ScalingRatio,
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
            Self::SplitSum => "hashgate-control-split-sum",
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
    /// Which gathered pass's merge reads a neighbouring node's delta
    /// ([`Knob::SplitSum`]), the phase-11 control for the threaded arms.
    ///
    /// A [`Split`] and not a `bool` because the control names *which* kernel it corrupts,
    /// and each kernel needs its own row to be shown to be compared.
    pub(super) split_sum: Split,
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
        split_sum: Split::None,
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
        let bad = |e: &dyn std::fmt::Display| format!("{}={text:?}: {e}", knob.env());
        match knob {
            Knob::ReferenceDegree => {
                setting.reference_degree = text.trim().parse().map_err(|e| bad(&e))?;
            }
            Knob::GridSpacing => setting.grid.spacing = text.trim().parse().map_err(|e| bad(&e))?,
            Knob::SugiyamaLayerSpacing => {
                setting.sugiyama.layer_spacing = text.trim().parse().map_err(|e| bad(&e))?;
            }
            Knob::NodeCount => setting.extra_nodes = text.trim().parse().map_err(|e| bad(&e))?,
            Knob::ForceTheta => setting.force.theta = text.trim().parse().map_err(|e| bad(&e))?,
            Knob::Fa2ScalingRatio => {
                setting.fa2.scaling_ratio = text.trim().parse().map_err(|e| bad(&e))?
            }
            // Parsed rather than treated as a presence flag, so `GM_MUTATE_SPLIT_SUM=0` is
            // the honest run and a typo (`=maybe`) is an error instead of a silent
            // mutation. `1`/`0` are accepted beside `true`/`false` because a gate row
            // reads `GM_MUTATE_SPLIT_SUM=1`, and a control whose documented spelling did
            // not work would be a control nobody runs.
            Knob::SplitSum => setting.split_sum = split(&text).ok_or_else(|| bad(&text))?,
        }
    }
    Ok(setting)
}

pub(super) fn env_setting() -> Result<Setting, String> {
    setting(|name| std::env::var(name))
}

/// Which merge `GM_MUTATE_SPLIT_SUM` corrupts: a pass's own name, or `1`/`true` for all
/// three. An unknown word is `None`, and the caller turns that into the parse error — a
/// control whose spelling did not work would be a control nobody runs.
fn split(text: &str) -> Option<Split> {
    match text.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "all" => Some(Split::All),
        "0" | "false" | "none" => Some(Split::None),
        "charge" => Some(Split::Charge),
        "collide" => Some(Split::Collide),
        "link" => Some(Split::Link),
        _ => None,
    }
}
