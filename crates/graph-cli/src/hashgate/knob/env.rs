//! The `GM_MUTATE_*` typo sweep (RG-26) — the one check that turns a misspelled variable
//! into a loud exit 2 instead of a silently unperturbed run.
//!
//! Split out of `setting.rs` by the house's 300-line limit, and because it is a rule about
//! *names* while everything beside it is a rule about *values*.

use std::ffi::OsString;

/// The prefix every negative control's variable carries. Not a knob — it is the shape they
/// share, which is what makes a misspelling of one recognisable without reading the list.
///
/// **Matched without regard to case**, because a lower-cased spelling is the same typo: the
/// knob loop reads `GM_MUTATE_*` by exact name, so `gm_mutate_post_style_bezier` perturbs
/// nothing exactly as `GM_MUTATE_POST_STYLE_BEZIER` does if that name were renamed. Matching
/// case-sensitively would leave the review's own example — a mistyped `gm_mutate_…` — as the
/// one unperturbed spelling nothing catches.
pub(crate) const PREFIX: &str = "GM_MUTATE_";

/// Whether `name` is trying to be a control variable at all.
fn is_control_name(name: &str) -> bool {
    name.len() >= PREFIX.len() && name[..PREFIX.len()].eq_ignore_ascii_case(PREFIX)
}

/// Every `GM_MUTATE_*` name in `names`, in the order the environment listed them.
///
/// `names` is read at call time rather than snapshotted into the reader, so the sweep sees
/// the environment as it is when the gate looks — not as it was when the reader was built.
pub(crate) fn control_names() -> Vec<OsString> {
    std::env::vars_os()
        .map(|(name, _)| name)
        .filter(|name| is_control_name(&name.to_string_lossy()))
        .collect()
}

/// **A `GM_MUTATE_*` variable that names no knob refuses the run** (RG-26), and the refusal
/// names the variable and the list it is not on.
///
/// The alternative — ignoring it — is a gate that reports PASS for a control nobody applied,
/// which is the one thing a negative control must never be. Only the 45 names in `Knob::ALL`
/// are ever read, so a misspelling or a rename used to be silently unperturbed: the control
/// row and its perturbation shared the typo, the gate hashed the honest bytes, and it exited
/// 0 having recorded itself as the exercised control. Every typo is now a loud refusal, and
/// the message says how many names are accepted so the reader does not go looking.
///
/// A name that is not valid UTF-8 is compared lossily: it cannot be one of the accepted
/// names (all of which are ASCII), so on the prefix alone it is still a refusal rather than a
/// pass. The refusal then echoes the name as it was spelled, so a reader can find it.
pub(crate) fn refuse_an_unknown_knob(names: &[OsString]) -> Result<(), String> {
    for name in names {
        let name = name.to_string_lossy();
        if !is_control_name(&name) || super::Knob::ALL.iter().any(|knob| knob.env() == name) {
            continue;
        }
        return Err(format!(
            "{name} is set but names no negative control: the accepted names are the {} the \
             control list holds (graph-cli's `Knob::ALL`), so a variable outside that list is \
             a typo that would leave this run unperturbed and record the control as exercised",
            super::Knob::ALL.len()
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "env/tests.rs"]
mod tests;
