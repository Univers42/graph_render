//! The comparison itself: N-way [`diverged`], the per-stage [`Tally`], and the inputs
//! that are **refused** because a comparison that cannot fail is not a check.

use super::super::compare::{Arm, Tally, diverged, per_stage};
use super::super::stages::stages as stage_ids;

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
            stage_ids()[i / 2],
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
/// has grown past that, so these tests exercise [`diverged`] and [`per_stage`] at this
/// small, fixed size, taken from the front of the real list so the stage names printed
/// still match production.
fn test_stages() -> Vec<&'static str> {
    stage_ids()[..3].to_vec()
}

#[test]
fn agreeing_arms_have_no_divergence() {
    assert_eq!(diverged(2, &test_stages(), &arms(HONEST)), Ok(vec![]));
}

/// The N-way claim: any number of arms compares, and **one** of them differing is caught
/// however late in the list it sits.
///
/// This is the gate step 1 makes about tiers — `--tiers all` runs ten arms, and the fifth
/// threaded arm diverging must be as visible as the first native one diverging. A rule
/// that only compared the first four would pass it silently.
#[test]
fn any_arm_of_any_count_diverging_is_caught_wherever_it_sits() {
    let all = ten_arms();
    assert_eq!(diverged(2, &test_stages(), &all), Ok(vec![]));
    for i in 0..all.len() {
        assert_arm_diverges(&all, i, 3);
    }
    // Two arms differing on different lines are both reported, not just the first.
    let mut two = all.clone();
    two[1].1[0] = broken_line(0, "1");
    two[9].1[5] = broken_line(5, "2");
    assert_eq!(diverged(2, &test_stages(), &two), Ok(vec![0, 5]));
    // Ten arms = 4 base + 1 scalar + 5 worker counts. Pinned so adding a tier is
    // a deliberate edit, not a silent change in the gate's width.
    assert_eq!(all.len(), 4 + 1 + super::tier::WORKER_COUNTS.len());
    assert!(
        ten_names().contains(&"native threads 7"),
        "odd count must be in the run"
    );
}

/// Returns the 10 arm names used by `--tiers all`.
fn ten_names() -> [&'static str; 10] {
    [
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
    ]
}

/// Builds the 10-arm vector where every arm has honest digests.
fn ten_arms() -> Vec<Arm> {
    let honest = arms(HONEST)[0].1.clone();
    ten_names().iter().map(|n| (*n, honest.clone())).collect()
}

/// Creates a diverging line for line index `i` (stage `i/2`, seed `i%2`).
fn broken_line(i: usize, fill: &str) -> String {
    format!("{} {} {}", test_stages()[i / 2], i % 2, fill.repeat(64))
}

/// Asserts that arm `idx` diverging on line `line` is caught.
fn assert_arm_diverges(all: &[Arm], idx: usize, line: usize) {
    let mut broken = all.to_vec();
    broken[idx].1[line] = broken_line(line, "0");
    assert_eq!(
        diverged(2, &test_stages(), &broken),
        Ok(vec![line]),
        "{} diverged and was not caught",
        all[idx].0
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
