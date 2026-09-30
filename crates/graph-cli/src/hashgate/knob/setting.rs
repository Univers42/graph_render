//! What the native arm runs with: the compiled-in defaults, or one knob's perturbation.
//!
//! Split out of `knob.rs` by the house's 300-line limit — the enum above it grew to
//! twenty-six arms, and the setting is the other half of the same concern: the one place
//! a knob's parsed value becomes a behaviour.

use graph_core::layout::circle_packing::CirclePackingParams;
use graph_core::layout::force::{ForceParams, Split};
use graph_core::layout::forceatlas2::Fa2Params;
use graph_core::layout::{circular, tidy_tree, treemap};
use graph_core::{GridParams, REFERENCE_DEGREE, SugiyamaParams};
use std::env::VarError;

use super::knobs;
use super::{Knob, stage_of};

/// What the native arm runs with: the compiled-in defaults, or one knob's perturbation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::hashgate) struct Setting {
    pub(in crate::hashgate) reference_degree: u32,
    pub(in crate::hashgate) grid: GridParams,
    pub(in crate::hashgate) sugiyama: SugiyamaParams,
    /// Extra nodes added to the gate's model, native arm only ([`Knob::NodeCount`]).
    pub(in crate::hashgate) extra_nodes: u32,
    /// Barnes-Hut's parameters, native arm only ([`Knob::ForceTheta`] perturbs them).
    pub(in crate::hashgate) force: ForceParams,
    /// ForceAtlas2's parameters, native arm only ([`Knob::Fa2ScalingRatio`] perturbs).
    pub(in crate::hashgate) fa2: Fa2Params,
    /// Circle packing's parameters, native arm only ([`Knob::PackingScale`] perturbs).
    pub(in crate::hashgate) packing: CirclePackingParams,
    /// The one stage whose own model a control re-draws, native arm only
    /// ([`Knob::TreeTidyNodes`], [`Knob::TreemapNodes`], [`Knob::CircularNodes`] and the
    /// fifteen ANALYSIS and POST controls).
    ///
    /// A stage id, never a node count: which stage the extra nodes are *for* is the whole
    /// claim, and a bare `u32` would let the same perturbation reach the shared model
    /// again — which is [`Setting::extra_nodes`], and moves every stage at once.
    pub(in crate::hashgate) stage_nodes: Option<(&'static str, u32)>,
    /// Which gathered pass's merge reads a neighbouring node's delta
    /// ([`Knob::SplitSum`]), the phase-11 control for the threaded arms.
    ///
    /// A [`Split`] and not a `bool` because the control names *which* kernel it corrupts,
    /// and each kernel needs its own row to be shown to be compared.
    pub(in crate::hashgate) split_sum: Split,
    /// Whether the closed-form point layouts' shared `coords` merge is split
    /// ([`Knob::SplitRescale`]), the compute-tier control for the non-force threaded arms.
    ///
    /// A `bool` because there is one merge to corrupt, against [`Setting::split_sum`]'s
    /// [`Split`] which names *which* of the three force passes it is.
    pub(in crate::hashgate) split_rescale: bool,
    pub(in crate::hashgate) control: Option<Knob>,
}

/// Reads the knobs through `read`. At most one may be set, and a set one must parse:
/// a typo falling back to the default would let the control pass as green. A spacing the
/// grid refuses is left for the grid to refuse, so the rule lives in one place.
pub(in crate::hashgate) fn setting(
    read: impl Fn(&str) -> Result<String, VarError>,
) -> Result<Setting, String> {
    let mut setting = Setting {
        reference_degree: REFERENCE_DEGREE,
        grid: GridParams::default(),
        sugiyama: SugiyamaParams::default(),
        extra_nodes: 0,
        force: ForceParams::default(),
        fa2: Fa2Params::default(),
        packing: CirclePackingParams::default(),
        stage_nodes: None,
        split_sum: Split::None,
        split_rescale: false,
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
        // Parsed rather than treated as a presence flag, so `GM_MUTATE_SPLIT_SUM=0` is
        // the honest run and a typo (`=maybe`) is an error instead of a silent
        // mutation. `1`/`0` are accepted beside `true`/`false` because a gate row
        // reads `GM_MUTATE_SPLIT_SUM=1`.
        Knob::SplitSum => setting.split_sum = split(text).ok_or_else(|| bad(&text))?,
        // Parsed the same way, for the same reason: `=0` must be the honest run and a typo
        // (`=maybe`) an error rather than a silent mutation.
        Knob::SplitRescale => {
            setting.split_rescale = yes(text).ok_or_else(|| bad(&text))?;
        }
        _ => knobs::apply(stage_of(knob), nodes(text, knob)?, setting),
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

pub(in crate::hashgate) fn env_setting() -> Result<Setting, String> {
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

/// `GM_MUTATE_SPLIT_RESCALE`'s value: a flag, and `None` for anything else so the caller
/// turns it into the parse error. Both a split control and its honest setting go through
/// this one spelling, so the two cannot drift on what counts as "on".
fn yes(text: &str) -> Option<bool> {
    match text.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}
