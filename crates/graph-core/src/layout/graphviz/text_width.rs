//! The rendered width of a node label, measured from the oracle image and pinned here.
//!
//! Graphviz sizes a node from its *rendered* label, so every coordinate the layered engine
//! produces downstream of ranking depends on a font metric the motor does not have. The
//! table below is that metric, measured rather than derived: one-node graphs, the node
//! attributes `width=0 margin=0`, read back from the plain format's own node line.
//!
//! ```text
//! docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
//!     dot -Tplain one-node.dot
//! ```
//!
//! The plain format prints **inches at five significant digits**, so the measurement's own
//! quantisation is 0.00001 in = 0.00072 pt. Every test below compares at that grid rather
//! than at a finer one, because a finer one would be comparing against digits the oracle
//! never printed.
//!
//! ## The shape of the table
//!
//! The measurement splits cleanly in two, and the two parts are separately exact. With the
//! node margin at zero and the shape forced to `box`, the printed width is the label's own
//! box and it is a whole number of points: **8 points for the first character and exactly 9
//! for each one after it**, verified for `k` = 1 to 8. With the default shape, the printed
//! width is the bounding box of the ellipse that circumscribes that padded label box, and
//! the ratio between the two series is constant:
//!
//! ```text
//! text_width_pt = (leading_box + 9 * (k - 1)) * ELLIPSE
//! ELLIPSE       = 1.130667
//! ```
//!
//! `ELLIPSE` is pinned to seven digits because that is the interval the measurements allow:
//! every inch string in the table is reproduced by every value in `[1.130661, 1.130670)`,
//! and nothing outside it reproduces all eight.
//!
//! The factor matters beyond tidiness. `docs/measurements/p13-gv2-dot.md` published the
//! advance as `10.1758`, which is `9 * ELLIPSE` truncated; truncated, it lands 0.40829 in at
//! k = 3, which the plain format prints as `0.40829`, while the oracle printed `0.4083`. The
//! same file's `9.04536` is `8 * ELLIPSE` rounded. Both are consistent with the
//! measurements *to within the oracle's own grid* and inconsistent with each other at four
//! decimal places, which is why this module keeps the two factors apart instead of storing
//! a base and an advance derived from them.
//!
//! Digits share `n`'s advance — measured, not assumed: `n0` = `nn` = 0.26696 in, `n10` =
//! `nnn` = 0.4083 in, `n100` = `n551` = `nnnn` = 0.54963 in. So for the 1000-seed fixture set
//! (`n0`..`n551`, lengths 2, 3 and 4) the rule covers every label the motor is fed.
//!
//! ponytail: a leading `1` is one point narrower than a leading `n` (7 points of box against
//! 8, so 0.10993 in against 0.12563 in) while advancing by the same 9 — a first-glyph side
//! bearing, not a per-character metric. Any character outside `n` and the ASCII digits is
//! given `n`'s leading box and the common advance and is **not** covered by measurement; a
//! non-ASCII id will be wrong by an unknown amount. The rule is verified for `k <= 8` only:
//! at `k = 9` the box series prints 79 points where 8 + 9 * 8 says 80, and the default-shape
//! series runs one point lower from there on, so a longer id is not covered. No fixture label
//! is longer than 4 characters, so nothing in the fixture set reaches that edge.
//!
//! ## Node width, and where this stops
//!
//! Node width is the next step, and it is *not* `max(NODE_W, text + 2 * 0.11 in)` in fact
//! even though that is the formula of record: for the same labels the oracle's default node
//! width is 0.75, 0.80475, 0.97719 and 1.1496 in for lengths 2, 3, 4 and 5, and the measured
//! relation is `node = 1.37952 * label_box + 0.30669` inches, the factor coming from the
//! ellipse circumscribing the label box *plus* the default margin. Node width is an input
//! only to the x-coordinate network simplex, which is the position pass and not the rank
//! pass, so the formula of record is implemented and the discrepancy is left here for the
//! position pass to settle.

use super::dot::NODE_W;

/// The label box of a leading character, in points, for every measured character but `1`.
const LEADING_BOX_PT: f64 = 8.0;

/// The label box of a leading `1`, in points: one narrower than every other measured glyph.
const LEADING_BOX_ONE_PT: f64 = 7.0;

/// The label box added by every character after the first, in points.
const BOX_ADVANCE_PT: f64 = 9.0;

/// The ratio of the default shape's bounding box to the label box, measured with the margin
/// at zero over `k` = 1 to 16. See the module's second section.
const ELLIPSE: f64 = 1.130667;

/// The default horizontal node margin, `0.11` inch, in points.
const MARGIN_PT: f64 = 0.11 * 72.0;

/// The rendered width of `label`, in points.
///
/// Only `n` and the ASCII digits are measured; see the module's caveat for what happens to
/// anything else.
pub fn text_width(label: &str) -> f64 {
    let mut chars = label.chars();
    let Some(first) = chars.next() else {
        return 0.0;
    };
    let rest = chars.count() as f64;
    (leading_box(first) + BOX_ADVANCE_PT * rest) * ELLIPSE
}

/// The node box width for a label of `text` points, in points: `max(0.75 in, text + 2 *
/// 0.11 in)`, the formula of record. See the module's last section — the oracle does not
/// agree with it above 0.75 in, and the position pass is where that is settled.
pub fn node_width(text: f64) -> f64 {
    (text + 2.0 * MARGIN_PT).max(NODE_W)
}

/// The measured label box of `c` as the label's first character. Unmeasured characters take
/// `n`'s box, which is the module's documented gap.
fn leading_box(c: char) -> f64 {
    if c == '1' {
        LEADING_BOX_ONE_PT
    } else {
        LEADING_BOX_PT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `k` copies of `n`, the run the table was measured on.
    fn run(k: usize) -> String {
        "n".repeat(k)
    }

    /// The plain format's own rendering of a value in inches: `%g` at five significant
    /// digits, trailing zeros stripped. Reproducing the oracle's formatter rather than
    /// rounding to a chosen number of decimals is what makes "exactly" checkable — the
    /// oracle's grid moves with the magnitude, so a fixed decimal count is wrong at both
    /// ends of the table.
    fn plain_g(inches: f64) -> String {
        let mut scaled = inches;
        let mut exponent = 0i32;
        while scaled < 1.0 {
            scaled *= 10.0;
            exponent -= 1;
        }
        while scaled >= 10.0 {
            scaled /= 10.0;
            exponent += 1;
        }
        let decimals = (4 - exponent).max(0) as usize;
        let text = format!("{inches:.decimals$}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }

    /// The inch string the plain format prints for a label of `k` characters.
    fn printed(k: usize) -> String {
        plain_g(text_width(&run(k)) / 72.0)
    }

    /// The oracle's quantisation in points: 0.00001 in.
    const PRINT_QUANTUM_PT: f64 = 0.00072;

    #[test]
    fn table_reproduces_every_measured_inch_string() {
        // (k, the string the oracle printed for `n` repeated k times)
        let measured = [
            (1, "0.12563"),
            (2, "0.26696"),
            (3, "0.4083"),
            (4, "0.54963"),
            (5, "0.69096"),
            (6, "0.8323"),
            (7, "0.97363"),
            (8, "1.115"),
        ];
        for (k, expected) in measured {
            assert_eq!(printed(k), expected, "k = {k}");
        }
    }

    #[test]
    fn the_label_box_is_whole_points_and_advances_by_nine() {
        // The measurement that splits the table: with `shape=box` and the margin at zero the
        // printed width is the label's own box, and it is a whole number of points. Nine
        // points is a whole number of points, and so is eight for the first character —
        // which is what the 0.14133 in apparent advance is: nine points seen through the
        // ellipse.
        assert!((text_width("n") / ELLIPSE - 8.0).abs() < 1e-9);
        assert!((text_width("nn") / ELLIPSE - 17.0).abs() < 1e-9);
        assert!((text_width("nnn") / ELLIPSE - 26.0).abs() < 1e-9);
        assert!((text_width("nnnnnnn") / ELLIPSE - 62.0).abs() < 1e-9);
    }

    #[test]
    fn table_matches_the_four_rows_in_the_measurements_file() {
        // `docs/measurements/p13-gv2-dot.md` states the text width of `n0`..`n9` as
        // 19.2211 pt, of `n10`..`n99` as 29.3976, of `n100`.. as 39.5734 and of `n1000` as
        // 49.7491. Those rows carry four decimals of a point, finer than the oracle's own
        // 0.00072 pt grid, so the rule is compared against them at that grid and no finer.
        let rows = [(2, 19.2211), (3, 29.3976), (4, 39.5734), (5, 49.7491)];
        for (k, expected) in rows {
            let got = text_width(&run(k));
            assert!(
                (got - expected).abs() <= PRINT_QUANTUM_PT,
                "k = {k}: {got} pt against {expected} pt, more than {PRINT_QUANTUM_PT} pt apart"
            );
        }
    }

    #[test]
    fn digits_share_the_advance_of_n() {
        // Measured on the oracle, not assumed: the fixture ids are `n` followed by digits,
        // and these are the comparisons that make the single advance safe to use for them.
        assert_eq!(plain_g(text_width("n0") / 72.0), printed(2));
        assert_eq!(plain_g(text_width("n9") / 72.0), printed(2));
        assert_eq!(plain_g(text_width("n10") / 72.0), printed(3));
        assert_eq!(plain_g(text_width("n99") / 72.0), printed(3));
        assert_eq!(plain_g(text_width("n100") / 72.0), printed(4));
        assert_eq!(plain_g(text_width("n551") / 72.0), printed(4));
    }

    #[test]
    fn a_leading_one_is_narrower_but_advances_the_same() {
        // 0.10993 in against 0.12563 in for `n`, and the same nine points after position 0.
        assert_eq!(plain_g(text_width("1") / 72.0), "0.10993");
        assert_eq!(plain_g(text_width("11") / 72.0), "0.25126");
        assert_eq!(plain_g(text_width("111") / 72.0), "0.39259");
    }

    #[test]
    fn an_empty_label_is_zero_wide() {
        assert_eq!(text_width(""), 0.0);
    }

    #[test]
    fn node_width_floors_at_the_default_box() {
        // 0.75 in is 54 pt; a two-character label plus two 0.11 in margins is 35.06 pt.
        assert_eq!(node_width(text_width("n0")), NODE_W);
        assert_eq!(node_width(0.0), NODE_W);
    }

    #[test]
    fn the_first_jobs_advance_does_not_reproduce_the_table() {
        // The negative control. `9.04536 + (k - 1) * 10.1758` is the base and advance the
        // first job published; 10.1758 is `9 * ELLIPSE` truncated, and truncated it lands
        // 0.40829 in at k = 3, which the plain format prints as `0.40829`, while the oracle
        // printed `0.4083`. Two wrong rules must not both pass this table.
        let wrong = |k: usize| 9.04536 + (k as f64 - 1.0) * 10.1758;
        assert_eq!(plain_g(wrong(3) / 72.0), "0.40829");
        assert_ne!(plain_g(wrong(3) / 72.0), printed(3));
        assert_ne!(plain_g(wrong(5) / 72.0), printed(5));
    }
}
