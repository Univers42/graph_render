//! Each knob is read strictly, one at a time, and moves only the stages it backs.
//!
//! [`TIDY_TREE`], [`TREEMAP`], [`CIRCULAR`] and [`PACKING`] are the four Phase 3 stage ids
//! re-exported by the parent module from the layout module that owns each one (see
//! `hashgate/stages.rs`'s module doc) — one spelling of each id, in the crate that
//! implements the layout, rather than a copy here.

mod ids;

use super::super::{Knob, knobs};
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

/// The ten controls that move a parameter. **Spelled out rather than derived from
/// [`Knob::env`]**, so this test is the independent statement of what the first ten are
/// called; the fifteen ANALYSIS and POST controls are absent because their variables come
/// from `knobs::ANALYSIS_POST_STAGES`, which has its own test below.
const PARAMETER_KNOBS: [(&str, &str); 10] = [
    ("GM_MUTATE_REFERENCE_DEGREE", "hashgate-control-reference-degree"),
    ("GM_MUTATE_GRID_SPACING", "hashgate-control-grid-spacing"),
    (
        "GM_MUTATE_SUGIYAMA_LAYER_SPACING",
        "hashgate-control-sugiyama-layer-spacing",
    ),
    ("GM_MUTATE_NODE_COUNT", "hashgate-control-node-count"),
    ("GM_MUTATE_FORCE_THETA", "hashgate-control-force-theta"),
    (
        "GM_MUTATE_FA2_SCALING_RATIO",
        "hashgate-control-fa2-scaling-ratio",
    ),
    (
        "GM_MUTATE_TREE_TIDY_NODES",
        "hashgate-control-tree-tidy-nodes",
    ),
    ("GM_MUTATE_TREEMAP_NODES", "hashgate-control-treemap-nodes"),
    ("GM_MUTATE_CIRCULAR_NODES", "hashgate-control-circular-nodes"),
    ("GM_MUTATE_PACKING_SCALE", "hashgate-control-packing-scale"),
];

#[test]
fn each_knob_names_its_own_variable_and_record() {
    assert_eq!(
        Knob::ALL.len(),
        PARAMETER_KNOBS.len() + knobs::ANALYSIS_POST_STAGES.len(),
        "every knob is either a parameter control or one of the fifteen stage controls"
    );
    for (env, record) in PARAMETER_KNOBS {
        let knob = knob_named(env);
        assert_eq!(knob.record(), record, "{env}");
    }
    // And each name is used once: two knobs sharing a variable would make "one control at a
    // time" refuse a run that set only one of them, and two sharing a record would fold two
    // controls into one line of evidence.
    for knob in Knob::ALL {
        assert_eq!(
            Knob::ALL.iter().filter(|o| o.env() == knob.env()).count(),
            1,
            "{}: two knobs read it",
            knob.env()
        );
        assert_eq!(
            Knob::ALL.iter().filter(|o| o.record() == knob.record()).count(),
            1,
            "{}: two knobs write it",
            knob.record()
        );
    }
}

/// The one knob that reads `env`.
fn knob_named(env: &str) -> Knob {
    *Knob::ALL
        .iter()
        .find(|knob| knob.env() == env)
        .unwrap_or_else(|| panic!("{env} is read"))
}

/// The fifteen ANALYSIS and POST controls are one table, and the enum's arms are held to
/// it: an arm whose variable, record or stage the table disagrees with would perturb a
/// stage nobody asked for, and — the failure mode that matters most — a control that
/// perturbs nothing would pass as green.
#[test]
fn the_analysis_and_post_controls_are_the_knobs_table() {
    let table = knobs::ANALYSIS_POST_STAGES;
    for row in table {
        let knob = knob_named(row.env);
        assert_eq!(knob.record(), row.record, "{}: the record", row.env);
        assert_eq!(knobs::by_env(row.env), Some(row), "{}: the row", row.env);
        let parsed = setting_for(row.env, "1").expect("parses");
        assert_eq!(parsed.control, Some(knob), "{}: the control", row.env);
        assert_eq!(
            parsed.stage_nodes,
            Some((row.id, 1)),
            "{}: the stage it re-draws",
            row.env
        );
    }
    assert_eq!(knobs::by_env("GM_MUTATE_POST_STYLE_CUBIC"), None, "not one of the four");
    assert_eq!(knobs::by_env("GM_MUTATE_GRID_SPACING"), None, "not one of the fifteen");
    // The reverse direction: no enum arm claims a variable the table does not carry, or the
    // table would grow a row `stage_of` could not resolve and the arm would panic at run
    // time instead of at compile time.
    for knob in Knob::ALL {
        let carried = table.iter().any(|row| row.env == knob.env());
        assert_eq!(
            carried || PARAMETER_KNOBS.iter().any(|(env, _)| *env == knob.env()),
            true,
            "{}: no table carries it",
            knob.env()
        );
    }
}

/// Every stage id the fifteen controls name is a stage the gate actually hashes, and comes
/// from the graph-core constant its own module publishes.
///
/// The stage ids are graph-core constants (`knobs::ANALYSIS_POST_STAGES` takes them from
/// `graph_core::analysis::*` and `graph_core::post::*`), so this test is what holds them
/// to the registries the gate walks: a control filed under an id no stage is hashed under
/// moves nothing at all, and the whole point of a negative control is that it moves.
#[test]
fn each_stage_id_is_the_constant_its_own_module_publishes() {
    let hashed = stages();
    for row in knobs::ANALYSIS_POST_STAGES {
        assert!(
            hashed.contains(&row.id),
            "{} is a control for {}, which the gate does not hash",
            row.env,
            row.id
        );
    }
    // One stage, one control — and one stage, one constant: a second control for a stage
    // that moved it too would make "only this stage moved" ambiguous, and two rows sharing
    // an id would fold two controls into one stage's evidence.
    let mut ids: Vec<&str> = knobs::ANALYSIS_POST_STAGES.iter().map(|r| r.id).collect();
    ids.sort_unstable();
    let before = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), before, "two controls name one stage");
}

/// Each of the fifteen turns the gate red for its own stage and no other: the property a
/// negative control exists for. One arm — the native arm, at one seed — with the variable
/// set to one more node, and every other stage's bytes compared against the honest run.
#[test]
fn each_analysis_and_post_stage_has_its_own_control_that_moves_only_its_stage() {
    let base = stage_bytes(P3_SEED, &honest()).expect("runs");
    for row in knobs::ANALYSIS_POST_STAGES {
        // Ponytail: one more node is a weak probe — a stage could be degenerate at this
        // model size and move nothing. The seed is the gate's smallest non-degenerate one
        // (`P3_SEED`), the same probe the three Phase 3 node controls use, and a stage
        // that did *not* move is caught below as a failing assertion rather than passed.
        let moved = setting_for(row.env, "1").expect("parses");
        let bytes = stage_bytes(P3_SEED, &moved).expect("runs");
        only_stage_moved(&base, &bytes, row.id);
    }
}

/// A typo and a zero are refused for the fifteen too, exactly as for the three Phase 3 node
/// controls: a control that falls back to the default, or perturbs by nothing, would pass
/// as green.
///
/// Zero is the case that matters most — it is a perturbation of nothing, and a control that
/// read it as "no control set" would run the honest model and pass.
#[test]
fn the_analysis_and_post_controls_are_refused_rather_than_defaulting() {
    for row in knobs::ANALYSIS_POST_STAGES {
        for value in ["one", "1.5", "-1", "0", ""] {
            let err = setting_for(row.env, value).expect_err("refused");
            assert!(
                err.starts_with(row.env),
                "{}={value:?}: {err}",
                row.env
            );
        }
    }
    // And the doubled case, the other half of "at most one": two of the fifteen together is
    // refused rather than one of them silently winning.
    let [a, b] = [knobs::ANALYSIS_POST_STAGES[0], knobs::ANALYSIS_POST_STAGES[1]];
    let err = setting(|read| match read {
        x if x == a.env => Ok("1".to_owned()),
        x if x == b.env => Ok("1".to_owned()),
        _ => Err(VarError::NotPresent),
    })
    .expect_err("one at a time");
    assert!(err.ends_with("one control at a time"), "{err}");
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

/// The [`Setting`] `env` produces alone. `env` reads a `&'static` pair list, so the slice
/// has to outlive the call — this is the one-line form, for a variable whose name is only
/// known at run time (the fifteen).
fn setting_for(name: &'static str, value: &'static str) -> Result<Setting, String> {
    setting(|read| match read == name {
        true => Ok(value.to_owned()),
        false => Err(VarError::NotPresent),
    })
}

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
