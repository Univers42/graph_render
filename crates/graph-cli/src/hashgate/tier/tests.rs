//! The tier list's own rules: what `--tiers` accepts, what it adds, and the arm names the
//! report prints. No run here spawns a thread or hashes a seed — these are the claims that
//! the *list* is what it says it is, which is the part a wrong edit would break quietly.

use super::{Tiers, WORKER_COUNTS, parse};
use graph_core::exec::{Caps, Thresholds, Tier as Selected, resolve, select};

#[test]
fn the_two_tier_lists_parse_and_nothing_else_does() {
    assert_eq!(parse("base"), Ok(Tiers::Base));
    assert_eq!(parse("all"), Ok(Tiers::All));
    for wrong in ["", "ALL", "threads", "simd", "1", " base"] {
        let err = parse(wrong).expect_err(wrong);
        assert!(
            err.starts_with(&format!("--tiers {wrong:?}")),
            "the error must quote what was typed: {err}"
        );
        assert!(err.contains("base") && err.contains("all"), "{err}");
    }
}

/// `all` adds one arm per worker count and the scalar reference, so its arm names are
/// pinned: a report that said "threads" without a count would not say which plan ran, and
/// a name that silently changed would make two runs' reports incomparable.
#[test]
fn the_worker_counts_and_their_arm_names_are_pinned() {
    assert_eq!(WORKER_COUNTS, [1, 2, 3, 4, 7]);
    let names: Vec<&str> = WORKER_COUNTS
        .iter()
        .map(|w| super::Tier::Threads(*w).arm_name())
        .collect();
    assert_eq!(
        names,
        [
            "native threads 1",
            "native threads 2",
            "native threads 3",
            "native threads 4",
            "native threads 7"
        ]
    );
    assert_eq!(super::Tier::Scalar.arm_name(), "native scalar");
    // A count the gate does not run still has to name itself, not borrow another's.
    assert_eq!(super::Tier::Threads(5).arm_name(), "native threads other");
}

/// The odd counts are the point, and this is the assertion that keeps them: 3 and 7 are
/// what make a boundary bug visible, so a table that dropped them would still pass every
/// even-split test.
#[test]
fn the_worker_counts_include_the_odd_ones_and_stop_at_seven() {
    assert!(WORKER_COUNTS.contains(&3) && WORKER_COUNTS.contains(&7));
    assert!(
        !WORKER_COUNTS.contains(&8),
        "the table is the gate's, not the host's"
    );
    assert_eq!(
        WORKER_COUNTS.len(),
        5,
        "one scalar + five counts is the whole set"
    );
}

/// The tiers the gate runs and the tiers `select` may choose are the same vocabulary, and
/// the gate's worker counts must be reachable by a `threads` selection — otherwise a
/// threshold could promote a width the gate never tested. This is the tie between the two
/// lists, and it is why it is a test rather than a comment.
#[test]
fn every_gated_worker_count_is_one_select_could_choose() {
    let thresholds = Thresholds {
        simd_nodes: u64::MAX,
        threads_nodes: 1,
        threads_max: 7,
    };
    let caps = Caps {
        simd: false,
        workers: 7,
        webgpu: false,
    };
    let chosen = match select(10, 0, caps, thresholds) {
        Selected::Threads(workers) => workers,
        other => panic!("expected threads at 7, got {other:?}"),
    };
    assert!(
        WORKER_COUNTS.contains(&chosen),
        "select chose {chosen} workers, which the gate never runs"
    );
    // And an explicit request for a gated width resolves rather than being refused, so a
    // host that names the tier gets it (select's refusal rule is about *availability*,
    // not about whether the gate has an arm for it).
    for workers in WORKER_COUNTS {
        assert_eq!(
            resolve(
                graph_core::exec::Exec::Named(Selected::Threads(workers)),
                10,
                0,
                caps,
                thresholds
            ),
            Ok(Selected::Threads(workers))
        );
    }
}
