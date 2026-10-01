//! Each knob is read strictly, one at a time, and moves only the stages it backs.
//!
//! [`TIDY_TREE`], [`TREEMAP`], [`CIRCULAR`] and [`PACKING`] are the four Phase 3 stage ids
//! re-exported by the parent module from the layout module that owns each one (see
//! `hashgate/stages.rs`'s module doc) — one spelling of each id, in the crate that
//! implements the layout, rather than a copy here.
//!
//! Split by the house's 300-line limit: [`controls`] holds the two force controls, the
//! vacuous-control refusal and the rescale-merge control, [`ids`] the four Phase 3 stage
//! ids, [`p3`] the four Phase 3 controls, and [`table`] the knob table itself — the
//! thirteen parameter controls, the fifteen ANALYSIS and POST controls, and the two
//! compute-tier controls, each held against the variable and record it claims. [`neato`] holds the
//! Graphviz stress engine's tolerance control, which is the first one here that perturbs a
//! *parameter* rather than re-drawing a model's size.

mod controls;
mod ids;
mod neato;
mod p3;
mod table;
mod twopi;
use controls::only_stage_moved;
use p3::P3_SEED;

use super::super::*;
use super::env;
use super::honest;
use super::{Knob, Setting, setting, stage_bytes};
use graph_core::Stage as _;
use graph_core::layout::circle_packing::CirclePackingParams;
pub(super) use graph_core::layout::circle_packing::ID as PACKING;
pub(super) use graph_core::layout::circular::ID as CIRCULAR;
use graph_core::layout::force::BarnesHut;
use graph_core::layout::force::Split;
use graph_core::layout::forceatlas2::ForceAtlas2;
pub(super) use graph_core::layout::tidy_tree::ID as TIDY_TREE;
pub(super) use graph_core::layout::treemap::ID as TREEMAP;
use graph_core::layout::{circular::ring, spiral};
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

/// `GM_MUTATE_SPLIT_RESCALE` is a flag, parsed rather than tested for presence: `0` is the
/// honest run and a typo an error rather than a silent mutation — the same discipline as
/// its sibling, and the reason the two cannot drift on what counts as "on".
#[test]
fn the_split_rescale_knob_is_a_flag_parsed_strictly() {
    for (word, want) in [
        ("1", true),
        ("true", true),
        ("TRUE", true),
        (" 1 ", true),
        ("0", false),
        ("false", false),
        ("FALSE", false),
    ] {
        let read = env(vec![("GM_MUTATE_SPLIT_RESCALE", word)]);
        let got = setting(read).expect(word);
        assert_eq!(got.split_rescale, want, "GM_MUTATE_SPLIT_RESCALE={word:?}");
        assert_eq!(got.control, Some(Knob::SplitRescale));
    }
    for typo in ["maybe", "2", "", "charge", "-1"] {
        let read = env(vec![("GM_MUTATE_SPLIT_RESCALE", typo)]);
        let err = setting(read).expect_err(typo);
        assert!(err.contains("GM_MUTATE_SPLIT_RESCALE"), "{err}");
    }
    // Off by default, and inert for the other control: an unset variable must not mutate
    // anything, and the two compute-tier knobs must not share a setting.
    assert!(!honest().split_rescale);
    let both = env(vec![
        ("GM_MUTATE_SPLIT_SUM", "1"),
        ("GM_MUTATE_SPLIT_RESCALE", "1"),
    ]);
    assert!(
        setting(both)
            .expect_err("one at a time")
            .ends_with("one control at a time")
    );
}
