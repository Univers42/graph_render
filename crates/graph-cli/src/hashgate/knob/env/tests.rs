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
        assert!(err.contains(name), "the refusal must name the variable: {err}");
        assert!(err.contains(&Knob::ALL.len().to_string()), "{err}");
        assert!(err.contains("no negative control"), "{err}");
    }
}

/// The control the sweep exists to protect: the row and its perturbation sharing a typo
/// ran the unperturbed gate and exited 0.
#[test]
fn the_whole_control_list_is_accepted_and_nothing_else_is() {
    let known: Vec<OsString> = Knob::ALL.iter().map(|knob| OsString::from(knob.env())).collect();
    assert_eq!(refuse_an_unknown_knob(&known), Ok(()));
    // One real knob beside one typo: the typo is still the refusal, not the neighbour's
    // validity that hides it.
    let mut mixed = known.clone();
    mixed.push(OsString::from("GM_MUTATE_NOPE"));
    assert!(refuse_an_unknown_knob(&mixed).is_err());
}

/// A variable that is not a control at all is none of this module's business: `PATH`,
/// `HOME` and every other ambient name must pass untouched, or the sweep would refuse every
/// gate on a developer's machine.
#[test]
fn a_variable_that_is_not_a_control_is_left_alone() {
    for name in ["PATH", "HOME", "GM_GATES_DIR", "CARGO", "GM_MUTATE", "MUTATE_X"] {
        assert_eq!(refuse_an_unknown_knob(&[OsString::from(name)]), Ok(()), "{name}");
    }
    assert_eq!(refuse_an_unknown_knob(&[]), Ok(()));
}

/// The sweep runs against the *whole* environment, so it has to read past the names it
/// knows: `setting`'s loop over `Knob::ALL` can only ever see the ones it lists.
#[test]
fn the_process_environment_is_where_the_names_come_from() {
    let env = Env::process();
    let names = KnobEnv::names(&env);
    assert!(
        names.iter().any(|name| name == "PATH"),
        "the process environment lists PATH, or the sweep reads nothing: {names:?}"
    );
    assert!(KnobEnv::read(&env, "PATH").is_ok());
    assert!(matches!(
        KnobEnv::read(&env, "GM_DEFINITELY_NOT_SET"),
        Err(VarError::NotPresent)
    ));
}
