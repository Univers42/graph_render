//! One knob's value, parsed: the shared parsers every arm goes through, and the two rules
//! that make a parsed value a *control* rather than a setting.
//!
//! Split out of `setting.rs` by the house's 300-line limit. The parsers live here so
//! `setting.rs` is the one place a knob's value becomes a behaviour, and the rules live
//! here so a new arm cannot route around them.

use super::Knob;
use graph_core::layout::force::Split;

/// Nodes added to one stage's own model — or to the gate's, for [`Knob::NodeCount`].
///
/// **Zero is refused**: a control that perturbs by nothing passes vacuously, which is the
/// one failure mode a negative control must not have (`cli_force.rs`'s `--seeds 2` note is
/// the same lesson at the other end of the seed range). `Knob::NodeCount` is parsed here
/// too, which it was not until RG-01: it is the one global count, and it was the one count
/// a row could set to zero, leaving the gate to measure exactly the honest run and exit 0.
pub(crate) fn nodes(text: &str, knob: Knob) -> Result<u32, String> {
    let count: u32 = text
        .parse()
        .map_err(|e| format!("{}={text:?}: {e}", knob.env()))?;
    if count == 0 {
        return Err(format!(
            "{}={text:?}: a control that adds no node perturbs nothing; accepted range 1..={}",
            knob.env(),
            u32::MAX
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
pub(crate) fn tolerance(text: &str, knob: Knob) -> Result<f64, String> {
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

/// Which merge `GM_MUTATE_SPLIT_SUM` corrupts: a pass's own name, or `1`/`true` for all
/// three. An unknown word is `None`, and the caller turns that into the parse error — a
/// control whose spelling did not work would be a control nobody runs.
///
/// **`0`/`false`/`none` parse, and are refused one layer up.** They parse because they are
/// how the honest value is *spelled*, and they are refused because writing the honest value
/// into a variable that then records this run as the control is the vacuous control RG-42
/// is about — `setting::Setting::bites` is what turns the parse into the refusal.
pub(crate) fn split(text: &str) -> Option<Split> {
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
pub(crate) fn yes(text: &str) -> Option<bool> {
    match text.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// The range `knob` accepts, for the message a refused value carries (RG-42).
///
/// Named here rather than spelled into each refusal so a knob's accepted values have one
/// home: the same one its parser reads.
pub(crate) fn accepted(knob: Knob) -> &'static str {
    match knob {
        Knob::NodeCount => "1..=4294967295 extra nodes on the gate's one model",
        Knob::SplitSum => "1, true, all, charge, collide or link",
        Knob::SplitRescale => "1, true, yes or on",
        Knob::NeatoEpsilon => "a finite non-negative tolerance, and not the compiled-in one",
        Knob::SpringIterations | Knob::ReferenceDegree => "a u32 that is not the compiled-in one",
        Knob::LayoutParamDefault => "an index into the stage's published parameters",
        _ => "a finite number the layout accepts, and not the compiled-in one",
    }
}
