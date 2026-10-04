//! What sharding claims, and the one claim that matters: three shards' lines come back
//! **byte for byte** as the whole arm's.
//!
//! Every other test here is a refusal. The equivalence ones are the gate's real invariant
//! — a merge that put a line in the wrong slot would still produce lines that parse, and
//! `compare::diverged` would read them as the wrong seeds, so the arm would be compared
//! against the wrong thing and pass or fail for reasons nobody could see.

use super::{Shard, concurrent, gathered, merge, per_arm};
use crate::hashgate::tests::honest;
use crate::hashgate::{arm_lines, stages, threads_lines};

/// Every shard's lines, as the merge sees them: `arm_lines` prints one string, so it is
/// split on newlines the same way the child arms' stdout is.
fn shard_arm(seeds: u32, shards: u32) -> Result<Vec<Vec<String>>, String> {
    let setting = honest();
    gathered(concurrent(shards, |shard| {
        arm_lines(seeds, shard, &setting).map(|printed| {
            printed
                .lines()
                .map(str::to_owned)
                .collect::<Vec<String>>()
        })
    }))
}

#[test]
fn three_shards_merge_back_into_the_whole_arm() {
    let whole = arm_lines(7, Shard::WHOLE, &honest()).expect("the whole arm runs");
    let merged = merge(7, &stages(), &shard_arm(7, 3).expect("every shard runs")).expect("merges");
    assert_eq!(
        merged,
        whole.lines().map(str::to_owned).collect::<Vec<_>>(),
        "three shards must come back as the one arm, in the order the comparator reads"
    );
}

#[test]
fn three_shards_merge_back_into_the_whole_threaded_arm() {
    let setting = honest();
    let whole = threads_lines(7, Shard::WHOLE, &setting, 2).expect("the whole arm runs");
    let shards = gathered(concurrent(3, |shard| {
        threads_lines(7, shard, &setting, 2)
    }))
    .expect("every shard runs");
    assert_eq!(merge(7, &stages(), &shards).expect("merges"), whole);
}

/// The stride covers every seed exactly once across the shards, and each shard's own seeds
/// are ascending — the two properties the merge's correctness rests on.
#[test]
fn the_shards_partition_the_seeds() {
    let seeds = 11;
    let mut seen: Vec<u32> = Vec::new();
    for index in 0..4 {
        seen.extend(Shard { index, count: 4 }.seeds(seeds));
    }
    assert_eq!(seen, (0..seeds).collect::<Vec<u32>>());
    assert_eq!(
        Shard { index: 2, count: 3 }.seeds(11).collect::<Vec<u32>>(),
        vec![2, 5, 8],
        "a shard is a stride, not a slice: seed cost grows with the seed number"
    );
    assert!(Shard::WHOLE.seeds(3).eq(0..3));
}

/// A refused `--shard` names the text it refused: an arm that ran the wrong seeds, or none,
/// would read as an agreement.
#[test]
fn a_shard_that_cannot_exist_is_refused_by_name() {
    for (text, why) in [
        ("1/0", "K is 0"),
        ("0/0", "K is 0"),
        ("1/1", "not one of the"),
        ("7/2", "not one of the"),
        ("x/2", "not a whole number"),
        ("2/x", "not a whole number"),
        ("2", "expected i/K"),
        ("2/", "not a whole number"),
        ("", "expected i/K"),
        ("1/2/3", "not a whole number"),
        ("-1/2", "not a whole number"),
    ] {
        let err = Shard::parse(text).expect_err("refused");
        assert!(err.contains(text), "{text:?}: {err}");
        assert!(err.contains(why), "{text:?}: {err}");
    }
    assert_eq!(Shard::parse("0/1"), Ok(Shard::WHOLE));
    assert_eq!(Shard::parse("2/3").map(|s| s.to_string()), Ok("2/3".to_owned()));
}

/// A merge that lost a line, doubled one, or named a stage or seed the run does not have,
/// is refused by name — each of the four is a red gate waiting for a reader who does not
/// know which shard went wrong.
#[test]
fn a_merge_that_does_not_add_up_is_refused_by_name() {
    let stages = ["topology", "layout.grid"];
    let line = |stage: &str, seed: u32, fill: char| {
        format!("{stage} {seed} {}", fill.to_string().repeat(64))
    };
    let whole = vec![line("topology", 0, 'a'), line("topology", 1, 'a')];
    assert_eq!(merge(2, &stages, &[whole.clone()]).map(|m| m.len()), Ok(2));

    let missing = merge(2, &stages, &[whole[..1].to_vec()]).expect_err("a slot is empty");
    assert!(missing.contains("slot 1"), "{missing}");
    assert!(missing.contains("empty"), "{missing}");

    let doubled = merge(
        2,
        &stages,
        &[whole.clone(), vec![line("topology", 0, 'b')]],
    )
    .expect_err("two shards claim one seed");
    assert!(doubled.contains("slot 0"), "{doubled}");
    assert!(doubled.contains("filled twice"), "{doubled}");
    assert!(doubled.contains(&line("topology", 0, 'b')), "{doubled}");

    let unknown = merge(2, &stages, &[vec![line("layout.force.nope", 0, 'a')]]).expect_err("stage");
    assert!(unknown.contains("not in the gate's list"), "{unknown}");

    let past = merge(2, &stages, &[vec![line("topology", 2, 'a')]]).expect_err("seed 2");
    assert!(past.contains("outside the run's 2 seeds"), "{past}");

    let not_a_line = merge(2, &stages, &[vec!["topology 0".to_owned()]]).expect_err("short");
    assert!(not_a_line.contains("not \"<stage> <seed> <sha256>\""), "{not_a_line}");
    let no_seed = merge(2, &stages, &[vec!["topology x a".to_owned()]]).expect_err("seed word");
    assert!(no_seed.contains("not a seed number"), "{no_seed}");
}

/// The results come back in shard order, not completion order: a merge fed in
/// completion order would be a determinism hole, and this is what pins the order down.
#[test]
fn concurrent_collects_in_shard_order() {
    let results: Vec<Result<String, String>> =
        concurrent(4, |shard| format!("{shard}"));
    assert_eq!(
        results,
        vec![
            Ok("0/4".to_owned()),
            Ok("1/4".to_owned()),
            Ok("2/4".to_owned()),
            Ok("3/4".to_owned()),
        ]
    );
    // A count of zero is one shard, never zero: a stride of zero would not terminate.
    assert_eq!(concurrent(0, |shard| shard).len(), 1);
}

/// The shard count is a wall-clock knob and nothing else: whatever `available_parallelism`
/// says, the merged arm is the same list of lines.
#[test]
fn the_shard_count_is_between_one_and_eight() {
    let count = per_arm();
    assert!((1..=8).contains(&count), "per_arm() was {count}");
    let setting = honest();
    let one = merge(4, &stages(), &[arm_lines(4, Shard::WHOLE, &setting)
        .expect("runs")
        .lines()
        .map(str::to_owned)
        .collect()])
    .expect("merges");
    let four = merge(4, &stages(), &shard_arm(4, 4).expect("every shard runs")).expect("merges");
    assert_eq!(one, four, "the shard count must not be able to change what the arm says");
}