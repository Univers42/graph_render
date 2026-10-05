//! The two measured causes of the position pass's disagreements with the oracle.
//!
//! Split out of `position_tests.rs` by the house 300-line limit. Both are **findings with their
//! seeds**, not notes: each names the seeds it accounts for, and each is pinned so that a change
//! which moves one of them is a change somebody has to look at. `docs/measurements/p13-gv2-dot.md`'s
//! "Position" section records both with the commands that measured them.

use super::oracle_probe::positioned;
use super::position_fixture_points::{FIXTURES, sweep_fixtures};

/// **The width table is the default box for every fixture label of two or three characters, and
/// that is the first of the two causes of the disagreements above.** This is the measurement, and
/// it is a finding rather than a note: `node_width(text_width(id))` is the *formula of record*,
/// `max(0.75 in, text + 2 * 0.11 in)`, and for `n0`…`n99` it returns exactly 54 points — the
/// default box — while the oracle prints 0.80475 in = 57.942 for `n10`. So every seed from 9 on,
/// whose ids reach three characters, is drawn with boxes three points too narrow, and no fixture
/// seed of the twenty can tell the two apart.
///
/// The relation that *does* reproduce the oracle's four node-width rows is the one
/// `text_width.rs`'s own module doc names, `node = 1.37952 * label_box + 0.30669` inches, with
/// `label_box` the un-ellipsed box. It is checked here against those rows so the finding carries
/// the numbers and not an opinion — but it is not used, because the job's contract names
/// `node_width(text_width(id))` as the only source of node widths and `text_width.rs` is outside
/// this change. Settling it is one constant pair in that module.
#[test]
fn the_width_table_is_the_default_box_below_four_characters_and_the_oracle_is_wider() {
    use crate::layout::graphviz::text_width::{node_width, text_width};
    // What this pass uses, against what the oracle printed for the same labels.
    for (id, oracle_inches) in [("n9", "0.75"), ("n10", "0.80475"), ("n100", "0.97719")] {
        let ours = node_width(text_width(id));
        let theirs = parse_inches(oracle_inches) * 72.0;
        if id == "n9" {
            assert_eq!(theirs, ours, "{id}: two characters, and the two agree");
        } else if id == "n100" {
            assert!(
                ours > crate::layout::graphviz::dot::NODE_W && ours < theirs,
                "{id}: {ours} pt is above the default box and still short of {theirs}"
            );
        } else {
            assert_eq!(
                ours,
                crate::layout::graphviz::dot::NODE_W,
                "{id}: the default box"
            );
            assert!(
                theirs - ours > 3.0,
                "{id}: and the oracle's own box is {theirs} pt, which is not it"
            );
        }
    }
    // The measured relation the module's own doc names, checked against the same three rows.
    for (id, oracle_inches) in [("n9", "0.75"), ("n10", "0.80475"), ("n100", "0.97719")] {
        let label_box = text_width(id) / ELLIPSE;
        let inches = (1.37952 * (label_box / 72.0) + 0.30669).max(0.75);
        let theirs = parse_inches(oracle_inches);
        assert!(
            (inches - theirs).abs() <= RELATION_TOLERANCE,
            "{id}: {inches} in against the oracle's {theirs}"
        );
    }
}

/// One of the oracle's own printed inch values, read back as the number it stands for.
fn parse_inches(inches: &str) -> f64 {
    inches.parse().expect("the table's inches parse")
}

/// How closely the measured node-width relation reproduces the oracle's rows: within two printed
/// digits at that magnitude. The relation's own two constants are given to five significant
/// digits (`1.37952` and `0.30669`), so it cannot do better than that and the check does not
/// pretend to.
const RELATION_TOLERANCE: f64 = 0.0002;

/// The plain format's own quantisation at `value`: five significant digits, so the grid moves
/// with the magnitude — 0.00001 inch below one inch, 0.0001 above it. Nothing in this file
/// compares against the oracle more finely than the oracle printed, which is why the tolerance
/// is a function of the value rather than one constant.
fn printed_quantum(value: f64) -> f64 {
    let mut magnitude = value.abs();
    let mut exponent = 0i32;
    while magnitude >= 10.0 {
        magnitude /= 10.0;
        exponent += 1;
    }
    while magnitude < 1.0 {
        magnitude *= 10.0;
        exponent -= 1;
    }
    let mut quantum = 1.0 / 10_000.0;
    for _ in 0..exponent {
        quantum *= 10.0;
    }
    for _ in 0..exponent.unsigned_abs() {
        quantum /= 10.0;
    }
    quantum
}

/// `text_width.rs`'s measured ellipse factor, restated so the relation above can be recomputed
/// from the rendered width without reaching into that module's private constant.
const ELLIPSE: f64 = 1.130667;

/// The negative control for the twenty above: seeds 0 to 8 disagree for a different reason from
/// seeds 9 to 19.
///
/// Seeds 0 to 8 have every label two characters long, so their boxes *are* the default box and
/// the width table cannot be the cause — and four of them still disagree. The cause there is a
/// chain dummy's slot inside its rank, which [`the_disagreements_are_chain_dummy_slots`]
/// isolates. So the twenty split 5 / 4 / 11 across three outcomes, and a test that only counted
/// agreements would not know which cause it was looking at.
#[test]
fn the_disagreements_split_by_whether_the_ids_reach_three_characters() {
    let (_, rest) = sweep_fixtures();
    let short: Vec<u32> = rest.iter().copied().filter(|&s| s < 9).collect();
    let long: Vec<u32> = rest.iter().copied().filter(|&s| s >= 9).collect();
    assert_eq!(
        short, DUMMY_SLOT_SEEDS,
        "two-character ids: dummy slots only"
    );
    assert_eq!(
        long.len(),
        11,
        "three-character ids: every one of them, the width table"
    );
    assert_eq!(short.len() + long.len() + 5, 20, "and 5 agree");
}

/// The seeds whose only cause of disagreement is a chain dummy's slot inside its rank.
const DUMMY_SLOT_SEEDS: [u32; 4] = [2, 4, 5, 7];

/// The disagreements above are a chain dummy's slot inside its rank, and the existing order sweep
/// cannot see it.
///
/// Seed 2 is `n1 -- n0, n2 -- n0 (twice), n3 -- n2, n3 -- n0`: ranks `n3 | n1 n2 | n0`, and the
/// two-rank edge `n3 -- n0` puts a chain dummy on the middle rank beside `n1` and `n2`. The
/// oracle draws `n1` and `n2` 110 points apart and this port 72. **110 is the two constraint
/// lengths of a row ordered `n1, dummy, n2`** — `27 + 10 + 18` and `10 + 27 + 18`, the dummy
/// being a one-point box widened by `nodesep / 2` on each side — and 72 is the single constraint
/// of a row ordered `n1, n2, …`. So the oracle puts the dummy between the two nodes and this port
/// puts it after both.
///
/// The rank and order sweeps both call seed 2 an agreement because they compare the *real* nodes
/// of a rank, and a dummy is not one. That is the whole of the gap between "408 of 692 seeds
/// agree on every rank's order" and the position sweep's count: an order of the real nodes can
/// agree while the row the x constraints read does not.
#[test]
fn the_disagreements_are_chain_dummy_slots_inside_a_rank() {
    let edges = &[(1, 0), (2, 0), (2, 0), (3, 2), (3, 0)];
    let g = positioned(4, edges);
    let xs: Vec<f64> = (0..4).map(|n| g.nodes[n].coord.x).collect();
    let ours = xs[2] - xs[1];
    assert_eq!(
        ours, 72.0,
        "one constraint of two 27-point boxes plus nodesep"
    );
    // The oracle's row for seed 2, from the pinned table above: 1.9028 - 0.375 inch.
    let (_, want_x, _) = FIXTURES
        .iter()
        .find(|(s, ..)| *s == 2)
        .expect("seed 2 is pinned");
    let column: Vec<f64> = want_x.split_whitespace().map(parse_inches).collect();
    let theirs_in = column[2] - column[1];
    let two_constraints_in = ((27.0 + 10.0 + 18.0) + (10.0 + 27.0 + 18.0)) / 72.0;
    assert!(
        (theirs_in - two_constraints_in).abs() <= printed_quantum(theirs_in),
        "the oracle's {theirs_in} in is the row n1, dummy, n2: two constraints of \
         {two_constraints_in} in"
    );
    assert_ne!(
        ours,
        theirs_in * 72.0,
        "and this port's row is the other one"
    );
}
