//! Each knob is read strictly, one at a time, and moves only the stages it backs.
//!
//! [`TIDY_TREE`], [`TREEMAP`], [`CIRCULAR`] and [`PACKING`] are the four Phase 3 stage ids
//! re-exported by the parent module from the layout module that owns each one (see
//! `hashgate/stages.rs`'s module doc) — one spelling of each id, in the crate that
//! implements the layout, rather than a copy here.

mod ids;
mod p3;
use p3::{P3_SEED, stage_of};

use super::super::*;
use super::env;
use super::honest;
use super::{Knob, Setting, setting, stage_bytes};
use graph_core::layout::circle_packing::CirclePackingParams;
pub(super) use graph_core::layout::circle_packing::ID as PACKING;
pub(super) use graph_core::layout::circular::ID as CIRCULAR;
use graph_core::layout::force::BarnesHut;
pub(super) use graph_core::layout::tidy_tree::ID as TIDY_TREE;
pub(super) use graph_core::layout::treemap::ID as TREEMAP;
use graph_core::layout::force::Split;
use graph_core::layout::forceatlas2::ForceAtlas2;
use graph_core::{GridParams, REFERENCE_DEGREE, SugiyamaParams};
use std::env::VarError;

#[test]
fn the_mutation_variables_parse_strictly_and_one_at_a_time() {
    let defaults = (REFERENCE_DEGREE, GridParams::default(), 0u32, None);
    let h = honest();
    assert_eq!(
        (h.reference_degree, h.grid, h.extra_nodes, h.control),
        defaults
    );

    assert_parses_reference_degree(&h);
    assert_parses_grid_spacing(&h);
    assert_parses_sugiyama_layer_spacing(&h);
    assert_parses_node_count(&h);
    assert_refuses_bad_values();
    assert_refuses_two_controls();
    assert_refuses_unreadable_variable();
}

fn assert_parses_reference_degree(h: &Setting) {
    let degree = setting(env(vec![("GM_MUTATE_REFERENCE_DEGREE", " 9 ")])).expect("parses");
    assert_eq!((degree.reference_degree, degree.grid), (9, h.grid));
    assert_eq!(degree.control, Some(Knob::ReferenceDegree));
}

fn assert_parses_grid_spacing(_h: &Setting) {
    let spacing = setting(env(vec![("GM_MUTATE_GRID_SPACING", "2.5")])).expect("parses");
    assert_eq!(
        (spacing.reference_degree, spacing.grid.spacing),
        (REFERENCE_DEGREE, 2.5)
    );
    assert_eq!(spacing.control, Some(Knob::GridSpacing));
}

fn assert_parses_sugiyama_layer_spacing(h: &Setting) {
    assert_eq!(h.sugiyama, SugiyamaParams::default());
    assert_eq!(h.packing, CirclePackingParams::default());
    assert_eq!(
        h.stage_nodes, None,
        "no stage re-draws its own model by default"
    );
    let layers = setting(env(vec![("GM_MUTATE_SUGIYAMA_LAYER_SPACING", "3.5")])).expect("parses");
    assert_eq!(layers.sugiyama.layer_spacing, 3.5);
    assert_eq!(
        (layers.reference_degree, layers.grid, layers.control),
        (REFERENCE_DEGREE, h.grid, Some(Knob::SugiyamaLayerSpacing))
    );
}

fn assert_parses_node_count(_h: &Setting) {
    let nodes = setting(env(vec![("GM_MUTATE_NODE_COUNT", " 1 ")])).expect("parses");
    assert_eq!(nodes.extra_nodes, 1);
    assert_eq!(nodes.control, Some(Knob::NodeCount));
}

fn assert_refuses_bad_values() {
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
}

fn assert_refuses_two_controls() {
    let both = env(vec![
        ("GM_MUTATE_REFERENCE_DEGREE", "9"),
        ("GM_MUTATE_GRID_SPACING", "2"),
    ]);
    let err = setting(both).expect_err("two controls");
    assert!(err.ends_with("one control at a time"), "{err}");
}

fn assert_refuses_unreadable_variable() {
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
            "GM_MUTATE_PACKING_SCALE",
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
            "hashgate-control-tree-tidy-nodes",
            "hashgate-control-treemap-nodes",
            "hashgate-control-circular-nodes",
            "hashgate-control-packing-scale",
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

/// `GM_MUTATE_SPLIT_SUM` names **which** gathered pass's merge to split, and is parsed
/// rather than treated as a presence flag: `0` is the honest run and a typo is an error
/// instead of a silent mutation.
#[test]
fn the_split_sum_knob_names_the_pass_it_corrupts() {
    for (word, want) in [
        ("1", Split::All),
        ("true", Split::All),
        ("TRUE", Split::All),
        (" 1 ", Split::All),
        ("0", Split::None),
        ("false", Split::None),
        ("FALSE", Split::None),
        ("charge", Split::Charge),
        ("collide", Split::Collide),
        ("link", Split::Link),
    ] {
        let read = env(vec![("GM_MUTATE_SPLIT_SUM", word)]);
        let got = setting(read).expect(word);
        assert_eq!(got.split_sum, want, "GM_MUTATE_SPLIT_SUM={word:?}");
        assert_eq!(got.control, Some(Knob::SplitSum));
    }
    for typo in ["yes", "2", "", "on", "manybody"] {
        let read = env(vec![("GM_MUTATE_SPLIT_SUM", typo)]);
        let err = setting(read).expect_err(typo);
        assert!(err.contains("GM_MUTATE_SPLIT_SUM"), "{err}");
    }
    // And it is off by default: an unset variable must not mutate anything.
    assert_eq!(honest().split_sum, Split::None);
}

/// Every word the knob accepts is a pass the stage actually hands to the runner: a control
/// for a pass that no longer exists would go red for the wrong reason, and one for a pass
/// that was threaded without being listed would go red for no reason at all.
///
/// Compared as a **set**, because the two lists are the same three names and the order is
/// not the claim: the knob takes one word at a time and the stage's list is printed in the
/// tick's own order. What must hold is that neither list has a name the other lacks.
#[test]
fn every_word_the_split_knob_accepts_is_a_threaded_pass() {
    let mut accepted: Vec<&str> = ["charge", "collide", "link"].to_vec();
    let mut listed: Vec<&str> = BarnesHut::THREADED_PASSES.to_vec();
    accepted.sort_unstable();
    listed.sort_unstable();
    assert_eq!(
        accepted, listed,
        "the knob's words and the stage's passes are one set, whatever their order"
    );
}

/// **A control that cannot bite must refuse the run, not pass it.** Collide's own control
/// moves nothing below five seeds — at two or three nodes link and many-body have already
/// pushed every pair past `2 * collideRadius`, so the deltas it would split are all zero
/// (measured in `barnes_hut/tests/kernels.rs`, where collide first bites at seed 4). A row
/// run at two seeds would exit 0 having corrupted nothing: a **vacuous pass**, which is
/// worse than a failure because it reads as evidence. So the gate refuses the run instead,
/// and the refusal is exit 2, "could not run" — never exit 0.
#[test]
fn a_control_that_cannot_bite_at_this_seed_count_refuses_rather_than_passing() {
    for split in [Split::Collide, Split::All] {
        for seeds in 0..5 {
            let err = refuse_a_vacuous_control(seeds, split).expect_err("refused");
            assert!(
                err.contains("collide") && err.contains(&format!("{seeds}")),
                "the refusal must name the pass and the count: {err:?}"
            );
        }
        assert!(
            refuse_a_vacuous_control(5, split).is_ok(),
            "{split:?} bites at five seeds and must run"
        );
        assert!(
            refuse_a_vacuous_control(8, split).is_ok(),
            "{split:?} at the phase gate's eight seeds must run"
        );
    }
    // The other two bites at the first seed, and an honest run sets no control at all: a
    // floor on those would refuse rows the gate has always run and needs.
    for split in [Split::None, Split::Charge, Split::Link] {
        assert!(
            refuse_a_vacuous_control(1, split).is_ok(),
            "{split:?} bites at one seed"
        );
    }
    assert!(
        refuse_a_vacuous_control(2, Split::None).is_ok(),
        "an honest run has no control to be vacuous"
    );
}

/// The floor is the model's, not a constant invented beside the gate: graph-core measures
/// which seed each pass's control first bites at, and the gate reads the same number.
#[test]
fn the_collide_controls_floor_is_the_one_graph_core_measures() {
    assert_eq!(
        Split::Collide.min_seeds(),
        5,
        "collide first bites at seed 4, so five seeds is the floor"
    );
    assert_eq!(Split::All.min_seeds(), Split::Collide.min_seeds());
    assert_eq!(Split::Charge.min_seeds(), 1);
    assert_eq!(Split::Link.min_seeds(), 1);
    assert_eq!(Split::None.min_seeds(), 1);
}

/// A force layout's own negative control must move that stage and *only* that stage: a
/// control that also moved the topology would back every stage at once and prove nothing
/// about the stage it is filed under.
pub(super) fn only_stage_moved(
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
