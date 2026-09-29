mod stages;

use super::compare::{Tally, diverged, per_stage};
use super::transport;
use super::*;

/// Two seeds per stage; `fills[arm][line]` is the digest's repeated hex digit.
fn arms(fills: [[char; 6]; 4]) -> Vec<Arm> {
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

const HONEST: [[char; 6]; 4] = [['a', 'b', 'c', 'd', 'e', 'f']; 4];

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
    restaged[0].1[4] = restaged[0].1[4].replace(TRANSPORT, "layout.grid");
    assert!(diverged(2, &restaged).is_err());
}

#[test]
fn a_stage_whose_seeds_all_hash_alike_is_refused_as_one_input() {
    let err = diverged(2, &arms([['a', 'b', 'c', 'c', 'e', 'f']; 4])).expect_err("one digest");
    assert!(
        err.starts_with("layout.grid: every seed hashed to one digest"),
        "{err}"
    );
    let one: Vec<Arm> = arms(HONEST)
        .into_iter()
        .map(|(n, l)| (n, vec![l[0].clone(), l[2].clone(), l[4].clone()]))
        .collect();
    assert_eq!(diverged(1, &one), Ok(vec![]));
}

#[test]
fn per_stage_counts_equal_seeds_per_stage_and_distinct_bad_seeds() {
    let tally = per_stage(2, &[1, 3]);
    let mut equal = vec![1, 1];
    equal.resize(stages().len(), 2);
    assert_eq!(
        tally,
        Tally {
            equal,
            diverged_seeds: 1
        }
    );
    assert_eq!(per_stage(2, &[0, 5]).diverged_seeds, 2);
    assert_eq!(per_stage(2, &[]).equal, vec![2; stages().len()]);
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
fn the_stages_are_the_topology_then_every_registered_layout_then_the_transport() {
    let layouts: Vec<&str> = graph_core::registry::LAYOUTS.iter().map(|l| l.id).collect();
    let mut want = vec!["topology"];
    want.extend(layouts);
    want.push(TRANSPORT);
    assert_eq!(stages(), want);
}

#[test]
fn the_transport_tally_counts_the_seeds_where_the_real_abi_matches_the_shim() {
    let wasm = |fill: char| arms([[fill; 6]; 4]).remove(2).1;
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
