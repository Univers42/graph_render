mod stages;

use super::compare::{Tally, diverged, per_stage};
use super::knob::setting;
use super::transport;
use super::*;
use graph_core::Stage;
use graph_core::layout::force::BarnesHut;
use graph_core::layout::forceatlas2::ForceAtlas2;
use graph_core::{GridParams, REFERENCE_DEGREE, SugiyamaParams, gate_node_count, seeded_model};
use std::env::VarError;

mod knob;
mod report;

/// Two seeds per stage; `fills[arm][line]` is the digest's repeated hex digit.
pub(super) fn arms(fills: [[char; 6]; 4]) -> Vec<Arm> {
    let names = [
        "native run 1",
        "native run 2",
        "wasm32 run 1",
        "wasm32 run 2",
    ];
    let line = |i: usize, fill: char| {
        format!(
            "{} {} {}",
            stages()[i / 2],
            i % 2,
            fill.to_string().repeat(64)
        )
    };
    names
        .iter()
        .zip(fills)
        .map(|(n, f)| (*n, (0..6).map(|i| line(i, f[i])).collect()))
        .collect()
}

pub(super) const HONEST: [[char; 6]; 4] = [['a', 'b', 'c', 'd', 'e', 'f']; 4];

/// `arms()` above fixes 3 stages, 2 seeds each (6 lines per arm); the real [`stages`]
/// has grown past that, so these tests exercise [`compare::diverged`] and
/// [`compare::per_stage`] at this small, fixed size, taken from the front of the real
/// list so the stage names printed still match production.
fn test_stages() -> Vec<&'static str> {
    stages()[..3].to_vec()
}

#[test]
fn agreeing_arms_have_no_divergence() {
    assert_eq!(diverged(2, &test_stages(), &arms(HONEST)), Ok(vec![]));
}

/// The N-way claim: any number of arms compares, and **one** of them differing is caught
/// however late in the list it sits.
///
/// This is the gate step 1 makes about tiers — `--tiers all` runs nine arms, and the fifth
/// threaded arm diverging must be as visible as the first native one diverging. A rule
/// that only compared the first four would pass it silently.
#[test]
fn any_arm_of_any_count_diverging_is_caught_wherever_it_sits() {
    // Four original arms plus what `--tiers all` adds: a scalar reference and one arm per
    // worker count in {1, 2, 3, 4, 7} — ten in all.
    let names: [&str; 10] = [
        "native run 1",
        "native run 2",
        "wasm32 run 1",
        "wasm32 run 2",
        "native scalar",
        "native threads 1",
        "native threads 2",
        "native threads 3",
        "native threads 4",
        "native threads 7",
    ];
    let all: Vec<Arm> = names
        .iter()
        .map(|name| (*name, arms(HONEST)[0].1.clone()))
        .collect();
    assert_eq!(all.len(), 10);
    assert_eq!(diverged(2, &test_stages(), &all), Ok(vec![]));
    for (index, name) in names.iter().enumerate() {
        let mut broken = all.clone();
        broken[index].1[3] = format!("{} 1 {}", test_stages()[1], "0".repeat(64));
        assert_eq!(
            diverged(2, &test_stages(), &broken),
            Ok(vec![3]),
            "{name} diverged and was not caught"
        );
    }
    // And two arms differing on different lines are both reported, not just the first.
    // Line `i` is stage `i / 2`, seed `i % 2`, so the prefix has to be the stage that line
    // really belongs to — a well-formedness check that fires first is the gate working.
    let mut two = all.clone();
    two[1].1[0] = format!("{} 0 {}", test_stages()[0], "1".repeat(64));
    two[9].1[5] = format!("{} 1 {}", test_stages()[2], "2".repeat(64));
    assert_eq!(diverged(2, &test_stages(), &two), Ok(vec![0, 5]));
    // Ten arms really is the gate's own `--tiers all` count: four base plus a scalar
    // reference plus one per worker count in {1, 2, 3, 4, 7}. Pinned so adding a tier is
    // a deliberate edit to this number rather than a silent change in the gate's width.
    assert_eq!(names.len(), 4 + 1 + super::tier::WORKER_COUNTS.len());
    assert!(
        names.contains(&"native threads 7"),
        "the odd count must be in the run"
    );
}

#[test]
fn one_arm_differing_on_one_line_names_that_line() {
    let mut fills = HONEST;
    fills[3][3] = 'e';
    assert_eq!(diverged(2, &test_stages(), &arms(fills)), Ok(vec![3]));
    fills[0][0] = 'f';
    assert_eq!(diverged(2, &test_stages(), &arms(fills)), Ok(vec![0, 3]));
}

#[test]
fn vacuous_comparisons_are_refused() {
    assert!(diverged(0, &test_stages(), &[]).is_err());
    // A single arm cannot disagree with itself, and a comparison that cannot fail is not a
    // check. (Three arms *can* disagree, so a three-arm list is compared, not refused —
    // that is the whole point of moving from 4-way to N-way.)
    assert!(diverged(2, &test_stages(), &arms(HONEST)[..1]).is_err());
    assert!(diverged(0, &test_stages(), &[]).is_err());
    // A short arm — one that printed fewer lines than the stage list demands — is refused
    // whatever the list length: a missing seed is a missing check, not a passing one.
    let mut short = arms(HONEST);
    short[1].1.pop();
    assert!(diverged(2, &test_stages(), &short).is_err());
    assert!(diverged(3, &test_stages(), &arms(HONEST)).is_err());
    let mut bad = arms(HONEST);
    bad[2].1[1] = "synthetic 1 xyzzy".into();
    assert!(diverged(2, &test_stages(), &bad).is_err());
    let mut renumbered = arms(HONEST);
    renumbered[1].1[1] = renumbered[1].1[0].clone();
    assert!(diverged(2, &test_stages(), &renumbered).is_err());
    let mut restaged = arms(HONEST);
    restaged[0].1[2] = restaged[0].1[2].replace("layout.grid", "topology");
    assert!(diverged(2, &test_stages(), &restaged).is_err());
}

#[test]
fn a_stage_whose_seeds_all_hash_alike_is_refused_as_one_input() {
    let err = diverged(
        2,
        &test_stages(),
        &arms([['a', 'b', 'c', 'c', 'e', 'f']; 4]),
    )
    .expect_err("one digest");
    assert!(
        err.starts_with("layout.grid: every seed hashed to one digest"),
        "{err}"
    );
    let one: Vec<Arm> = arms(HONEST)
        .into_iter()
        .map(|(n, l)| (n, vec![l[0].clone(), l[2].clone(), l[4].clone()]))
        .collect();
    assert_eq!(diverged(1, &test_stages(), &one), Ok(vec![]));
}

#[test]
fn per_stage_counts_equal_seeds_per_stage_and_distinct_bad_seeds() {
    let tally = per_stage(2, 3, &[1, 3]);
    let equal = vec![1, 1, 2];
    assert_eq!(
        tally,
        Tally {
            equal,
            diverged_seeds: 1
        }
    );
    assert_eq!(per_stage(2, 3, &[0, 5]).diverged_seeds, 2);
    assert_eq!(per_stage(2, 3, &[]).equal, [2, 2, 2]);
}

/// A reader of the variables in `pairs`, every other one unset.
///
/// Takes an owned `Vec` rather than a `&'static` slice so a test can build its pairs from
/// a loop variable; a `'static` bound here would have forced every such test to spell out
/// a `const` table, which is noise around the claim being made.
fn env(pairs: Vec<(&str, &str)>) -> impl Fn(&str) -> Result<String, VarError> {
    move |name| {
        let found = pairs.iter().find(|(key, _)| *key == name);
        found
            .map(|(_, value)| (*value).to_owned())
            .ok_or(VarError::NotPresent)
    }
}

fn honest() -> Setting {
    setting(env(Vec::new())).expect("no knob set")
}

#[test]
fn the_stages_are_the_topology_then_every_registered_layout_then_the_transport() {
    let layouts: Vec<&str> = graph_core::registry::LAYOUTS.iter().map(|l| l.id).collect();
    let mut want = vec!["topology"];
    want.extend(layouts);
    want.push(TRANSPORT);
    assert_eq!(stages(), want);
}

#[test]
fn stage_bytes_are_the_registered_pipeline_and_the_reference_and_spacing_knobs_move_theirs() {
    let honest_run = stage_bytes(4, &honest()).expect("runs");
    let ids: Vec<&str> = honest_run.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, stages());
    let (topology, layout) = (honest_run[0].1.clone(), honest_run[1].1.clone());
    let (nodes, edges) = seeded_model(4, gate_node_count(4), REFERENCE_DEGREE);
    for (id, bytes) in &honest_run[1..honest_run.len() - 1] {
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
        moved[3].1, honest_run[3].1,
        "treemap reads node weight, so the reference degree moves it too"
    );
    assert_eq!(
        moved[dag()].1,
        honest_run[dag()].1,
        "the layered drawing ignores weights"
    );
    let spacing = Setting {
        grid: GridParams { spacing: 2.0 },
        ..honest()
    };
    let spaced = stage_bytes(4, &spacing).expect("runs");
    assert_eq!(spaced[0].1, topology);
    assert_ne!(spaced[1].1, layout);
    assert_eq!(spaced[dag()].1, honest_run[dag()].1);
}

/// Where `layout.dag.sugiyama` sits in [`stages`].
fn dag() -> usize {
    stages()
        .iter()
        .position(|id| *id == "layout.dag.sugiyama")
        .expect("registered")
}

#[test]
fn the_layer_spacing_knob_moves_only_the_layered_drawing_and_zero_is_refused() {
    let honest_run = stage_bytes(4, &honest()).expect("runs");
    assert_eq!(honest_run[dag()].0, "layout.dag.sugiyama");
    let layers = Setting {
        sugiyama: SugiyamaParams { layer_spacing: 2.0 },
        ..honest()
    };
    let moved = stage_bytes(4, &layers).expect("runs");
    for (index, (moved, honest)) in moved.iter().zip(&honest_run).enumerate() {
        assert_eq!(moved.1 != honest.1, index == dag(), "{}", moved.0);
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
    let honest_run = stage_bytes(4, &honest()).expect("runs");
    let more_nodes = Setting {
        extra_nodes: 1,
        ..honest()
    };
    let grown = stage_bytes(4, &more_nodes).expect("runs");
    for (grown, honest) in grown.iter().zip(&honest_run) {
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
    let expected: Vec<String> = stages()
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

/// A wasm arm's lines over the three stages the C20 tally reads, 2 seeds each.
fn transport_arm(fill: char) -> Vec<String> {
    ["topology", LAYOUT, TRANSPORT]
        .iter()
        .flat_map(|stage| {
            (0..2).map(move |seed| format!("{stage} {seed} {}", fill.to_string().repeat(64)))
        })
        .collect()
}

/// The threaded arm and the scalar arm must be **the same list of lines**, in the same
/// order — not merely equal digests.
///
/// This is the test that a bug in `threads_lines` actually caught while this slice was
/// being written: the arm was built seed-major while the comparison reads stage-major, so
/// the gate refused it as malformed at line 1 rather than comparing anything. A test that
/// only compared the *sets* of lines would have passed that arm, and the gate would still
/// have been red — but the failure would have been "not comparable" (exit 2) instead of the
/// "equal" the arm claims.
#[test]
fn the_threaded_arm_prints_its_stages_in_the_same_order_as_the_scalar_one() {
    let setting = honest();
    let scalar: Vec<String> = arm_lines(2, &setting)
        .expect("runs")
        .lines()
        .map(str::to_owned)
        .collect();
    for workers in tier::WORKER_COUNTS {
        let threaded = super::threads_lines(2, &setting, workers).expect("runs");
        assert_eq!(
            threaded.len(),
            scalar.len(),
            "workers={workers}: wrong line count"
        );
        assert_eq!(
            threaded, scalar,
            "workers={workers}: the threaded arm is not the scalar arm's lines"
        );
    }
    // And the order is stage-major: line 0 and line 1 are the same stage, two seeds.
    let prefixes: Vec<&str> = scalar
        .iter()
        .map(|line| line.rsplit_once(' ').expect("digest").0)
        .collect();
    assert_eq!(prefixes[0], format!("{} 0", stages()[0]));
    assert_eq!(prefixes[1], format!("{} 1", stages()[0]));
    assert_eq!(prefixes[2], format!("{} 0", stages()[1]));
}

#[test]
fn the_transport_tally_counts_the_seeds_where_the_real_abi_matches_the_shim() {
    let wasm = |fill: char| transport_arm(fill);
    assert_eq!(transport::agree_with_shim(2, &wasm('a')), Ok(2));
    let mut diverged_at_1 = wasm('a');
    diverged_at_1[5] = format!("{TRANSPORT} 1 {}", "b".repeat(64));
    assert_eq!(transport::agree_with_shim(2, &diverged_at_1), Ok(1));
    assert!(transport::agree_with_shim(2, &wasm('a')[..4]).is_err());
    let mut no_transport = wasm('a');
    no_transport[4] = no_transport[4].replace(TRANSPORT, "topology");
    assert!(transport::agree_with_shim(2, &no_transport).is_err());
    let mut no_layout = wasm('a');
    no_layout[2] = no_layout[2].replace(LAYOUT, "topology");
    assert!(transport::agree_with_shim(2, &no_layout).is_err());
    assert!(transport::agree_with_shim(0, &[]).is_err());
}
