//! The knob table: every control's variable, record and stage, held against the one source
//! of truth for the per-stage controls (the fifteen ANALYSIS and POST rows and the six
//! igraph layout rows).
//!
//! Split from `knob.rs` by the house's 300-line limit. The ten parameter controls are
//! spelled out here rather than derived from [`Knob::env`], so this test is the
//! independent statement of what they are called; the per-stage controls are absent because
//! their variables come from `knobs::all()`, which has its own test below.

use super::*;
use crate::hashgate::knob::setting::setting;
use crate::hashgate::knobs;

/// The nineteen controls that move a parameter or re-draw one layout's model. **Spelled
/// out rather than derived from [`Knob::env`]**, so this test is the independent statement
/// of what they are called; the per-stage controls are absent because their variables come
/// from `knobs::all()`, which has its own test below.
const PARAMETER_KNOBS: [(&str, &str); 19] = [
    (
        "GM_MUTATE_REFERENCE_DEGREE",
        "hashgate-control-reference-degree",
    ),
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
    (
        "GM_MUTATE_CIRCULAR_NODES",
        "hashgate-control-circular-nodes",
    ),
    ("GM_MUTATE_TWOPI_NODES", "hashgate-control-twopi-nodes"),
    ("GM_MUTATE_NEATO_EPSILON", "hashgate-control-neato-epsilon"),
    (
        "GM_MUTATE_PATCHWORK_NODES",
        "hashgate-control-patchwork-nodes",
    ),
    (
        "GM_MUTATE_SPRING_ITERATIONS",
        "hashgate-control-spring-iterations",
    ),
    (
        "GM_MUTATE_CIRCULAR_HIERARCHY_NODES",
        "hashgate-control-circular-hierarchy-nodes",
    ),
    ("GM_MUTATE_PACKING_SCALE", "hashgate-control-packing-scale"),
    (
        "GM_MUTATE_LAYOUT_PARAM_DEFAULT",
        "hashgate-control-layout-param-default",
    ),
    (
        "GM_MUTATE_FORCE_SESSION_GRAVITY",
        "forcegate-control-force-session-gravity",
    ),
    (
        "GM_MUTATE_OVERLAP_RELAXATION",
        "hashgate-control-overlap-relaxation",
    ),
    ("GM_MUTATE_DROP_DELTA", "forcegate-control-drop-delta"),
];

/// The six igraph layout controls, spelled out by variable and record rather than read off
/// `knob::igraph::ENV`/`RECORD`: that table is what `Knob::env` and `Knob::record` return,
/// so deriving the test from it would check the table against itself. Being an independent
/// copy is the property, exactly as above.
///
/// The order is `Knob::ALL`'s igraph group and `knobs::IGRAPH_LAYOUT_STAGES`'s, and the
/// stage each names is the graph-core constant its layout publishes — both asserted
/// below, so a permutation of the group cannot pass.
const IGRAPH_KNOBS: [(&str, &str, &str); 6] = [
    (
        "GM_MUTATE_FORCE_FRUCHTERMAN_REINGOLD_NODES",
        "hashgate-control-force-fruchterman-reingold-nodes",
        "layout.force.fruchterman_reingold",
    ),
    (
        "GM_MUTATE_FORCE_KAMADA_KAWAI_NODES",
        "hashgate-control-force-kamada-kawai-nodes",
        "layout.force.kamada_kawai",
    ),
    (
        "GM_MUTATE_FORCE_GRAPHOPT_NODES",
        "hashgate-control-force-graphopt-nodes",
        "layout.force.graphopt",
    ),
    (
        "GM_MUTATE_FORCE_DAVIDSON_HAREL_NODES",
        "hashgate-control-force-davidson-harel-nodes",
        "layout.force.davidson_harel",
    ),
    (
        "GM_MUTATE_FORCE_LGL_NODES",
        "hashgate-control-force-lgl-nodes",
        "layout.force.lgl",
    ),
    (
        "GM_MUTATE_FORCE_DRL_NODES",
        "hashgate-control-force-drl-nodes",
        "layout.force.drl",
    ),
];

/// The compute-tier controls, spelled out rather than counted: each one corrupts a **merge**
/// rather than a parameter, so it is the control that proves a threaded arm recomputed that
/// merge. One per merge family — Barnes-Hut's three range kernels share
/// [`Knob::SplitSum`], the closed-form point layouts' single `coords` merge is
/// [`Knob::SplitRescale`]. A third arm here would be a merge nobody has, and a control that
/// perturbs nothing passes vacuously.
const COMPUTE_TIER_KNOBS: [Knob; 2] = [Knob::SplitSum, Knob::SplitRescale];

#[test]
fn each_knob_names_its_own_variable_and_record() {
    assert_eq!(
        Knob::ALL.len(),
        PARAMETER_KNOBS.len() + knobs::all().count() + COMPUTE_TIER_KNOBS.len(),
        "every knob is a parameter control, one of the twenty-six per-stage controls, or \
         one of the compute-tier controls"
    );
    for (env, record) in PARAMETER_KNOBS {
        let knob = knob_named(env);
        assert_eq!(knob.record(), record, "{env}");
    }
    for (env, record, id) in IGRAPH_KNOBS {
        let knob = knob_named(env);
        assert_eq!(
            (knob.record(), knobs::by_env(env).expect("tabled").id),
            (record, id),
            "{env}"
        );
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
            Knob::ALL
                .iter()
                .filter(|o| o.record() == knob.record())
                .count(),
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

/// The twenty-one per-stage controls are one table, and the enum's arms are held to it: an
/// arm whose variable, record or stage a table disagrees with would perturb a stage nobody
/// asked for, and — the failure mode that matters most — a control that perturbs nothing
/// would pass as green.
#[test]
fn the_analysis_and_post_controls_are_the_knobs_table() {
    for row in knobs::all() {
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
    assert_eq!(
        knobs::by_env("GM_MUTATE_POST_STYLE_CUBIC"),
        None,
        "not one of the four"
    );
    assert_eq!(
        knobs::by_env("GM_MUTATE_FORCE_FORCEATLAS2_NODES"),
        None,
        "not one of the six: ForceAtlas2 has its own parameter control"
    );
    assert_eq!(
        knobs::by_env("GM_MUTATE_GRID_SPACING"),
        None,
        "not one of the per-stage controls"
    );
    // The reverse direction: no enum arm claims a variable no table and the parameter list
    // carry, or a table would grow a row `stage_of` could not resolve and the arm would
    // panic at run time instead of at compile time. The compute-tier controls are the arms
    // outside both, by design — they are the `+ 2` in the count above.
    for knob in Knob::ALL {
        let carried = knobs::all().any(|row| row.env == knob.env());
        let parameter = PARAMETER_KNOBS.iter().any(|(env, _)| *env == knob.env());
        assert!(
            carried || parameter || COMPUTE_TIER_KNOBS.contains(&knob),
            "{}: no table carries it",
            knob.env()
        );
    }
}

/// Every stage id the per-stage controls name is a stage the gate actually hashes, and comes
/// from the graph-core constant its own module publishes.
///
/// The stage ids are graph-core constants (`knobs::ANALYSIS_POST_STAGES` takes them from
/// `graph_core::analysis::*` and `graph_core::post::*`; `knobs::IGRAPH_LAYOUT_STAGES` from
/// each layout's `Stage::ID`), so this test is what holds them to the registries the gate
/// walks: a control filed under an id no stage is hashed under moves nothing at all, and
/// the whole point of a negative control is that it moves.
#[test]
fn each_stage_id_is_the_constant_its_own_module_publishes() {
    let hashed = stages();
    for row in knobs::all() {
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
    let mut ids: Vec<&str> = knobs::all().map(|r| r.id).collect();
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
    for row in knobs::all() {
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
    for row in knobs::all() {
        for value in ["one", "1.5", "-1", "0", ""] {
            let err = setting_for(row.env, value).expect_err("refused");
            assert!(err.starts_with(row.env), "{}={value:?}: {err}", row.env);
        }
    }
    // And the doubled case, the other half of "at most one": two of the per-stage controls
    // together is refused rather than one of them silently winning — and one from each
    // family, since the rule is about the variable and not about which table carried it.
    let [a, b] = [
        knobs::ANALYSIS_POST_STAGES[0],
        knobs::IGRAPH_LAYOUT_STAGES[0],
    ];
    let err = setting(|read| match read {
        x if x == a.env => Ok("1".to_owned()),
        x if x == b.env => Ok("1".to_owned()),
        _ => Err(VarError::NotPresent),
    })
    .expect_err("one at a time");
    assert!(err.ends_with("one control at a time"), "{err}");
}

/// The [`Setting`] `env` produces alone. `env` reads a `&'static` pair list, so the slice
/// has to outlive the call — this is the one-line form, for a variable whose name is only
/// known at run time (the fifteen).
fn setting_for(name: &'static str, value: &'static str) -> Result<Setting, String> {
    setting(|read| match read == name {
        true => Ok(value.to_owned()),
        false => Err(VarError::NotPresent),
    })
}
