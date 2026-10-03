//! The text of one knob's variable, parsed: a node count, a stopping tolerance, a split
//! pass or a flag. Split out of the parent by the house's 300-line limit.

use super::Knob;
use graph_core::layout::force::Split;
/// Nodes added to one stage's own model. Zero is refused: a control that perturbs by
/// nothing passes vacuously, which is the one failure mode a negative control must not
/// have (`cli_force.rs`'s `--seeds 2` note is the same lesson at the other end of the
/// seed range).
pub(super) fn nodes(text: &str, knob: Knob) -> Result<u32, String> {
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
pub(super) fn tolerance(text: &str, knob: Knob) -> Result<f64, String> {
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
pub(super) fn split(text: &str) -> Option<Split> {
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
pub(super) fn yes(text: &str) -> Option<bool> {
    match text.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}
