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
    let degree = setting(env(vec![("GM_MUTATE_REFERENCE_DEGREE", " 9 ")])).expect("parses");
    assert_eq!((degree.reference_degree, degree.grid), (9, h.grid));
    assert_eq!(degree.control, Some(Knob::ReferenceDegree));
    let spacing = setting(env(vec![("GM_MUTATE_GRID_SPACING", "2.5")])).expect("parses");
    assert_eq!(
        (spacing.reference_degree, spacing.grid.spacing),
        (REFERENCE_DEGREE, 2.5)
    );
    assert_eq!(spacing.control, Some(Knob::GridSpacing));
    assert_eq!(h.sugiyama, SugiyamaParams::default());
    let layers = setting(env(vec![("GM_MUTATE_SUGIYAMA_LAYER_SPACING", "3.5")])).expect("parses");
    assert_eq!(layers.sugiyama.layer_spacing, 3.5);
    assert_eq!(
        (layers.reference_degree, layers.grid, layers.control),
        (REFERENCE_DEGREE, h.grid, Some(Knob::SugiyamaLayerSpacing))
    );
    let nodes = setting(env(vec![("GM_MUTATE_NODE_COUNT", " 1 ")])).expect("parses");
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
        let err = setting(env(pairs.to_vec())).expect_err("refused");
        assert!(err.starts_with(pairs[0].0), "{err}");
    }
    let both = env(vec![
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
            "GM_MUTATE_SPLIT_SUM"
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
            "hashgate-control-split-sum"
        ]
    );
    // Every variable is distinct and every record is distinct: two knobs sharing a name
    // would make one of them unreachable, and two sharing a record would overwrite it.
    for (label, names) in [("variable", envs), ("record", records)] {
        let mut sorted = names.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "two knobs share a {label}");
    }
}

/// `GM_MUTATE_SPLIT_SUM` is parsed, not treated as a presence flag: `0` is the honest run
/// and a typo is an error rather than a silent mutation.
#[test]
fn the_split_sum_knob_takes_a_boolean_and_refuses_anything_else() {
    let on = setting(env(vec![("GM_MUTATE_SPLIT_SUM", "true")])).expect("true");
    assert!(on.split_sum);
    assert_eq!(on.control, Some(Knob::SplitSum));
    for truthy in ["1", "true", "TRUE", " 1 "] {
        let read = env(vec![("GM_MUTATE_SPLIT_SUM", truthy)]);
        assert!(
            setting(read).expect(truthy).split_sum,
            "{truthy:?} must set it"
        );
    }
    for falsy in ["0", "false", "FALSE"] {
        let read = env(vec![("GM_MUTATE_SPLIT_SUM", falsy)]);
        assert!(
            !setting(read).expect(falsy).split_sum,
            "{falsy:?} must leave it off"
        );
    }
    for typo in ["yes", "2", "", "on"] {
        let read = env(vec![("GM_MUTATE_SPLIT_SUM", typo)]);
        let err = setting(read).expect_err(typo);
        assert!(err.contains("GM_MUTATE_SPLIT_SUM"), "{err}");
    }
    // And it is off by default: an unset variable must not mutate anything.
    assert!(!honest().split_sum);
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

#[test]
fn each_force_layout_has_its_own_negative_control_that_moves_only_its_stage() {
    let base = stage_bytes(FORCE_SEED, &honest()).expect("runs");
    let theta = setting(env(vec![("GM_MUTATE_FORCE_THETA", "0.5")])).expect("parses");
    assert_eq!(theta.control, Some(Knob::ForceTheta));
    assert_eq!(theta.force.theta, 0.5);
    only_stage_moved(
        &base,
        &stage_bytes(FORCE_SEED, &theta).expect("runs"),
        BarnesHut::ID,
    );
    let scaling = setting(env(vec![("GM_MUTATE_FA2_SCALING_RATIO", "3")])).expect("parses");
    assert_eq!(scaling.control, Some(Knob::Fa2ScalingRatio));
    assert_eq!(scaling.fa2.scaling_ratio, 3.0);
    only_stage_moved(
        &base,
        &stage_bytes(FORCE_SEED, &scaling).expect("runs"),
        ForceAtlas2::ID,
    );
    let both = env(vec![
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
        let err = setting(env(pairs.to_vec())).expect_err("refused");
        assert!(err.starts_with(pairs[0].0), "{err}");
    }
}

/// The seed whose model is large enough that a theta change reaches the quadtree's
/// opening test. At the gate's smallest models every cell is already inside theta and
/// the two values coincide, which would make the control vacuous.
const FORCE_SEED: u32 = 30;
