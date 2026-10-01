//! The `layout.treemap.patchwork` control, moving its own stage and no other.
//!
//! `patchwork` is a closed form with no parameter of its own — it pins Graphviz's default
//! `area` of 1 and no `inset`, and its module doc says so — so its control is the same
//! re-drawn-model probe the four other node controls use, scoped to this one stage.

use super::*;
use p3::stage_of;

/// One slice per control because [`env`] reads a slice of pairs.
const PATCHWORK: [(&str, &str); 1] = [("GM_MUTATE_PATCHWORK_NODES", "1")];

/// The seed the control is checked at, and the stage id it is filed under.
///
/// `gate_node_count(4)` is 6 nodes, which is one of the sizes graph-core's own patchwork
/// closed cases draw by hand, so a control here and a `Ponytail (area and inset)` divergence
/// there are the same shape. One seed is enough because the claim is "this control moves this
/// stage and no other", which is a statement about the stage list rather than about the seed.
const PATCHWORK_SEED: u32 = 4;
const PATCHWORK_ID: &str = "layout.treemap.patchwork";

/// The control re-draws one stage's model and moves only that stage.
#[test]
fn the_patchwork_control_re_draws_one_stages_model_and_moves_only_that_stage() {
    let base = stage_bytes(PATCHWORK_SEED, &honest()).expect("runs");
    let moved = setting(env(PATCHWORK.to_vec())).expect("parses");
    assert_eq!(moved.stage_nodes, Some((PATCHWORK_ID, 1)));
    assert_eq!(moved.extra_nodes, 0, "the gate's own model is untouched");
    let bytes = stage_bytes(PATCHWORK_SEED, &moved).expect("runs");
    only_stage_moved(&base, &bytes, PATCHWORK_ID);
    assert_eq!(
        stage_of(&bytes, "topology"),
        stage_of(&base, "topology"),
        "a stage-scoped control must not move the topology the transport stage rests on"
    );
}

/// A typo and a zero are refused: a control that perturbs by nothing would pass vacuously,
/// which is the one failure mode a negative control must not have.
#[test]
fn the_patchwork_control_is_refused_rather_than_falling_back_to_the_default() {
    for value in ["one", "0", "-1"] {
        let pairs = [("GM_MUTATE_PATCHWORK_NODES", value)];
        let err = setting(env(pairs.to_vec())).expect_err("refused");
        assert!(err.starts_with("GM_MUTATE_PATCHWORK_NODES"), "{err}");
    }
    let both = setting(env(vec![
        ("GM_MUTATE_PATCHWORK_NODES", "1"),
        ("GM_MUTATE_TWOPI_NODES", "1"),
    ]))
    .expect_err("one at a time");
    assert!(both.ends_with("one control at a time"), "{both}");
}
