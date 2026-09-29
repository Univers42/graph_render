//! Each knob is read strictly, one at a time, and moves only the stages it backs.

use super::*;

#[test]
fn the_mutation_variables_parse_strictly_and_one_at_a_time() {
    let defaults = (REFERENCE_DEGREE, GridParams::default(), 0u32, None);
    let h = honest();
    assert_eq!(
        (h.reference_degree, h.grid, h.extra_nodes, h.control),
        defaults
    );
    let degree = setting(env(&[("GM_MUTATE_REFERENCE_DEGREE", " 9 ")])).expect("parses");
    assert_eq!((degree.reference_degree, degree.grid), (9, h.grid));
    assert_eq!(degree.control, Some(Knob::ReferenceDegree));
    let spacing = setting(env(&[("GM_MUTATE_GRID_SPACING", "2.5")])).expect("parses");
    assert_eq!(
        (spacing.reference_degree, spacing.grid.spacing),
        (REFERENCE_DEGREE, 2.5)
    );
    assert_eq!(spacing.control, Some(Knob::GridSpacing));
    assert_eq!(h.sugiyama, SugiyamaParams::default());
    assert_eq!(h.packing, CirclePackingParams::default());
    assert_eq!(
        h.stage_nodes, None,
        "no stage re-draws its own model by default"
    );
    let layers = setting(env(&[("GM_MUTATE_SUGIYAMA_LAYER_SPACING", "3.5")])).expect("parses");
    assert_eq!(layers.sugiyama.layer_spacing, 3.5);
    assert_eq!(
        (layers.reference_degree, layers.grid, layers.control),
        (REFERENCE_DEGREE, h.grid, Some(Knob::SugiyamaLayerSpacing))
    );
    let nodes = setting(env(&[("GM_MUTATE_NODE_COUNT", " 1 ")])).expect("parses");
    assert_eq!(nodes.extra_nodes, 1);
    assert_eq!(nodes.control, Some(Knob::NodeCount));
    let bad: [&'static [(&str, &str)]; 5] = [
        &[("GM_MUTATE_SUGIYAMA_LAYER_SPACING", "tall")],
        &[("GM_MUTATE_REFERENCE_DEGREE", "nine")],
        &[("GM_MUTATE_REFERENCE_DEGREE", "")],
        &[("GM_MUTATE_GRID_SPACING", "wide")],
        &[("GM_MUTATE_NODE_COUNT", "-1")],
    ];
    for pairs in bad {
        let err = setting(env(pairs)).expect_err("refused");
        assert!(err.starts_with(pairs[0].0), "{err}");
    }
    let both = env(&[
        ("GM_MUTATE_REFERENCE_DEGREE", "9"),
        ("GM_MUTATE_GRID_SPACING", "2"),
    ]);
    let err = setting(both).expect_err("two controls");
    assert!(err.ends_with("one control at a time"), "{err}");
    let unreadable = setting(|_| Err(VarError::NotUnicode("\u{fffd}".into())));
    assert!(unreadable.is_err());
}

#[test]
fn each_knob_names_its_own_variable_and_record() {
    let envs = Knob::ALL.map(Knob::env);
    let records = Knob::ALL.map(Knob::record);
    assert_eq!(
        envs,
        [
            "GM_MUTATE_REFERENCE_DEGREE",
            "GM_MUTATE_GRID_SPACING",
            "GM_MUTATE_SUGIYAMA_LAYER_SPACING",
            "GM_MUTATE_NODE_COUNT",
            "GM_MUTATE_FORCE_THETA",
            "GM_MUTATE_FA2_SCALING_RATIO",
            "GM_MUTATE_TREE_TIDY_NODES",
            "GM_MUTATE_TREEMAP_NODES",
            "GM_MUTATE_CIRCULAR_NODES",
            "GM_MUTATE_PACKING_SCALE"
        ]
    );
    assert_eq!(
        records,
        [
            "hashgate-control-reference-degree",
            "hashgate-control-grid-spacing",
            "hashgate-control-sugiyama-layer-spacing",
            "hashgate-control-node-count",
            "hashgate-control-force-theta",
            "hashgate-control-fa2-scaling-ratio",
            "hashgate-control-tree-tidy-nodes",
            "hashgate-control-treemap-nodes",
            "hashgate-control-circular-nodes",
            "hashgate-control-packing-scale"
        ]
    );
}

/// A force layout's own negative control must move that stage and *only* that stage: a
/// control that also moved the topology would back every stage at once and prove nothing
/// about the stage it is filed under.
fn only_stage_moved(
    base: &[(&'static str, Vec<u8>)],
    moved: &[(&'static str, Vec<u8>)],
    stage: &str,
) {
    for ((id, a), (_, b)) in base.iter().zip(moved) {
        assert_eq!(
            a == b,
            *id != stage,
            "only {stage} may move, but {id} did not"
        );
    }
}

/// Every stage's own control moves that stage and no other, at the same seed for all of
/// them. The one arm is the native arm; the wasm arm runs the compiled-in defaults and
/// cannot see a variable, which is what makes a wired knob a cross-target divergence.
/// The four Phase 3 controls, each with a value that must move its own stage: the
/// variable's own environment, and the stage it is filed under. One slice per control
/// because [`env`] reads a `&'static` pair list.
const P3_NODES: [(&str, &str); 1] = [("GM_MUTATE_TREE_TIDY_NODES", "1")];
const P3_TREEMAP: [(&str, &str); 1] = [("GM_MUTATE_TREEMAP_NODES", "1")];
const P3_CIRCULAR: [(&str, &str); 1] = [("GM_MUTATE_CIRCULAR_NODES", "1")];
const P3_PACKING: [(&str, &str); 1] = [("GM_MUTATE_PACKING_SCALE", "9")];

#[test]
fn every_p3_layout_has_its_own_negative_control_that_moves_only_its_stage() {
    let base = stage_bytes(P3_SEED, &honest()).expect("runs");
    for (pairs, stage) in [
        (&P3_NODES, TIDY_TREE),
        (&P3_TREEMAP, TREEMAP),
        (&P3_CIRCULAR, CIRCULAR),
        (&P3_PACKING, PACKING),
    ] {
        let moved = setting(env(pairs)).expect("parses");
        only_stage_moved(&base, &stage_bytes(P3_SEED, &moved).expect("runs"), stage);
    }
}

/// The four Phase 3 stage ids are the registry's own: a constant here that drifted from
/// `graph_core::registry::LAYOUTS` would file a control under a stage the gate does not
/// hash, and it would then move nothing at all.
#[test]
fn the_p3_stage_ids_are_the_registry_s_own() {
    for id in [TIDY_TREE, TREEMAP, CIRCULAR, PACKING] {
        assert!(
            graph_core::registry::find(id).is_some(),
            "{id} is registered"
        );
    }
    assert_eq!(
        stage_bytes(P3_SEED, &honest())
            .expect("runs")
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>(),
        stages(),
        "the ids the knobs name are the ids the gate hashes, in the same order"
    );
}

/// The three node-count controls name a *stage*, not a count: the same three variables
/// with the shared `GM_MUTATE_NODE_COUNT`'s meaning would move every stage at once, which
/// is the thing per-stage controls exist to avoid.
#[test]
fn the_p3_node_controls_re_draw_one_stages_model_and_not_the_gate_s_own() {
    let base = stage_bytes(P3_SEED, &honest()).expect("runs");
    let moved = setting(env(&[("GM_MUTATE_TREE_TIDY_NODES", "3")])).expect("parses");
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
        let err = setting(env(pairs)).expect_err("refused");
        assert!(err.starts_with(pairs[0].0), "{err}");
    }
    let both = setting(env(&[
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
        let setting = setting(env(pairs)).expect("parses");
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
const P3_SEED: u32 = 4;

/// The bytes of one stage from a `stage_bytes` result.
fn stage_of<'a>(stages: &'a [(&'static str, Vec<u8>)], id: &str) -> &'a [u8] {
    stages
        .iter()
        .find(|(name, _)| *name == id)
        .map(|(_, bytes)| bytes.as_slice())
        .unwrap_or_else(|| panic!("{id} is a stage"))
}

#[test]
fn each_force_layout_has_its_own_negative_control_that_moves_only_its_stage() {
    let base = stage_bytes(FORCE_SEED, &honest()).expect("runs");
    let theta = setting(env(&[("GM_MUTATE_FORCE_THETA", "0.5")])).expect("parses");
    assert_eq!(theta.control, Some(Knob::ForceTheta));
    assert_eq!(theta.force.theta, 0.5);
    only_stage_moved(
        &base,
        &stage_bytes(FORCE_SEED, &theta).expect("runs"),
        BarnesHut::ID,
    );
    let scaling = setting(env(&[("GM_MUTATE_FA2_SCALING_RATIO", "3")])).expect("parses");
    assert_eq!(scaling.control, Some(Knob::Fa2ScalingRatio));
    assert_eq!(scaling.fa2.scaling_ratio, 3.0);
    only_stage_moved(
        &base,
        &stage_bytes(FORCE_SEED, &scaling).expect("runs"),
        ForceAtlas2::ID,
    );
    let both = env(&[
        ("GM_MUTATE_FORCE_THETA", "0.5"),
        ("GM_MUTATE_FA2_SCALING_RATIO", "3"),
    ]);
    assert!(
        setting(both)
            .expect_err("one at a time")
            .ends_with("one control at a time")
    );
    // A typo in either new variable must be refused, not fall back to the default and
    // let the control pass as green.
    let bad: [&'static [(&str, &str)]; 2] = [
        &[("GM_MUTATE_FORCE_THETA", "wide")],
        &[("GM_MUTATE_FA2_SCALING_RATIO", "")],
    ];
    for pairs in bad {
        let err = setting(env(pairs)).expect_err("refused");
        assert!(err.starts_with(pairs[0].0), "{err}");
    }
}

/// The seed whose model is large enough that a theta change reaches the quadtree's
/// opening test. At the gate's smallest models every cell is already inside theta and
/// the two values coincide, which would make the control vacuous.
const FORCE_SEED: u32 = 30;
