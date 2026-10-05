//! The twenty fixture seeds' **printed coordinates**, as the oracle wrote them.
//!
//! Split out of `position_tests.rs` by the house 300-line limit, and by the same split
//! `rank_fixture_edges.rs` records: a fixture table is data, so it lives in a file of its own
//! and nothing in it is derived or hand-edited to make a test pass. Each row is `(seed, x, y)`
//! where `x` and `y` are the inch strings `-Tplain` printed per node, in node order
//! `n0`, `n1`, ..., **verbatim** — no rescale, no re-round — because the comparison downstream is
//! byte for byte at the plain format's five significant digits.
//!
//! Reproduce the table (the command is also in `position_tests.rs`'s header):
//!
//! ```sh
//! scripts/orch/gr cargo run -q -p graph-cli --release -- \
//!     emit-graphviz-fixtures --engine twopi --seeds 20 --out target/dot-pos20
//! cp target/dot-pos20/twopi.jsonl target/dotfix/dot.jsonl
//! scripts/orch/drun --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
//!     python3 harness/oracle-dot-probe.py target/dotfix --fixtures=dot.jsonl \
//!     --digest=target/probe/dot20.txt
//! ```

use super::oracle_probe::{centres, positioned};
use super::position_tests::{inch_columns, nodes_of};
use super::rank_fixture_edges::all as fixture_edges;

/// The twenty fixture seeds, `n` = 2 to 21, with the inch strings the oracle printed.
///
/// **Written from the oracle's output, not computed.** The command is in this file's header;
/// the strings are what `-Tplain` printed, kept verbatim so the comparison cannot be looser
/// than the measurement they come from.
pub const FIXTURES: &[(u32, &str, &str)] = &[
    (0, "0.375 0.375", "0.25 1.25"),
    (1, "1.375 0.375 1.375", "0.25 1.25 1.25"),
    (2, "1.5139 0.375 1.9028 1.5139", "0.25 1.25 1.25 2.25"),
    (
        3,
        "1.2639 0.76389 1.7639 2.7639 0.375",
        "0.25 1.25 1.25 1.25 2.25",
    ),
    (
        4,
        "1.8056 1.3056 0.375 2.3056 3.3056 1.375",
        "0.25 1.25 2.25 1.25 1.25 2.25",
    ),
    (
        5,
        "2.1528 0.625 2.1528 3.1528 4.1528 0.375 1.6389",
        "0.25 1.25 1.25 1.25 1.25 2.25 2.25",
    ),
    (
        6,
        "2.875 0.375 1.375 2.375 3.375 0.375 1.375 4.375",
        "0.25 1.25 1.25 1.25 1.25 2.25 2.25 1.25",
    ),
    (
        7,
        "3.1528 0.625 2.1528 3.1528 4.1528 0.375 1.6389 5.1528 6.1528",
        "0.25 1.25 1.25 1.25 1.25 2.25 2.25 1.25 1.25",
    ),
    (
        8,
        "3.375 1.375 3.375 4.375 1.375 2.375 5.375 6.375 0.375 2.375",
        "0.25 1.25 1.25 1.25 2.25 1.25 1.25 1.25 2.25 2.25",
    ),
    (
        9,
        "2.375 1.375 0.375 2.375 1.375 0.375 3.375 4.0139 1.75 2.9028 4.9306",
        "0.25 1.25 1.25 1.25 2.25 2.25 1.25 2.25 3.25 2.25 1.25",
    ),
    (
        10,
        "3.4861 1.4306 3.4861 0.40278 4.4861 3.4861 5.4861 6.4861 7.4861 0.375 2.4583 1.4028",
        "0.25 1.25 1.25 1.25 1.25 1.25 2.25 1.25 1.25 1.25 2.25 2.25 2.25",
    ),
    (
        11,
        "4.375 2.375 4.375 1.375 3.375 5.375 6.375 0.375 4.4306 2.375 7.4028 3.4028 4.4306",
        "0.25 1.25 1.25 2.25 1.25 1.25 1.25 2.25 2.25 2.25 1.25 2.25 3.25",
    ),
    (
        12,
        "3.1389 2.375 1.375 0.375 3.2639 3.9028 4.9028 5.9028 0.86111 5.2917 2.2361 5.8194 3.2639 5.8194",
        "0.25 1.25 1.25 1.25 2.25 1.25 1.25 1.25 2.25 2.25 2.25 3.25 3.25 4.25",
    ),
    (
        13,
        "5.1667 2.6389 5.1667 1.375 4.1667 6.1667 7.1667 0.375 4.4306 2.375 8.1944 3.4028 4.4306 9.25 5.4583",
        "0.25 1.25 1.25 2.25 1.25 1.25 1.25 2.25 2.25 2.25 1.25 2.25 3.25 1.25 2.25",
    ),
    (
        14,
        "3.6894 3.3144 5.8421 1.6755 2.6199 6.8421 4.8421 7.8421 1.6199 4.6755 3.6477 0.68936 2.6199 0.68936 4.5505 0.59213",
        "0.25 1.25 1.25 1.25 2.25 1.25 1.25 1.25 2.25 2.25 2.25 3.25 3.25 4.25 3.25 2.25",
    ),
    (
        15,
        "4.3472 2.8472 4.8472 5.8472 3.8472 0.375 0.81944 3.875 1.8194 6.8472 2.8472 3.875 7.875 4.9028 7.875 0.44444 5.9583",
        "0.25 1.25 1.25 1.25 1.25 1.25 2.25 2.25 2.25 1.25 2.25 3.25 1.25 2.25 2.25 3.25 2.25",
    ),
    (
        16,
        "5.7079 4.1802 5.7079 1.5968 7.7079 8.7079 6.7079 6.1802 5.1802 4.1802 2.5135 1.4579 2.5135 8.2218 0.40237 6.944 3.1524 5.8885",
        "0.25 1.25 1.25 2.25 1.25 1.25 1.25 2.25 2.25 2.25 3.25 3.25 4.25 2.25 3.25 3.25 2.25 3.25",
    ),
    (
        17,
        "5.3052 4.5413 6.069 2.5135 0.45793 6.569 2.5135 3.5135 7.069 5.569 2.5135 8.1107 0.40237 8.1246 1.4579 1.4857 6.444 4.5413 3.569",
        "0.25 1.25 1.25 1.25 1.25 2.25 2.25 2.25 2.25 1.25 2.25 3.25 1.25 3.25 2.25 3.25 2.25 3.25 2.25 3.25",
    ),
    (
        18,
        "6.9306 6.4167 4.875 1.4028 6.5694 2.4028 9.5278 0.375 8.0972 10.528 2.4583 3.4306 11.556 4.4861 6.5694 1.4028 7.4444 5.5417 7.8611 9.125",
        "0.25 1.25 1.25 1.25 2.25 2.25 2.25 1.25 3.25 2.25 1.25 3.25 2.25 1.25 2.25 3.25 3.25 1.25 2.25 3.25 2.25",
    ),
    (
        19,
        "5.6959 2.7654 5.8209 3.3626 7.4182 3.7654 2.8487 1.7654 6.9876 2.7654 2.8487 8.5432 7.4182 8.5432 4.3626 3.8765 3.7793 0.73761 2.2376 4.7932 5.7932",
        "0.25 1.25 2.25 4.25 5.25 2.25 5.25 2.25 1.25 2.25 6.25 1.25 6.25 2.25 6.25 5.25 3.25 2.25 3.25 2.25 3.25",
    ),
];

/// The twenty fixture seeds against the oracle, byte for byte at the printed precision.
///
/// **This asserts a count, not twenty successes.** The table above is the oracle's, all twenty
/// rows of it, and the pass reproduces [`AGREEING_SEEDS`] of them exactly; the rest are recorded
/// disagreements, each named by its seed in the failure message, and the cause is measured in
/// [`the_disagreements_are_chain_dummy_slots`]. A test that asserted twenty successes would be
/// asserting something untrue, and a test that asserted nothing would be worthless.
#[test]
pub fn the_first_twenty_fixture_seeds_are_placed_as_the_oracle_places_them() {
    let (agreed, seeds) = sweep_fixtures();
    eprintln!("{agreed} of 20 fixture seeds agree node for node; the rest: {seeds:?}");
    assert_eq!(
        agreed, AGREEING_SEEDS,
        "seeds placed exactly as the oracle places them"
    );
}

/// How many of the twenty the pass places byte for byte as the oracle places them, measured.
pub const AGREEING_SEEDS: usize = 5;

/// Every fixture seed, and whether the pass places it exactly as the oracle does.
pub fn sweep_fixtures() -> (usize, Vec<u32>) {
    let mut agreed = 0;
    let mut rest = Vec::new();
    for (seed, edges) in fixture_edges() {
        let (_, want_x, want_y) = FIXTURES
            .iter()
            .find(|(s, ..)| *s == seed)
            .unwrap_or_else(|| panic!("seed {seed} has no printed table"));
        let count = nodes_of(edges);
        let got = inch_columns(&centres(&positioned(count, edges)));
        if got == (want_x.to_string(), want_y.to_string()) {
            agreed += 1;
        } else {
            rest.push(seed);
        }
    }
    (agreed, rest)
}
