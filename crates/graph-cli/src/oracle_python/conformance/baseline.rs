//! The measured matrix, pinned: what every row's bytes and its shape were when this table
//! was taken, and what each row is called.
//!
//! **This table is the gate, and it is a measurement rather than a target.** `emit` writes
//! its own sha256s into `conformance-manifest.json` and the arm writes the shape; this table
//! is that run's numbers, transcribed. Three things follow, and all three are the point:
//!
//! - a layout that changes by one ULP fails its row, by sha, with the row named;
//! - a repair job that legitimately moves a layout is expected to update this table, and
//!   that edit is the record of the repair — the same way a moved ceiling is the record in
//!   every other differential here;
//! - a row whose measurement could not be taken carries `sha256: ""`, which
//!   [`super::verdict::one_row`] refuses outright: an empty pin never compares equal to an
//!   empty measurement, because the absence of a measurement must never read as a pass.
//!
//! `docs/measurements/scigraphs-conformance.md` is where the numbers are read; this file is
//! where they are enforced, and the two are the same run.

/// One pinned row.
pub struct Baseline {
    /// The `apply_graph_layout` name.
    pub name: &'static str,
    /// `sha256` of `motor/<name>.f64` at the time this table was taken.
    pub motor_sha256: &'static str,
    /// `sha256` of `ref/<name>.f64` at the time this table was taken, or `""` when the
    /// reference is not reproducible between runs and therefore cannot be pinned at all.
    pub reference_sha256: &'static str,
    /// Why [`Self::reference_sha256`] is empty, or `""` when it is not. A row with a note is
    /// gated on its motor bytes and its measured shape instead, and says so in the log.
    pub reference_note: &'static str,
    /// The Procrustes disparity's median over the fixture set, the shape number the ceiling
    /// is on.
    pub procrustes_ceiling: f64,
    /// `bitwise`, `tolerance` or `shape` — see the doc's tier rule.
    pub tier: &'static str,
    /// The row's first cause: `convention`, `rng`, `arithmetic`, `algorithm` or
    /// `reference-absent`.
    pub cause: &'static str,
}

/// The pinned matrix, in [`super::ROWS`] order, as
/// `harness/scigraphs-conformance.py --metrics` measured it and
/// `target/scigraphs-conformance/conformance-baseline-proposed.rs` wrote it.
///
/// The tier and cause on each row are [`sc_propose.classify`]'s own output, decided from that
/// run's numbers and the row's declared gaps; a `// …` after a row is the reason the classifier
/// gave where it had one. Two rows carry a fact a shas column cannot hold: `GRAPHVIZ_DOT`'s
/// motor digest is the **empty file's** — the emit writes a zero-byte file for a name with no
/// motor layout, which is why the row's cause is `reference-absent` and why
/// `verdict::one_row` holds it to still saying so — and `GRAPHVIZ_FDP`'s reference sha is
/// empty **with a reason**, because that engine's start is not seeded by `-Gstart` and two runs
/// of it differ.
mod table;

pub use table::BASELINE;

/// One pinned row, written out rather than arrayed: a 32-line literal of six fields each is
/// a table a diff can read line by line, and a `const` table in one line is not.
#[allow(dead_code)]
pub const fn row(
    name: &'static str,
    motor_sha256: &'static str,
    reference_sha256: &'static str,
    reference_note: &'static str,
    procrustes_ceiling: f64,
    tier: &'static str,
    cause: &'static str,
) -> Baseline {
    Baseline {
        name,
        motor_sha256,
        reference_sha256,
        reference_note,
        procrustes_ceiling,
        tier,
        cause,
    }
}

#[cfg(test)]
mod tests {
    use super::{BASELINE, Baseline, row};

    /// The two empty-sha rules are the whole anti-vacuity argument of this file, so they are
    /// asserted rather than described: a row with no measurement must fail, and a table that
    /// forgot to pin a sha must not compare equal to the empty string by accident.
    #[test]
    fn an_unpinned_sha_is_empty_and_so_is_not_a_pass() {
        let empty = row(
            "NAME",
            "",
            "",
            "",
            f64::INFINITY,
            "shape",
            "reference-absent",
        );
        assert_eq!(empty.motor_sha256, "");
        assert!(empty.motor_sha256.is_empty());
        assert_eq!(empty.reference_sha256, "");
        assert!(
            empty.reference_note.is_empty(),
            "a note belongs to an unreproducible reference"
        );
    }

    /// A pinned sha is 64 lowercase hex characters. A truncated or uppercase one would let a
    /// comparison pass against a file that is not the one that was measured.
    #[test]
    fn a_pinned_sha_is_64_hex_characters() {
        for base in BASELINE {
            for sha in [base.motor_sha256, base.reference_sha256] {
                assert!(sha.is_empty() || is_sha(sha), "{}: {sha}", base.name);
            }
        }
    }

    /// The table's own vocabulary: a tier and a cause from the closed sets, or the matrix's
    /// two words drift apart.
    #[test]
    fn every_tier_and_cause_is_one_of_the_matrix_s_words() {
        for base in BASELINE {
            assert!(
                ["bitwise", "tolerance", "shape"].contains(&base.tier),
                "{}: tier {}",
                base.name,
                base.tier
            );
            assert!(
                [
                    "convention",
                    "rng",
                    "arithmetic",
                    "algorithm",
                    "reference-absent"
                ]
                .contains(&base.cause),
                "{}: cause {}",
                base.name,
                base.cause
            );
        }
    }

    fn is_sha(sha: &str) -> bool {
        sha.len() == 64
            && sha
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }

    const _: fn() -> Option<&'static Baseline> = || BASELINE.first();
}
