use super::compare::{Tally, diverged, per_stage};
use super::knob::setting;
use super::*;
use graph_core::Stage;
use graph_core::layout::force::BarnesHut;
use graph_core::layout::forceatlas2::ForceAtlas2;
use graph_core::{GridParams, REFERENCE_DEGREE, SugiyamaParams};
use std::env::VarError;

mod knob;
mod report;

/// Two seeds per stage; `fills[arm][line]` is the digest's repeated hex digit.
fn arms(fills: [[char; 4]; 4]) -> Vec<Arm> {
    let names = [
        "native run 1",
        "native run 2",
        "wasm32 run 1",
        "wasm32 run 2",
    ];
    let line = |i: usize, fill: char| {
        format!(
            "{} {} {}",
            STAGES[i / 2],
            i % 2,
            fill.to_string().repeat(64)
        )
    };
    names
        .iter()
        .zip(fills)
        .map(|(n, f)| (*n, (0..4).map(|i| line(i, f[i])).collect()))
        .collect()
}

const HONEST: [[char; 4]; 4] = [['a', 'b', 'c', 'd']; 4];

/// `arms()` above fixes 2 stages, 2 seeds each (4 lines per arm); the real [`STAGES`]
/// has grown past that, so these tests exercise [`compare::diverged`] and
/// [`compare::per_stage`] at this small, fixed size, taken from the front of the real
/// list so the stage names printed still match production.
const TEST_STAGES: [&str; 2] = [STAGES[0], STAGES[1]];

#[test]
fn agreeing_arms_have_no_divergence() {
    assert_eq!(diverged(2, &TEST_STAGES, &arms(HONEST)), Ok(vec![]));
}

#[test]
fn one_arm_differing_on_one_line_names_that_line() {
    let mut fills = HONEST;
    fills[3][3] = 'e';
    assert_eq!(diverged(2, &TEST_STAGES, &arms(fills)), Ok(vec![3]));
    fills[0][0] = 'f';
    assert_eq!(diverged(2, &TEST_STAGES, &arms(fills)), Ok(vec![0, 3]));
}

#[test]
fn vacuous_comparisons_are_refused() {
    assert!(diverged(0, &TEST_STAGES, &[]).is_err());
    assert!(diverged(2, &TEST_STAGES, &arms(HONEST)[..3]).is_err());
    assert!(diverged(3, &TEST_STAGES, &arms(HONEST)).is_err());
    let mut bad = arms(HONEST);
    bad[2].1[1] = "synthetic 1 xyzzy".into();
    assert!(diverged(2, &TEST_STAGES, &bad).is_err());
    let mut renumbered = arms(HONEST);
    renumbered[1].1[1] = renumbered[1].1[0].clone();
    assert!(diverged(2, &TEST_STAGES, &renumbered).is_err());
    let mut restaged = arms(HONEST);
    restaged[0].1[2] = restaged[0].1[2].replace("layout.grid", "topology");
    assert!(diverged(2, &TEST_STAGES, &restaged).is_err());
}

#[test]
fn a_stage_whose_seeds_all_hash_alike_is_refused_as_one_input() {
    let err = diverged(2, &TEST_STAGES, &arms([['a', 'b', 'c', 'c']; 4])).expect_err("one digest");
    assert!(
        err.starts_with("layout.grid: every seed hashed to one digest"),
        "{err}"
    );
    let one: Vec<Arm> = arms(HONEST)
        .into_iter()
        .map(|(n, l)| (n, vec![l[0].clone(), l[2].clone()]))
        .collect();
    assert_eq!(diverged(1, &TEST_STAGES, &one), Ok(vec![]));
}

#[test]
fn per_stage_counts_equal_seeds_per_stage_and_distinct_bad_seeds() {
    let tally = per_stage(2, 2, &[1, 3]);
    assert_eq!(
        tally,
        Tally {
            equal: vec![1, 1],
            diverged_seeds: 1
        }
    );
    assert_eq!(per_stage(2, 2, &[0, 3]).diverged_seeds, 2);
    assert_eq!(per_stage(2, 2, &[]).equal, [2, 2]);
}

/// A reader of the variables in `pairs`, every other one unset.
fn env(pairs: &'static [(&str, &str)]) -> impl Fn(&str) -> Result<String, VarError> {
    |name| {
        let found = pairs.iter().find(|(key, _)| *key == name);
        found
            .map(|(_, value)| (*value).to_owned())
            .ok_or(VarError::NotPresent)
    }
}

fn honest() -> Setting {
    setting(env(&[])).expect("no knob set")
}

#[test]
fn the_stages_are_the_topology_then_every_registered_layout() {
    let layouts: Vec<&str> = graph_core::registry::LAYOUTS.iter().map(|l| l.id).collect();
    assert_eq!(STAGES[0], "topology");
    assert_eq!(STAGES[1..], layouts);
}

#[test]
fn stage_bytes_are_the_registered_pipeline_and_the_reference_and_spacing_knobs_move_theirs() {
    let stages = stage_bytes(4, &honest()).expect("runs");
    let ids: Vec<&str> = stages.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, STAGES);
    let (topology, layout) = (stages[0].1.clone(), stages[1].1.clone());
    let (nodes, edges) = seeded_model(4, gate_node_count(4), REFERENCE_DEGREE);
    for (id, bytes) in &stages[1..] {
        let layout = graph_core::registry::find(id).expect("registered");
        let registered = graph_core::run_with(&nodes, &edges, layout.id, layout.run).expect("runs");
        assert_eq!(
            (&registered.topology, &registered.snapshot.to_bytes()),
            (&topology, bytes),
            "{id}"
        );
    }
    let degree = Setting {
        reference_degree: REFERENCE_DEGREE + 1,
        ..honest()
    };
    let moved = stage_bytes(4, &degree).expect("runs");
    assert_ne!(moved[0].1, topology);
    assert_eq!(moved[1].1, layout, "the grid ignores weights");
    assert_ne!(
        moved[3].1, stages[3].1,
        "treemap reads node weight, so the reference degree moves it too"
    );
    assert_eq!(
        moved[DAG].1, stages[DAG].1,
        "the layered drawing ignores weights"
    );
    let spacing = Setting {
        grid: GridParams { spacing: 2.0 },
        ..honest()
    };
    let spaced = stage_bytes(4, &spacing).expect("runs");
    assert_eq!(spaced[0].1, topology);
    assert_ne!(spaced[1].1, layout);
    assert_eq!(spaced[DAG].1, stages[DAG].1);
}

/// Where `layout.dag.sugiyama` sits in [`STAGES`].
const DAG: usize = STAGE_COUNT - 1;

#[test]
fn the_layer_spacing_knob_moves_only_the_layered_drawing_and_zero_is_refused() {
    let stages = stage_bytes(4, &honest()).expect("runs");
    assert_eq!(stages[DAG].0, "layout.dag.sugiyama");
    let layers = Setting {
        sugiyama: SugiyamaParams { layer_spacing: 2.0 },
        ..honest()
    };
    let moved = stage_bytes(4, &layers).expect("runs");
    for (index, (moved, honest)) in moved.iter().zip(&stages).enumerate() {
        assert_eq!(moved.1 != honest.1, index == DAG, "{}", moved.0);
    }
    let flat = Setting {
        sugiyama: SugiyamaParams { layer_spacing: 0.0 },
        ..honest()
    };
    let err = stage_bytes(4, &flat).expect_err("zero layer spacing");
    assert_eq!(err, "parameter layer_spacing: finite and above 0");
}

#[test]
fn the_node_count_knob_moves_every_stage_and_zero_spacing_is_refused() {
    let stages = stage_bytes(4, &honest()).expect("runs");
    let more_nodes = Setting {
        extra_nodes: 1,
        ..honest()
    };
    let grown = stage_bytes(4, &more_nodes).expect("runs");
    for (grown, honest) in grown.iter().zip(&stages) {
        assert_ne!(
            grown.1, honest.1,
            "{}: one more node must move every stage",
            grown.0
        );
    }
    let refused = Setting {
        grid: GridParams { spacing: 0.0 },
        ..honest()
    };
    let err = stage_bytes(4, &refused).expect_err("zero spacing");
    assert_eq!(err, "parameter spacing: finite and above 0");
}

#[test]
fn an_arm_prints_every_seed_of_one_stage_before_the_next() {
    let lines = arm_lines(2, &honest()).expect("runs");
    let prefixes: Vec<_> = lines
        .lines()
        .map(|l| l.rsplit_once(' ').expect("digest").0)
        .collect();
    let expected: Vec<String> = STAGES
        .iter()
        .flat_map(|stage| (0..2).map(move |seed| format!("{stage} {seed}")))
        .collect();
    assert_eq!(prefixes, expected);
    let refused = Setting {
        grid: GridParams { spacing: -1.0 },
        ..honest()
    };
    assert!(
        arm_lines(2, &refused)
            .expect_err("refused")
            .starts_with("seed 0: ")
    );
}
