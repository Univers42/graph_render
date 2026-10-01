//! What the native arm runs with: the compiled-in defaults, or one knob's perturbation.
//!
//! Split out of `knob.rs` by the house's 300-line limit — the enum above it grew to
//! twenty-six arms, and the setting is the other half of the same concern: the one place
//! a knob's parsed value becomes a behaviour.

use graph_core::layout::circle_packing::CirclePackingParams;
use graph_core::layout::force::{ForceParams, LiveParams, Split};
use graph_core::layout::forceatlas2::Fa2Params;
use graph_core::layout::radial::twopi;
use graph_core::layout::{circular, tidy_tree, treemap};
use graph_core::{GridParams, REFERENCE_DEGREE, SugiyamaParams};
use std::env::VarError;

use super::knobs;
use super::{Knob, stage_of};

/// What the native arm runs with: the compiled-in defaults, or one knob's perturbation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Setting {
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
    /// `layout.force.neato`'s stopping tolerance ([`Knob::NeatoEpsilon`]), native arm only.
    ///
    /// An `Option` and not an `f64` defaulting to the compiled-in `EPSILON`, because `0` is
    /// itself a legal tolerance — the reference's own `|| new_stress < Epsilon` clause makes
    /// a zero epsilon stop the first pass — and a flag that could not say "zero" would make
    /// the control's honest value inexpressible. `None` is the honest run. Reached through
    /// [`Setting::neato_epsilon`], beside the field.
    pub(in crate::hashgate) neato_epsilon: Option<f64>,
    /// The one stage whose own model a control re-draws, native arm only
    /// ([`Knob::TreeTidyNodes`], [`Knob::TreemapNodes`], [`Knob::CircularNodes`], the
    /// twenty-one per-stage controls in [`knobs`], and the six igraph layout controls).
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
    /// The live force session's `gravity` ([`Knob::ForceSessionGravity`]), the perturbation
    /// `force-gate`'s native arm runs at. `None` is the honest run.
    ///
    /// An `Option` rather than a `f64` defaulting to the compiled-in `0`, because `0` is itself
    /// a legal gravity and a flag that could not say "set to zero" would make the control's
    /// honest value inexpressible. Reach it through [`Setting::live_force_params`], which is the
    /// only reader and lives in this module with the field.
    pub(in crate::hashgate) live_gravity: Option<f64>,
    pub(in crate::hashgate) control: Option<Knob>,
}

impl Setting {
    /// The live force parameters `force-gate`'s native arm runs at: the frozen force set —
    /// which is `LiveParams::default()`, because the frozen layout *is* a default session
    /// (`docs/decisions/live-force-session.md`) — with `gravity` replaced when
    /// [`Knob::ForceSessionGravity`] is set.
    ///
    /// One reader, beside the field it reads, so the wasm arm's own implicit parameters (which
    /// are exactly these defaults, because it sends `params_len == 0`) and the native arm's
    /// cannot disagree about what the honest run *is*.
    pub(crate) fn live_force_params(&self) -> LiveParams {
        LiveParams {
            gravity: self.live_gravity.unwrap_or(LiveParams::default().gravity),
            ..LiveParams::default()
        }
    }

    /// The `epsilon` `layout.force.neato` runs at: the registry's own `EPSILON`, or
    /// [`Knob::NeatoEpsilon`]'s perturbation.
    ///
    /// One reader, beside the field it reads, so the native arm's parameters and the wasm
    /// arm's implicit ones (which are exactly the registry defaults, because it sends
    /// `params_len == 0`) cannot disagree about what the honest run *is*.
    pub(crate) fn neato_epsilon(&self) -> f64 {
        self.neato_epsilon
            .unwrap_or(graph_core::layout::graphviz::neato::EPSILON)
    }

    /// Which control this run is under, if any — readable from outside this module's tree,
    /// which is what lets `force-gate` refuse a control that cannot reach a session.
    ///
    /// An accessor rather than a widened field: eleven arms' worth of parsing writes that field,
    /// and a second reader is not something that should learn to reach for it.
    pub(crate) fn control(&self) -> Option<Knob> {
        self.control
    }
}

/// Reads the knobs through `read`. At most one may be set, and a set one must parse:
/// a typo falling back to the default would let the control pass as green. A spacing the
/// grid refuses is left for the grid to refuse, so the rule lives in one place.
pub(crate) fn setting(read: impl Fn(&str) -> Result<String, VarError>) -> Result<Setting, String> {
    let mut setting = Setting {
        reference_degree: REFERENCE_DEGREE,
        grid: GridParams::default(),
        sugiyama: SugiyamaParams::default(),
        extra_nodes: 0,
        force: ForceParams::default(),
        fa2: Fa2Params::default(),
        packing: CirclePackingParams::default(),
        neato_epsilon: None,
        stage_nodes: None,
        split_sum: Split::None,
        split_rescale: false,
        live_gravity: None,
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
        Knob::TwopiNodes => {
            setting.stage_nodes = Some((twopi::ID, nodes(text, knob)?));
        }
        // Parsed as a float, not as a presence flag, for the reason every parameter knob
        // here is: a value that failed to parse must be an error rather than a silent
        // fall-back to the default, or the control would pass vacuously. A *legal* epsilon
        // (`0`) is the honest run and is accepted.
        Knob::NeatoEpsilon => setting.neato_epsilon = Some(tolerance(text, knob)?),
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
        // Parsed like the other parameter knobs, and for the same reason: `=0` is a real
        // gravity (the force is skipped, which is the honest run) and a typo is an error, so a
        // control that failed to parse cannot pass vacuously as the default.
        Knob::ForceSessionGravity => {
            setting.live_gravity = Some(text.parse().map_err(|e| bad(&e))?);
        }
        // The twenty-one per-stage controls, the fifteen ANALYSIS and POST rows and the six
        // igraph layout rows, are one arm here: `stage_of` resolves the stage from the
        // variable the knob was dispatched by, and every one of them is the same shape — a
        // node count for one stage's own model. A layout that took a real parameter would
        // get its own arm above, as Barnes-Hut and ForceAtlas2 do.
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

/// A stopping tolerance, which must be a finite non-negative number.
///
/// **Zero is accepted and is not the same as unset.** The reference's own convergence test is
/// `change / old < Epsilon || stress < Epsilon` (`stress.c:1059-1066`), so a zero epsilon
/// stops the iteration on the *second* clause as soon as the stress is non-negative — a
/// legal, different drawing, and a control that could not express it would be a control whose
/// honest value is unreachable. A negative tolerance is refused instead: no pass can satisfy
/// it, so the run would take the whole budget and claim a result it never converged to.
fn tolerance(text: &str, knob: Knob) -> Result<f64, String> {
    let value: f64 = text
        .parse()
        .map_err(|e| format!("{}={text:?}: {e}", knob.env()))?;
    if !value.is_finite() || value < 0.0 {
        return Err(format!(
            "{}={text:?}: a stopping tolerance is a finite non-negative number",
            knob.env()
        ));
    }
    Ok(value)
}

pub(crate) fn env_setting() -> Result<Setting, String> {
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
