//! The `layout.twopi` control, moving its own stage and no other.
//!
//! `twopi` is a closed form with no parameter of its own — it pins Graphviz's defaults and
//! its module doc says so — so its control is the same re-drawn-model probe the three
//! Phase 3 node controls use, scoped to this one stage.

use super::*;
use p3::stage_of;

/// One slice per control because [`env`] reads a slice of pairs.
const TWOPI: [(&str, &str); 1] = [("GM_MUTATE_TWOPI_NODES", "1")];

/// The control names a *stage*, not a count: the same variable with the shared
/// `GM_MUTATE_NODE_COUNT`'s meaning would move every stage at once, which is the thing
/// per-stage controls exist to avoid.
#[test]
fn the_twopi_control_re_draws_one_stages_model_and_moves_only_that_stage() {
    let base = stage_bytes(TWOPI_SEED, &honest()).expect("runs");
    let moved = setting(env(TWOPI.to_vec())).expect("parses");
    assert_eq!(moved.stage_nodes, Some((TWOPI_ID, 1)));
    assert_eq!(moved.extra_nodes, 0, "the gate's own model is untouched");
    let bytes = stage_bytes(TWOPI_SEED, &moved).expect("runs");
    only_stage_moved(&base, &bytes, TWOPI_ID);
    assert_eq!(
        stage_of(&bytes, "topology"),
        stage_of(&base, "topology"),
        "a stage-scoped control must not move the topology the transport stage rests on"
    );
}

/// A typo and a zero are refused: a control that perturbs by nothing would pass vacuously,
/// which is the one failure mode a negative control must not have.
#[test]
fn the_twopi_control_is_refused_rather_than_falling_back_to_the_default() {
    for value in ["one", "0", "-1"] {
        let pairs = [("GM_MUTATE_TWOPI_NODES", value)];
        let err = setting(env(pairs.to_vec())).expect_err("refused");
        assert!(err.starts_with("GM_MUTATE_TWOPI_NODES"), "{err}");
    }
    let both = setting(env(vec![
        ("GM_MUTATE_TWOPI_NODES", "1"),
        ("GM_MUTATE_CIRCULAR_NODES", "1"),
    ]))
    .expect_err("one at a time");
    assert!(both.ends_with("one control at a time"), "{both}");
}

/// The seed the control is checked at, and the stage id it is filed under.
///
/// `gate_node_count(4)` is 6 nodes: the largest model the model builder makes below the
/// six-node cases graph-core's own twopi tests draw by hand, so a control here and a
/// `Ponytail (parallel edge)` bug there are the same shape. One seed is enough because the
/// claim is "this control moves this stage and no other", which is a statement about the
/// stage list rather than about the seed.
const TWOPI_SEED: u32 = 4;
const TWOPI_ID: &str = "layout.twopi";
