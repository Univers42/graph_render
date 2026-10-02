//! What the native arm runs with: the compiled-in defaults, or one knob's perturbation.
//!
//! Split out of `knob.rs` by the house's 300-line limit — the enum above it grew to
//! twenty-six arms, and the setting is the other half of the same concern: the one place
//! a knob's parsed value becomes a behaviour.
//!
//! **Two rules turn a parsed value into a control**, and both are here rather than in each
//! arm, so a new arm cannot route around them:
//!
//! - a value that *is* the honest run's own value is refused, not accepted (RG-42). Three
//!   knobs could spell it — `GM_MUTATE_SPLIT_SUM=0`, `GM_MUTATE_SPLIT_RESCALE=0`,
//!   `GM_MUTATE_FORCE_SESSION_GRAVITY=0` — and each of them still recorded this run as the
//!   exercised control, so a row could write its own record having perturbed zero bytes.
//!   [`Setting::bites`] is the one comparison, against the compiled-in defaults.
//! - a `GM_MUTATE_*` variable that names no knob at all is refused (RG-26), in
//!   [`env::refuse_an_unknown_knob`], before any value is read.

use graph_core::layout::circle_packing::CirclePackingParams;
use graph_core::layout::force::spring::SpringParams;
use graph_core::layout::force::{ForceParams, LiveParams, Split};
use graph_core::layout::forceatlas2::Fa2Params;
use graph_core::layout::graphviz::{neato, patchwork};
use graph_core::layout::radial::twopi;
use graph_core::layout::{circular, tidy_tree, treemap};
use graph_core::{GridParams, REFERENCE_DEGREE, SugiyamaParams};
use std::env::VarError;
use std::ffi::OsString;

use super::env;
use super::knobs;
use super::value;
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
    /// Spring's parameters, native arm only ([`Knob::SpringIterations`] perturbs).
    pub(in crate::hashgate) spring: SpringParams,
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
    /// honest value unreachable. Reach it through [`Setting::live_force_params`], which is the
    /// only reader and lives in this module with the field.
    pub(in crate::hashgate) live_gravity: Option<f64>,
    pub(in crate::hashgate) control: Option<Knob>,
}

impl Setting {
    /// The compiled-in defaults: the honest run, with no knob set.
    ///
    /// Named, and not a `Default` impl, because "the honest run" is the value
    /// [`Setting::bites`] compares every control against — it is a second definition of what
    /// the gate runs when nothing perturbs it, and there is one of them.
    pub(crate) fn compiled_in() -> Setting {
        Setting {
            reference_degree: REFERENCE_DEGREE,
            grid: GridParams::default(),
            sugiyama: SugiyamaParams::default(),
            extra_nodes: 0,
            force: ForceParams::default(),
            fa2: Fa2Params::default(),
            spring: SpringParams::default(),
            packing: CirclePackingParams::default(),
            neato_epsilon: None,
            stage_nodes: None,
            split_sum: Split::None,
            split_rescale: false,
            live_gravity: None,
            control: None,
        }
    }

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

    /// **Whether this run perturbs anything at all** (RG-42) — one comparison against the
    /// compiled-in defaults, so every knob is covered by the rule rather than by an arm
    /// remembering to apply it.
    ///
    /// `control` is left out: it names *which* knob was set, not what the run computes. The
    /// two `Option` fields are normalised through their accessors first, because `neato`'s
    /// `EPSILON` and the live session's `0` gravity are the honest values spelled out
    /// explicitly — a flag that could not say "the default" would make those two controls
    /// inexpressible, and a field comparison alone would call them perturbations.
    pub(crate) fn bites(&self) -> bool {
        self.normalised() != Self::compiled_in().normalised()
    }

    /// `self` with every field that is only an `Option` *because* it has to be able to say
    /// "unset" collapsed to unset, and `control` cleared.
    fn normalised(self) -> Setting {
        let mut out = self;
        if out.neato_epsilon() == neato::EPSILON {
            out.neato_epsilon = None;
        }
        if out.live_force_params().gravity == LiveParams::default().gravity {
            out.live_gravity = None;
        }
        out.control = None;
        out
    }
}

/// Reads the knobs through `env`. At most one may be set, and a set one must parse *and
/// perturb*: a typo falling back to the default, a variable naming no knob at all, or a knob
/// carrying the honest run's own value would all let the control pass as green (RG-26,
/// RG-42). A spacing the grid refuses is left for the grid to refuse, so the rule lives in
/// one place.
#[cfg(test)]
pub(crate) fn setting(read: impl Fn(&str) -> Result<String, VarError>) -> Result<Setting, String> {
    setting_named(read, Vec::new)
}

/// [`setting`], with the variable **names** this run was handed alongside its values.
///
/// The names are what the `GM_MUTATE_*` sweep reads (RG-26), and they are a second argument
/// rather than part of the reader because the reader is a `Fn(&str)` every existing test
/// seam already builds — `forcecheck`'s own included — and widening that is a rewrite of
/// files this repair does not own. A caller that passes no name list runs **unswept**:
/// [`setting`] is that caller, and it exists for the unit tests. The one production reader is
/// [`env_setting`], which passes the process environment.
pub(crate) fn setting_named(
    read: impl Fn(&str) -> Result<String, VarError>,
    names: impl Fn() -> Vec<OsString>,
) -> Result<Setting, String> {
    env::refuse_an_unknown_knob(&names())?;
    let mut setting = Setting::compiled_in();
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
    refuse_a_no_op(&setting)?;
    Ok(setting)
}

/// **A control that perturbs nothing refuses the run** (RG-42): the parsed value is the
/// honest run's own, so the run would hash exactly the honest bytes and write this knob's
/// evidence record claiming the control had been exercised.
///
/// The message names the variable and the range it accepts, because the value the caller
/// typed is *in* the range — refusing a legal value has to say what to type instead.
fn refuse_a_no_op(setting: &Setting) -> Result<(), String> {
    let Some(knob) = setting.control else {
        return Ok(());
    };
    if setting.bites() {
        return Ok(());
    }
    Err(format!(
        "{} carries the honest run's own value, so it perturbs nothing while recording this \
         run as the exercised control; accepted range: {}",
        knob.env(),
        value::accepted(knob)
    ))
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
        // The one *global* count, and the one that was parsed outside [`value::nodes`] — so
        // `=0` was accepted, the run hashed exactly the honest bytes, and the gate exited 0
        // having recorded itself as the exercised control (RG-01). Every *per-stage* count
        // already refused zero; this one goes through the same parser as all of them.
        Knob::NodeCount => setting.extra_nodes = value::nodes(text, knob)?,
        Knob::ForceTheta => setting.force.theta = text.parse().map_err(|e| bad(&e))?,
        Knob::Fa2ScalingRatio => setting.fa2.scaling_ratio = text.parse().map_err(|e| bad(&e))?,
        Knob::TreeTidyNodes => {
            setting.stage_nodes = Some((tidy_tree::ID, value::nodes(text, knob)?));
        }
        Knob::TreemapNodes => {
            setting.stage_nodes = Some((treemap::ID, value::nodes(text, knob)?));
        }
        Knob::CircularNodes => {
            setting.stage_nodes = Some((circular::ID, value::nodes(text, knob)?));
        }
        Knob::TwopiNodes => {
            setting.stage_nodes = Some((twopi::ID, value::nodes(text, knob)?));
        }
        // Parsed as a float, not as a presence flag, for the reason every parameter knob
        // here is: a value that failed to parse must be an error rather than a silent
        // fall-back to the default, or the control would pass vacuously. A *legal* epsilon
        // (`0`) is the honest run and is accepted — and refused by [`refuse_a_no_op`] only if
        // it happens to equal the compiled-in `EPSILON`, which it does not.
        Knob::NeatoEpsilon => setting.neato_epsilon = Some(value::tolerance(text, knob)?),
        Knob::PatchworkNodes => {
            setting.stage_nodes = Some((patchwork::ID, value::nodes(text, knob)?));
        }
        // Parsed, not treated as a presence flag, and `0` is refused by [`refuse_a_no_op`]
        // like every other honest value: an iteration budget of zero would still return the
        // rescaled start field, which *is* a different drawing, so the parser takes it and
        // the one no-op rule decides. `iterations` is a `u32`, so a negative value is a parse
        // error rather than a silent wrap.
        Knob::SpringIterations => {
            setting.spring.iterations = text.parse().map_err(|e| bad(&e))?;
        }
        Knob::CircularHierarchyNodes => {
            setting.stage_nodes = Some((circular::hierarchy::ID, value::nodes(text, knob)?));
        }
        Knob::PackingScale => setting.packing.scale = text.parse().map_err(|e| bad(&e))?,
        // Parsed rather than treated as a presence flag, so `GM_MUTATE_SPLIT_SUM=0` reaches
        // [`refuse_a_no_op`] as a parse and is refused there for perturbing nothing, and a
        // typo (`=maybe`) is an error instead of a silent mutation. `1`/`0` are accepted
        // beside `true`/`false` because a gate row reads `GM_MUTATE_SPLIT_SUM=1`.
        Knob::SplitSum => {
            setting.split_sum = value::split(text).ok_or_else(|| bad(&text))?;
        }
        // Parsed the same way, for the same reason.
        Knob::SplitRescale => {
            setting.split_rescale = value::yes(text).ok_or_else(|| bad(&text))?;
        }
        // Parsed like the other parameter knobs: `=0` is a real gravity (the force is
        // skipped, which *is* the honest run) and so is refused by [`refuse_a_no_op`] as the
        // one thing it is, while a typo is an error at the parse.
        Knob::ForceSessionGravity => {
            setting.live_gravity = Some(text.parse().map_err(|e| bad(&e))?);
        }
        // The twenty-one per-stage controls, the fifteen ANALYSIS and POST rows and the six
        // igraph layout rows, are one arm here: `stage_of` resolves the stage from the
        // variable the knob was dispatched by, and every one of them is the same shape — a
        // node count for one stage's own model. A layout that took a real parameter would
        // get its own arm above, as Barnes-Hut and ForceAtlas2 do.
        _ => knobs::apply(stage_of(knob)?, value::nodes(text, knob)?, setting)?,
    }
    Ok(())
}

/// The production reader: this process's environment, values *and* names, so the run is
/// swept for a `GM_MUTATE_*` variable that names no control.
pub(crate) fn env_setting() -> Result<Setting, String> {
    setting_named(|name| std::env::var(name), env::control_names)
}

#[cfg(test)]
#[path = "setting/tests.rs"]
mod tests;
