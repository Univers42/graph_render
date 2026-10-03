//! The `layout.force.neato` control, moving its own stage and no other.
//!
//! `neato` is the one hashed stage whose control perturbs a **parameter** rather than
//! re-drawing a model: its `Epsilon` is a tolerance on convergence (`stress.h:25`), so
//! moving it changes how far the iteration runs and therefore the drawing, with the graph
//! untouched. That makes it a sharper probe than the re-drawn-model controls, and it is why
//! graph-core publishes `neato::run_with` — a parameter a gate row cannot name is not a
//! parameter the layout has.

use super::*;
use p3::stage_of;

/// One slice per control because [`env`] reads a slice of pairs.
const NEATO: [(&str, &str); 1] = [("GM_MUTATE_NEATO_EPSILON", "1e-2")];

/// The control names a *tolerance*, not a node count, so it cannot reach the shared model
/// and therefore cannot move a stage that is only a function of the topology.
#[test]
fn the_neato_control_perturbs_one_stages_tolerance_and_moves_only_that_stage() {
    let base = stage_bytes(NEATO_SEED, &honest()).expect("runs");
    let moved = setting(env(NEATO.to_vec())).expect("parses");
    assert_eq!(moved.neato_epsilon, Some(1e-2));
    assert_eq!(moved.stage_nodes, None, "no stage's own model is re-drawn");
    assert_eq!(moved.extra_nodes, 0, "the gate's own model is untouched");
    let bytes = stage_bytes(NEATO_SEED, &moved).expect("runs");
    only_stage_moved(&base, &bytes, NEATO_ID);
    assert_eq!(
        stage_of(&bytes, "topology"),
        stage_of(&base, "topology"),
        "a stage-scoped control must not move the topology the transport stage rests on"
    );
}

/// The negative control, and the one that makes the test above mean anything: a tolerance
/// that is *the default* moves nothing. Without it, a control whose perturbation never
/// reached the layout would also "move only that stage" — by moving none, and passing.
///
/// `1e-4` is `neato::EPSILON` exactly, so this asserts the gate compares the parsed value
/// against the layout's own constant rather than against a copy of it that could drift.
///
/// **It is refused, not parsed** (RG-42). The control used to accept `=1e-4`, set
/// `control = Some(NeatoEpsilon)`, and then hashed byte-for-byte the honest drawing while
/// writing this knob's evidence record — a negative control that claimed a comparison which
/// never happened. The claim the test makes is unchanged and is now a refusal.
#[test]
fn the_neato_control_at_the_default_tolerance_is_refused_as_a_no_op() {
    let pairs = [("GM_MUTATE_NEATO_EPSILON", "1e-4")];
    let err = setting(env(pairs.to_vec())).expect_err("the layout's own EPSILON");
    assert!(err.contains("perturbs nothing"), "{err}");
    assert_eq!(
        graph_core::layout::graphviz::neato::EPSILON,
        1e-4,
        "the refusal is this constant's: change it and this test's input is no longer it"
    );
    // Zero is a *legal* tolerance and is not the compiled-in one, so it is accepted and does
    // perturb — the no-op rule is one comparison, not a blanket ban on the honest-looking.
    let zero = setting(env(vec![("GM_MUTATE_NEATO_EPSILON", "0")])).expect("a legal tolerance");
    assert_eq!(zero.neato_epsilon(), 0.0);
    let base = stage_bytes(NEATO_SEED, &honest()).expect("runs");
    let stopped = stage_bytes(NEATO_SEED, &zero).expect("runs");
    assert_ne!(
        stage_of(&base, graph_core::layout::graphviz::neato::ID),
        stage_of(&stopped, graph_core::layout::graphviz::neato::ID),
        "a zero tolerance stops the solve on the stress clause, so it is a different drawing"
    );
}

/// A coarser tolerance must change *this* stage's bytes, which is what says the control
/// reaches the layout's arithmetic rather than only its bookkeeping: the parsed tolerance is
/// carried into the solve, and a solve that ran a different number of passes is a different
/// drawing.
#[test]
fn a_coarser_tolerance_changes_the_stages_own_bytes() {
    let base = stage_bytes(NEATO_SEED, &honest()).expect("runs");
    for value in ["1e-2", "1e-1", "0.5"] {
        let pairs = [("GM_MUTATE_NEATO_EPSILON", value)];
        let setting = setting(env(pairs.to_vec())).expect("parses");
        let bytes = stage_bytes(NEATO_SEED, &setting).expect("runs");
        assert_ne!(
            stage_of(&bytes, NEATO_ID),
            stage_of(&base, NEATO_ID),
            "epsilon {value} did not move the stage"
        );
    }
}

/// A typo and a negative value are refused: a control that perturbed by nothing, or by an
/// unreachable one, would pass vacuously. **`0` is accepted**, because it is a legal
/// tolerance — the reference's second convergence clause stops the first pass — so the
/// control's honest value has to be expressible.
#[test]
fn the_neato_control_is_refused_rather_than_falling_back_to_the_default() {
    for value in ["maybe", "-1", "nan", "inf"] {
        let pairs = [("GM_MUTATE_NEATO_EPSILON", value)];
        let err = setting(env(pairs.to_vec())).expect_err("refused");
        assert!(err.starts_with("GM_MUTATE_NEATO_EPSILON"), "{err}");
    }
    let zero = setting(env([("GM_MUTATE_NEATO_EPSILON", "0")].to_vec())).expect("legal");
    assert_eq!(zero.neato_epsilon, Some(0.0));
    let both = setting(env(vec![
        ("GM_MUTATE_NEATO_EPSILON", "1e-2"),
        ("GM_MUTATE_CIRCULAR_NODES", "1"),
    ]))
    .expect_err("one at a time");
    assert!(both.ends_with("one control at a time"), "{both}");
}

/// The seed the control is checked at, and the stage id it is filed under.
///
/// `gate_node_count(4)` is 6 nodes, the same seed the twopi control uses, so the two are
/// directly comparable: one re-draws that stage's model and this one perturbs a parameter,
/// and both must move exactly one stage. Six nodes is above the smallest graphs
/// graph-core's own neato tests draw by hand, so a control here and a bug there are the
/// same shape rather than two different regimes.
const NEATO_SEED: u32 = 4;
const NEATO_ID: &str = "layout.force.neato";
