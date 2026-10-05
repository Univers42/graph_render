//! The `layout.dag.lanes` control, moving its own stage and no other.
//!
//! `lanes` is the same shape as the `dot` control above and for the same reason: what it
//! reads is the whole `Topology` — every vertex's `version` and every edge's directedness —
//! so its model is its entire input and one more node is exactly what moves it.
//!
//! **This is the control `layout.dag.lanes` needed for `Status::Gated`.** `verdict::hash_4way`
//! refuses a gated row whose own stage no control went red on, and `GM_MUTATE_NODE_COUNT`
//! moves the one shared model without naming this stage. The names below are the external
//! check on the row the gate actually reads: spelled out here rather than taken from
//! `Knob::env`, `Knob::record` or `knobs::LANES_LAYOUT_STAGES`, because those are what the arm
//! returns and deriving the test from them would check the table against itself.
//!
//! One honest limit, recorded here rather than left for a reader to find: this control proves
//! the layout *moves* when its model is re-drawn. It cannot prove the layout is right — see
//! `docs/decisions/dag-lanes.md` condition 7 on what the gate model does and does not reach.

use super::*;
use graph_core::layout::lanes;

/// One slice per control because [`env`] reads a slice of pairs.
const LANES: [(&str, &str); 1] = [("GM_MUTATE_DAG_LANES_NODES", "1")];

/// The variable, the record its run writes, and the stage it is filed under.
///
/// **The record name is the claim that matters to the ledger**: `red_control` finds a control
/// by matching this exact string against the record files the gate writes, so a control whose
/// record name drifts from the one the ledger looks for perturbs the gate and still backs
/// nothing. The stage is `lanes::ID`, the constant the layout's own module publishes.
#[test]
fn the_lanes_control_names_its_own_variable_record_and_stage() {
    let knob = *Knob::ALL
        .iter()
        .find(|knob| knob.env() == LANES[0].0)
        .unwrap_or_else(|| panic!("{} is read", LANES[0].0));
    assert_eq!(
        (
            knob.record(),
            crate::hashgate::knobs::LANES_LAYOUT_STAGES[0].env,
            crate::hashgate::knobs::LANES_LAYOUT_STAGES[0].id,
        ),
        (
            "hashgate-control-dag-lanes-nodes",
            "GM_MUTATE_DAG_LANES_NODES",
            "layout.dag.lanes",
        ),
        "the variable, the record and the stage are each spelled out independently of the \
         table the arm reads"
    );
    assert_eq!(lanes::ID, "layout.dag.lanes", "the table's id is the constant");
}

/// The control names a *stage*, not a count: the same variable with the shared
/// `GM_MUTATE_NODE_COUNT`'s meaning would move every stage at once, which is the thing
/// per-stage controls exist to avoid — and would leave the ledger unable to say which stage a
/// divergence came from.
#[test]
fn the_lanes_control_re_draws_one_stages_model_and_moves_only_that_stage() {
    let base = stage_bytes(LANES_SEED, &honest()).expect("runs");
    let moved = setting(env(LANES.to_vec())).expect("parses");
    assert_eq!(moved.stage_nodes, Some((lanes::ID, 1)));
    assert_eq!(moved.extra_nodes, 0, "the gate's own model is untouched");
    let bytes = stage_bytes(LANES_SEED, &moved).expect("runs");
    only_stage_moved(&base, &bytes, lanes::ID);
}

/// A typo, a negative count and a zero are refused: a control that falls back to the default,
/// or perturbs by nothing, would pass vacuously, which is the one failure mode a negative
/// control must not have.
#[test]
fn the_lanes_control_is_refused_rather_than_falling_back_to_the_default() {
    for value in ["one", "1.5", "-1", "0", ""] {
        let pairs = [("GM_MUTATE_DAG_LANES_NODES", value)];
        let err = setting(env(pairs.to_vec())).expect_err("refused");
        assert!(err.starts_with("GM_MUTATE_DAG_LANES_NODES"), "{err}");
    }
    // And it is one control at a time: two of them together is refused rather than one
    // silently winning.
    let both = setting(env(vec![
        ("GM_MUTATE_DAG_LANES_NODES", "1"),
        ("GM_MUTATE_SUGIYAMA_LAYER_SPACING", "2"),
    ]))
    .expect_err("one at a time");
    assert!(both.ends_with("one control at a time"), "{both}");
}

/// The seed the control is checked at.
///
/// `P3_SEED` rather than a seed of its own, for the reason the re-drawn-model controls
/// everywhere share: the claim is "this control moves this stage and no other", which is a
/// statement about the stage list rather than about the seed, so the gate's smallest
/// non-degenerate model is the right place to make it.
const LANES_SEED: u32 = P3_SEED;