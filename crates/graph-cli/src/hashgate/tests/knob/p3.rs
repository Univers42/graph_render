//! The four Phase 3 controls, each moving its own stage and no other.

use super::*;

/// Every stage's own control moves that stage and no other, at the same seed for all of
/// them. The one arm is the native arm; the wasm arm runs the compiled-in defaults and
/// cannot see a variable, which is what makes a wired knob a cross-target divergence.
/// The four Phase 3 controls, each with a value that must move its own stage: the
/// variable's own environment, and the stage it is filed under. One slice per control
/// because [`env`] reads a slice of pairs.
const P3_NODES: [(&str, &str); 1] = [("GM_MUTATE_TREE_TIDY_NODES", "1")];
const P3_TREEMAP: [(&str, &str); 1] = [("GM_MUTATE_TREEMAP_NODES", "1")];
const P3_CIRCULAR: [(&str, &str); 1] = [("GM_MUTATE_CIRCULAR_NODES", "1")];
const P3_PACKING: [(&str, &str); 1] = [("GM_MUTATE_PACKING_SCALE", "9")];

#[test]
fn every_p3_layout_has_its_own_negative_control_that_moves_only_its_stage() {
    let base = stage_bytes(P3_SEED, &honest()).expect("runs");
    for (pairs, stage) in [
        (P3_NODES, TIDY_TREE),
        (P3_TREEMAP, TREEMAP),
        (P3_CIRCULAR, CIRCULAR),
        (P3_PACKING, PACKING),
    ] {
        let moved = setting(env(pairs.to_vec())).expect("parses");
        only_stage_moved(&base, &stage_bytes(P3_SEED, &moved).expect("runs"), stage);
    }
}

/// The three node-count controls name a *stage*, not a count: the same three variables
/// with the shared `GM_MUTATE_NODE_COUNT`'s meaning would move every stage at once, which
/// is the thing per-stage controls exist to avoid.
#[test]
fn the_p3_node_controls_re_draw_one_stages_model_and_not_the_gate_s_own() {
    let base = stage_bytes(P3_SEED, &honest()).expect("runs");
    let moved = setting(env(vec![("GM_MUTATE_TREE_TIDY_NODES", "3")])).expect("parses");
    assert_eq!(moved.stage_nodes, Some((TIDY_TREE, 3)));
    assert_eq!(moved.extra_nodes, 0, "the gate's own model is untouched");
    let bytes = stage_bytes(P3_SEED, &moved).expect("runs");
    only_stage_moved(&base, &bytes, TIDY_TREE);
    assert_eq!(
        stage_of(&bytes, "topology"),
        stage_of(&base, "topology"),
        "a stage-scoped control must not move the topology the transport stage rests on"
    );
}

/// A typo, a negative count and a zero are all refused, per knob: a control that
/// perturbs by nothing, or that falls back to the default, would pass as green. A
/// negative `scale` parses, and the *packing* refuses it — the same split the grid's own
/// spacing keeps, so each layout's rule stays in the layout.
#[test]
fn the_p3_controls_are_refused_rather_than_falling_back_to_the_default() {
    static REFUSED: [[(&str, &str); 1]; 4] = [
        [("GM_MUTATE_TREE_TIDY_NODES", "one")],
        [("GM_MUTATE_TREEMAP_NODES", "-1")],
        [("GM_MUTATE_CIRCULAR_NODES", "0")],
        [("GM_MUTATE_PACKING_SCALE", "wide")],
    ];
    for pairs in &REFUSED {
        let err = setting(env(pairs.to_vec())).expect_err("refused");
        assert!(err.starts_with(pairs[0].0), "{err}");
    }
    let both = setting(env(vec![
        ("GM_MUTATE_TREE_TIDY_NODES", "1"),
        ("GM_MUTATE_CIRCULAR_NODES", "1"),
    ]))
    .expect_err("one at a time");
    assert!(both.ends_with("one control at a time"), "{both}");
}

/// A `scale` the packing itself refuses stops the run rather than perturbing nothing:
/// the same division of labour `GridSpacing` keeps, where the knob parses the number and
/// the layout owns the rule.
#[test]
fn a_scale_the_packing_refuses_stops_the_run() {
    static REFUSED_SCALES: [[(&str, &str); 1]; 2] = [
        [("GM_MUTATE_PACKING_SCALE", "0")],
        [("GM_MUTATE_PACKING_SCALE", "-1")],
    ];
    for pairs in &REFUSED_SCALES {
        let value = pairs[0].1;
        let setting = setting(env(pairs.to_vec())).expect("parses");
        assert_eq!(setting.packing.scale, value.parse::<f32>().expect("f32"));
        let err = stage_bytes(P3_SEED, &setting).expect_err("refused by the packing");
        assert_eq!(err, "parameter scale: finite and above 0", "{value}");
    }
}

/// The seed every Phase 3 control is checked at. `gate_node_count(seed)` is
/// `2 + seed % 600`, so 4 gives a 6-node model: small enough that the twelve stages
/// finish in well under a second, large enough that a squarify or a packing is not
/// degenerate (a 2-node model has one child and one box, and a control that cannot
/// perturb anything there would pass vacuously).
pub(super) const P3_SEED: u32 = 4;

/// The bytes of one stage from a `stage_bytes` result.
pub(super) fn stage_of<'a>(stages: &'a [(&'static str, Vec<u8>)], id: &str) -> &'a [u8] {
    stages
        .iter()
        .find(|(name, _)| *name == id)
        .map(|(_, bytes)| bytes.as_slice())
        .unwrap_or_else(|| panic!("{id} is a stage"))
}
