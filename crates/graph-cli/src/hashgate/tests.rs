use super::compare::{Tally, diverged, per_stage};
use super::*;

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

#[test]
fn agreeing_arms_have_no_divergence() {
    assert_eq!(diverged(2, &arms(HONEST)), Ok(vec![]));
}

#[test]
fn one_arm_differing_on_one_line_names_that_line() {
    let mut fills = HONEST;
    fills[3][3] = 'e';
    assert_eq!(diverged(2, &arms(fills)), Ok(vec![3]));
    fills[0][0] = 'f';
    assert_eq!(diverged(2, &arms(fills)), Ok(vec![0, 3]));
}

#[test]
fn vacuous_comparisons_are_refused() {
    assert!(diverged(0, &[]).is_err());
    assert!(diverged(2, &arms(HONEST)[..3]).is_err());
    assert!(diverged(3, &arms(HONEST)).is_err());
    let mut bad = arms(HONEST);
    bad[2].1[1] = "synthetic 1 xyzzy".into();
    assert!(diverged(2, &bad).is_err());
    let mut renumbered = arms(HONEST);
    renumbered[1].1[1] = renumbered[1].1[0].clone();
    assert!(diverged(2, &renumbered).is_err());
    let mut restaged = arms(HONEST);
    restaged[0].1[2] = restaged[0].1[2].replace("layout.grid", "topology");
    assert!(diverged(2, &restaged).is_err());
}

#[test]
fn a_stage_whose_seeds_all_hash_alike_is_refused_as_one_input() {
    let err = diverged(2, &arms([['a', 'b', 'c', 'c']; 4])).expect_err("one digest");
    assert!(
        err.starts_with("layout.grid: every seed hashed to one digest"),
        "{err}"
    );
    let one: Vec<Arm> = arms(HONEST)
        .into_iter()
        .map(|(n, l)| (n, vec![l[0].clone(), l[2].clone()]))
        .collect();
    assert_eq!(diverged(1, &one), Ok(vec![]));
}

#[test]
fn per_stage_counts_equal_seeds_per_stage_and_distinct_bad_seeds() {
    let tally = per_stage(2, &[1, 3]);
    assert_eq!(
        tally,
        Tally {
            equal: vec![1, 1],
            diverged_seeds: 1
        }
    );
    assert_eq!(per_stage(2, &[0, 3]).diverged_seeds, 2);
    assert_eq!(per_stage(2, &[]).equal, [2, 2]);
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
fn the_mutation_variables_parse_strictly_and_one_at_a_time() {
    let defaults = (REFERENCE_DEGREE, GridParams::default(), None);
    let h = honest();
    assert_eq!((h.reference_degree, h.grid, h.control), defaults);
    let degree = setting(env(&[("GM_MUTATE_REFERENCE_DEGREE", " 9 ")])).expect("parses");
    assert_eq!((degree.reference_degree, degree.grid), (9, h.grid));
    assert_eq!(degree.control, Some(Knob::ReferenceDegree));
    let spacing = setting(env(&[("GM_MUTATE_GRID_SPACING", "2.5")])).expect("parses");
    assert_eq!(
        (spacing.reference_degree, spacing.grid.spacing),
        (REFERENCE_DEGREE, 2.5)
    );
    assert_eq!(spacing.control, Some(Knob::GridSpacing));
    let bad: [&'static [(&str, &str)]; 3] = [
        &[("GM_MUTATE_REFERENCE_DEGREE", "nine")],
        &[("GM_MUTATE_REFERENCE_DEGREE", "")],
        &[("GM_MUTATE_GRID_SPACING", "wide")],
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
        ["GM_MUTATE_REFERENCE_DEGREE", "GM_MUTATE_GRID_SPACING"]
    );
    assert_eq!(
        records,
        [
            "hashgate-control-reference-degree",
            "hashgate-control-grid-spacing"
        ]
    );
}

#[test]
fn the_stages_are_the_topology_then_every_registered_layout() {
    let layouts: Vec<&str> = graph_core::registry::LAYOUTS.iter().map(|l| l.id).collect();
    assert_eq!(STAGES[0], "topology");
    assert_eq!(STAGES[1..], layouts);
}

#[test]
fn stage_bytes_are_the_registered_pipeline_and_each_knob_moves_one_stage() {
    let [(t, topology), (l, layout)] = stage_bytes(4, &honest()).expect("runs");
    assert_eq!([t, l], STAGES);
    let grid = graph_core::registry::find(l).expect("registered");
    let (nodes, edges) = seeded_model(4, gate_node_count(4), REFERENCE_DEGREE);
    let registered = graph_core::run_with(&nodes, &edges, grid.id, grid.run).expect("runs");
    assert_eq!(
        (&registered.topology, registered.snapshot.to_bytes()),
        (&topology, layout.clone())
    );
    let degree = Setting {
        reference_degree: REFERENCE_DEGREE + 1,
        ..honest()
    };
    let [(_, moved), (_, kept)] = stage_bytes(4, &degree).expect("runs");
    assert_ne!(moved, topology);
    assert_eq!(kept, layout, "the grid ignores weights");
    let spacing = Setting {
        grid: GridParams { spacing: 2.0 },
        ..honest()
    };
    let [(_, kept), (_, moved)] = stage_bytes(4, &spacing).expect("runs");
    assert_eq!(kept, topology);
    assert_ne!(moved, layout);
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
    assert_eq!(
        prefixes,
        ["topology 0", "topology 1", "layout.grid 0", "layout.grid 1"]
    );
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
