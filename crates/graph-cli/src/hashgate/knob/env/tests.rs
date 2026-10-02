//! The `GM_MUTATE_*` typo sweep (RG-26): the one check that turns a misspelled variable
//! into a loud exit 2 instead of a silently unperturbed run.

use super::*;
use crate::hashgate::Knob;

/// The names the sweep must refuse: a typo'd spelling, a renamed knob, a lower-cased name,
/// and the empty suffix a shell leaves behind (`GM_MUTATE_POST_STYLE_=`).
const TYPOS: [&str; 4] = [
    "GM_MUTATE_POST_STYLE_",
    "gm_mutate_post_style_bezier",
    "GM_MUTATE_REFERENCE_DEGREEE",
    "GM_MUTATE_",
];

#[test]
fn a_misspelled_control_variable_is_refused_by_name() {
    for name in TYPOS {
        let err = refuse_an_unknown_knob(&[OsString::from(name)]).expect_err("refused");
        assert!(
            err.contains(name),
            "the refusal must name the variable: {err}"
        );
        assert!(err.contains(&Knob::ALL.len().to_string()), "{err}");
        assert!(err.contains("no negative control"), "{err}");
    }
}

/// The control the sweep exists to protect: the row and its perturbation sharing a typo ran
/// the unperturbed gate and exited 0.
#[test]
fn the_whole_control_list_is_accepted_and_nothing_else_is() {
    let known: Vec<OsString> = Knob::ALL
        .iter()
        .map(|knob| OsString::from(knob.env()))
        .collect();
    assert_eq!(refuse_an_unknown_knob(&known), Ok(()));
    // One real knob beside one typo: the typo is still the refusal, not the neighbour's
    // validity that hides it.
    let mut mixed = known;
    mixed.push(OsString::from("GM_MUTATE_NOPE"));
    assert!(refuse_an_unknown_knob(&mixed).is_err());
}

/// A variable that is not a control at all is none of this module's business: `PATH`,
/// `HOME` and every other ambient name must pass untouched, or the sweep would refuse every
/// gate on a developer's machine.
#[test]
fn a_variable_that_is_not_a_control_is_left_alone() {
    for name in [
        "PATH",
        "HOME",
        "GM_GATES_DIR",
        "CARGO",
        "GM_MUTATE",
        "MUTATE_X",
    ] {
        assert_eq!(
            refuse_an_unknown_knob(&[OsString::from(name)]),
            Ok(()),
            "{name}"
        );
    }
    assert_eq!(refuse_an_unknown_knob(&[]), Ok(()));
}

/// **A lower-cased spelling is the same typo.** The knob loop reads `GM_MUTATE_*` by exact
/// name, so `gm_mutate_post_style_bezier` perturbs nothing — which is why the review named it
/// as an input. A case-sensitive prefix check would have left precisely that one uncaught.
#[test]
fn a_lower_cased_control_variable_is_refused_too() {
    let err = refuse_an_unknown_knob(&[OsString::from("gm_mutate_post_style_bezier")])
        .expect_err("same typo, different case");
    assert!(err.contains("gm_mutate_post_style_bezier"), "{err}");
    assert!(err.contains("no negative control"), "{err}");
}

/// The sweep reads the **whole** environment, so it has to see past the names it knows: the
/// `setting` loop over `Knob::ALL` can only ever look at the ones it lists.
#[test]
fn the_control_names_are_the_process_environments_own() {
    // Whatever this process has set, the filter keeps exactly the `GM_MUTATE_*` prefix.
    let names = control_names();
    for name in &names {
        assert!(name.to_string_lossy().starts_with(PREFIX), "{name:?}");
    }
    // A name the control list does hold is never swept, whatever else is in the list.
    assert!(!refuse_an_unknown_knob(&names).is_err_and(|err| !err.contains("PATH")));
}
