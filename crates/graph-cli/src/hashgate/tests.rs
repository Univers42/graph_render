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
    restaged[0].1[2] = restaged[0].1[2].replace("topology", "synthetic");
    assert!(diverged(2, &restaged).is_err());
}

#[test]
fn a_stage_whose_seeds_all_hash_alike_is_refused_as_one_input() {
    let err = diverged(2, &arms([['a', 'b', 'c', 'c']; 4])).expect_err("one digest");
    assert!(
        err.starts_with("topology: every seed hashed to one digest"),
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

#[test]
fn the_mutation_variable_parses_strictly() {
    use std::env::VarError;
    assert_eq!(
        parse_reference_degree(Err(VarError::NotPresent)),
        Ok(graph_core::REFERENCE_DEGREE)
    );
    assert_eq!(parse_reference_degree(Ok(" 9 ".into())), Ok(9));
    assert!(parse_reference_degree(Ok("nine".into())).is_err());
    assert!(parse_reference_degree(Ok(String::new())).is_err());
}

#[test]
fn stage_bytes_knows_both_stages_and_refuses_others() {
    let degree = graph_core::REFERENCE_DEGREE;
    assert_eq!(
        stage_bytes("synthetic", 1, degree),
        graph_core::synthetic_snapshot(1, degree).map_err(|e| e.to_string())
    );
    assert_eq!(
        stage_bytes("topology", 1, degree),
        graph_core::topology_stage(1, degree).map_err(|e| e.to_string())
    );
    assert!(stage_bytes("layout", 1, degree).is_err());
}
