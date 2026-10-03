//! The C20 tally's refusals, which are the check: a seed the arm printed **twice** cannot
//! be tallied, because taking the first of the two digests would let a later, differing
//! digest for the same seed pass as agreement — the exact failure the tally exists to catch.

use super::{LAYOUT, TRANSPORT, agree_with_shim};

/// A tally-clean arm: the two stages the tally reads, `seeds` lines each, every digest
/// `fill.to_string().repeat(64)`.
fn wasm_arm(seeds: usize, fill: char) -> Vec<String> {
    [TRANSPORT, LAYOUT]
        .iter()
        .flat_map(|stage| (0..seeds).map(move |seed| line(stage, seed, fill)))
        .collect()
}

/// One `stage seed <64 hex>` line, the shape both `compare::diverged` and the tally read.
fn line(stage: &str, seed: usize, fill: char) -> String {
    format!("{stage} {seed} {}", fill.to_string().repeat(64))
}

/// **A duplicated `(stage, seed)` line is refused, for each stage the tally tallies.**
///
/// Both stages matter: the tally reads `TRANSPORT` against `LAYOUT`, so a repeat of either
/// one leaves the count resting on a choice the arm never stated. The message names the
/// stage, the seed and the word `twice`, and is pinned verbatim below so it cannot drift
/// into something a reader has to guess at.
#[test]
fn a_duplicate_stage_seed_line_is_refused() {
    for stage in [TRANSPORT, LAYOUT] {
        let mut arm = wasm_arm(2, 'a');
        assert_eq!(
            agree_with_shim(2, &arm),
            Ok(2),
            "the clean arm agrees on both seeds"
        );

        arm.push(line(stage, 1, 'b'));
        let err = agree_with_shim(2, &arm).expect_err("the duplicate must be refused");
        assert!(err.contains(stage), "names the stage: {err:?}");
        assert!(err.contains("seed 1"), "names the seed: {err:?}");
        assert!(
            err.contains("twice"),
            "says the pair was printed twice: {err:?}"
        );
        assert_eq!(
            err,
            format!(
                "the wasm arm printed {stage} for seed 1 twice: \
                 one (stage, seed) pair carries one digest"
            ),
            "the message is pinned so it cannot drift"
        );
    }
}

/// The refusal that predates this one still holds, for both stages: a seed the arm never
/// printed is a seed the tally cannot read, and guessing which line was meant is how a
/// tally starts counting an input it never saw.
#[test]
fn a_missing_line_is_still_refused() {
    let missing = |stage: &str| {
        let mut arm = wasm_arm(2, 'a');
        arm.retain(|line| !line.starts_with(&format!("{stage} 1 ")));
        assert_eq!(arm.len(), 3, "one line short of the clean arm");
        agree_with_shim(2, &arm)
    };
    assert_eq!(
        missing(TRANSPORT),
        Err("the wasm arm printed no transport.wasm.columnar line for seed 1".to_string())
    );
    assert_eq!(
        missing(LAYOUT),
        Err("the wasm arm printed no layout.grid line for seed 1".to_string())
    );
}

/// A tally over no seeds proves nothing, and is refused before the arm is even read.
#[test]
fn zero_seeds_is_still_refused() {
    assert!(agree_with_shim(0, &[]).is_err());
}

/// The happy path, on an arm whose digests differ from seed to seed: the count is the
/// number of seeds the two stages agreed on, so all of them is `Ok(seeds)`.
#[test]
fn agreeing_arms_tally_every_seed() {
    let fills = ['a', 'b', 'c'];
    let arm: Vec<String> = [TRANSPORT, LAYOUT]
        .iter()
        .flat_map(|stage| (0..fills.len()).map(move |seed| line(stage, seed, fills[seed])))
        .collect();
    let seeds = fills.len() as u32;
    assert_eq!(agree_with_shim(seeds, &arm), Ok(seeds));
}

/// A seed where the stages *do* differ is tallied as a miss, not as a refusal: counting
/// those is the tally's job, and only an arm that cannot be read is an error.
#[test]
fn a_diverged_seed_is_counted_not_refused() {
    let mut arm = wasm_arm(2, 'a');
    arm.retain(|line| !line.starts_with(&format!("{TRANSPORT} 1 ")));
    arm.push(line(TRANSPORT, 1, 'b'));
    assert_eq!(
        agree_with_shim(2, &arm),
        Ok(1),
        "seed 1 disagreed and is counted short"
    );
}
