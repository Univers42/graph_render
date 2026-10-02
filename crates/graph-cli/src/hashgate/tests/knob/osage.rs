//! The `layout.packing.osage` control, moving its own stage and no other.
//!
//! osage is a closed form with no parameter of its own — it publishes no `Params`, has no
//! `impl Stage`, and names a `Params` as a contract change that is not that job's
//! (`osage.rs:72`) — so its control is the same re-drawn-model probe the twopi and patchwork
//! controls use, scoped to this one stage. What it does read is the node count and no edge
//! at all, so its model is its entire input and one more node is exactly what moves it.
//!
//! **This is the control `layout.packing.osage` needed for `Status::Gated`.**
//! `verdict::hash_4way` refuses a gated row whose own stage no control went red on, and none
//! of the controls that existed reached it: `GM_MUTATE_NODE_COUNT` moves the one shared
//! model, and `GM_MUTATE_PACKING_SCALE` is `layout.packing.circle`'s real parameter while
//! osage reads no scale — so the row stayed `implemented` however good its oracle record
//! was. The names below are the external check on the row the gate actually reads: they are
//! spelled out here rather than taken from `Knob::env`/`Knob::record` or from
//! `knobs::OSAGE_LAYOUT_STAGES`, because those are what the arm returns and deriving the test
//! from them would check the table against itself.

use super::*;
use graph_core::layout::graphviz::osage;

/// One slice per control because [`env`] reads a slice of pairs.
const OSAGE: [(&str, &str); 1] = [("GM_MUTATE_PACKING_OSAGE_NODES", "1")];

/// The variable, the record its run writes, and the stage it is filed under.
///
/// **The record name is the claim that matters to the ledger**: `red_control` finds a
/// control by matching this exact string against the record files the gate writes, so a
/// control whose record name drifts from the one the ledger looks for perturbs the gate and
/// still backs nothing. The stage is `osage::ID`, the constant osage's own module publishes.
#[test]
fn the_osage_control_names_its_own_variable_record_and_stage() {
    let knob = *Knob::ALL
        .iter()
        .find(|knob| knob.env() == OSAGE[0].0)
        .unwrap_or_else(|| panic!("{} is read", OSAGE[0].0));
    assert_eq!(
        (
            knob.record(),
            crate::hashgate::knobs::OSAGE_LAYOUT_STAGES[0].env,
            crate::hashgate::knobs::OSAGE_LAYOUT_STAGES[0].id,
        ),
        (
            "hashgate-control-packing-osage-nodes",
            "GM_MUTATE_PACKING_OSAGE_NODES",
            "layout.packing.osage",
        ),
        "the variable, the record and the stage are each spelled out independently of the \
         table the arm reads"
    );
    assert_eq!(
        osage::ID,
        "layout.packing.osage",
        "the table's id is the constant"
    );
}

/// The control names a *stage*, not a count: the same variable with the shared
/// `GM_MUTATE_NODE_COUNT`'s meaning would move every stage at once, which is the thing
/// per-stage controls exist to avoid — and would leave the ledger unable to say which stage a
/// divergence came from.
#[test]
fn the_osage_control_re_draws_one_stages_model_and_moves_only_that_stage() {
    let base = stage_bytes(OSAGE_SEED, &honest()).expect("runs");
    let moved = setting(env(OSAGE.to_vec())).expect("parses");
    assert_eq!(moved.stage_nodes, Some((osage::ID, 1)));
    assert_eq!(moved.extra_nodes, 0, "the gate's own model is untouched");
    let bytes = stage_bytes(OSAGE_SEED, &moved).expect("runs");
    only_stage_moved(&base, &bytes, osage::ID);
}

/// A typo, a negative count and a zero are refused: a control that falls back to the default,
/// or perturbs by nothing, would pass vacuously, which is the one failure mode a negative
/// control must not have.
#[test]
fn the_osage_control_is_refused_rather_than_falling_back_to_the_default() {
    for value in ["one", "1.5", "-1", "0", ""] {
        let pairs = [("GM_MUTATE_PACKING_OSAGE_NODES", value)];
        let err = setting(env(pairs.to_vec())).expect_err("refused");
        assert!(err.starts_with("GM_MUTATE_PACKING_OSAGE_NODES"), "{err}");
    }
    // And it is one control at a time beside the packing neighbour whose name it shares a
    // prefix with: two of them together is refused rather than one silently winning.
    let both = setting(env(vec![
        ("GM_MUTATE_PACKING_OSAGE_NODES", "1"),
        ("GM_MUTATE_PACKING_SCALE", "2"),
    ]))
    .expect_err("one at a time");
    assert!(both.ends_with("one control at a time"), "{both}");
}

/// The seed the control is checked at.
///
/// `P3_SEED` rather than a seed of its own, for the reason the re-drawn-model controls
/// everywhere share: the claim is "this control moves this stage and no other", which is a
/// statement about the stage list rather than about the seed, so the gate's smallest
/// non-degenerate model is the right place to make it. osage does not degenerate at it —
/// `only_stage_moved` above fails the test if it does.
const OSAGE_SEED: u32 = P3_SEED;
