//! The two rules that make a parsed knob value a *control*: it must perturb something, and
//! the variable that carries it must name a knob at all.
//!
//! Each test here is the negative control for one review finding, and each fails on the code
//! as it stood before it: `no_op_controls_are_refused` on `GM_MUTATE_SPLIT_SUM=0`, which
//! wrote the control's own record having hashed the honest bytes; `a_misspelled_variable…`
//! on `GM_MUTATE_POST_STYLE_=1`, which was silently ignored.

use super::*;
use crate::hashgate::Knob;

/// **RG-42, the report case: a knob carrying the honest run's own value.** The three the
/// review named — `SPLIT_SUM=0`, `SPLIT_SUM=false`, `SPLIT_SUM=none`, `SPLIT_RESCALE=0`,
/// `FORCE_SESSION_GRAVITY=0` — each parsed, each set `control`, and each left every byte the
/// honest run produces. Each is now refused, and the refusal names the variable and the range.
#[test]
fn no_op_controls_are_refused() {
    let no_ops: [(&str, &str); 6] = [
        ("GM_MUTATE_SPLIT_SUM", "0"),
        ("GM_MUTATE_SPLIT_SUM", "false"),
        ("GM_MUTATE_SPLIT_SUM", "none"),
        ("GM_MUTATE_SPLIT_RESCALE", "0"),
        ("GM_MUTATE_SPLIT_RESCALE", "off"),
        ("GM_MUTATE_FORCE_SESSION_GRAVITY", "0"),
    ];
    for (name, value) in no_ops {
        let err = setting(Env::list(vec![(name, value)])).expect_err("perturbs nothing");
        assert!(err.starts_with(name), "the refusal must name the argument: {err}");
        assert!(err.contains("perturbs nothing"), "{err}");
        assert!(err.contains("accepted range"), "and the range: {err}");
    }
}

/// **RG-01, the report case: `GM_MUTATE_NODE_COUNT=0`.** The one *global* count was parsed
/// with `text.parse()` and never went through the parser every per-stage count goes through,
/// so it accepted zero, hashed the honest bytes, and exited 0 having recorded itself as the
/// exercised control. Every per-stage count already refused it, which is what made this one a
/// hole rather than a policy.
#[test]
fn a_zero_node_count_is_refused_like_every_per_stage_count() {
    let err = setting(Env::list(vec![("GM_MUTATE_NODE_COUNT", "0")]))
        .expect_err("zero extra nodes perturb nothing");
    assert!(err.starts_with("GM_MUTATE_NODE_COUNT"), "{err}");
    assert!(err.contains("perturbs nothing"), "{err}");
    // The same message the per-stage counts give, from the same parser.
    let stage = setting(Env::list(vec![("GM_MUTATE_TREE_TIDY_NODES", "0")]))
        .expect_err("zero extra nodes perturb nothing");
    assert!(stage.starts_with("GM_MUTATE_TREE_TIDY_NODES"), "{stage}");
    assert!(
        err.split(" a control").nth(1) == stage.split(" a control").nth(1),
        "one parser, one message: {err:?} vs {stage:?}"
    );
    // One is accepted, and it moves the shared model.
    let one = setting(Env::list(vec![("GM_MUTATE_NODE_COUNT", "1")])).expect("one more node");
    assert_eq!(one.control, Some(Knob::NodeCount));
    assert_eq!(one.extra_nodes, 1);
}

/// **RG-26, the report case: `GM_MUTATE_POST_STYLE_=1`.** A misspelled variable matched no
/// knob, so it was ignored, the gate ran the unperturbed arm, and it exited 0 — a row that
/// believed its control had run.
#[test]
fn a_misspelled_variable_is_refused_rather_than_ignored() {
    let err = setting(Env::list(vec![("GM_MUTATE_POST_STYLE_", "1")]))
        .expect_err("names no knob");
    assert!(err.contains("GM_MUTATE_POST_STYLE_"), "{err}");
    assert!(err.contains("no negative control"), "{err}");
    // A well-spelled neighbour does not rescue the typo beside it.
    let mixed = setting(Env::list(vec![
        ("GM_MUTATE_GRID_SPACING", "2"),
        ("GM_MUTATE_NEATO_EPSILON_", "1e-2"),
    ]))
    .expect_err("the typo is the refusal");
    assert!(mixed.contains("GM_MUTATE_NEATO_EPSILON_"), "{mixed}");
}

/// The two rules must not cost the gate a legal value: every value the shipped rows set is
/// a perturbation and still parses.
#[test]
fn every_value_a_shipped_row_sets_still_parses() {
    let rows: [(&str, &str); 11] = [
        ("GM_MUTATE_REFERENCE_DEGREE", "9"),
        ("GM_MUTATE_GRID_SPACING", "2"),
        ("GM_MUTATE_SUGIYAMA_LAYER_SPACING", "2"),
        ("GM_MUTATE_NODE_COUNT", "1"),
        ("GM_MUTATE_FORCE_THETA", "0.5"),
        ("GM_MUTATE_FA2_SCALING_RATIO", "3"),
        ("GM_MUTATE_NEATO_EPSILON", "1e-2"),
        ("GM_MUTATE_SPRING_ITERATIONS", "3"),
        ("GM_MUTATE_PACKING_SCALE", "2"),
        ("GM_MUTATE_SPLIT_SUM", "collide"),
        ("GM_MUTATE_SPLIT_RESCALE", "1"),
    ];
    for (name, value) in rows {
        let setting = setting(Env::list(vec![(name, value)])).unwrap_or_else(|e| panic!("{name}={value}: {e}"));
        assert!(setting.bites(), "{name}={value} must perturb the run");
        assert_eq!(setting.control.map(Knob::env), Some(name));
    }
    // The live session's gravity is read by `force-gate`, and 0.5 is not its default of 0.
    let gravity = setting(Env::list(vec![("GM_MUTATE_FORCE_SESSION_GRAVITY", "0.5")]))
        .expect("a real gravity");
    assert_eq!(gravity.live_force_params().gravity, 0.5);
}

/// **The honest value spelled out is still the honest run.** `neato`'s `EPSILON` and the
/// live session's `0` gravity are what those knobs *are*, so `=EPSILON` and `=0` perturb
/// nothing — and the reason [`Setting::normalised`] reads them through their accessors
/// instead of comparing the raw `Option`s.
#[test]
fn an_option_field_carrying_its_default_is_still_a_no_op() {
    assert!(!setting(Env::list(vec![("GM_MUTATE_NEATO_EPSILON", "1e-4")]))
        .is_ok_and(|parsed| parsed.bites()));
    assert!(!setting(Env::list(vec![("GM_MUTATE_NEATO_EPSILON", "0")]))
        .is_ok_and(|parsed| parsed.bites()), "0 is a legal tolerance, but the default is not 0");
    assert!(!Setting::compiled_in().bites(), "the honest run bites nothing");
    assert!(!setting(Env::process()).is_ok_and(|parsed| parsed.bites()));
}

/// `Setting::compiled_in` is a second definition of "the honest run", so it must be the
/// default every arm documents itself against — and the comparison it feeds must not depend
/// on which knob was set.
#[test]
fn the_compiled_in_setting_is_the_honest_run() {
    let honest = Setting::compiled_in();
    assert!(!honest.bites());
    assert_eq!(honest.control, None);
    assert_eq!(honest, setting(Env::list(Vec::new())).expect("no knob set"));
}
