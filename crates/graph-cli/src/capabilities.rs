//! The capabilities ledger (`prompt.md` §8): generated from the registry, never
//! hand-written, so it cannot rot the way a PROGRESS.md does.
//!
//! `scale_ceiling`, `degradation` and `ponytail` are plain non-optional fields: a row
//! cannot be written without them, so no later phase can register a capability that
//! skips them. `--check` then refuses any row that claims more than its evidence.

use serde::Serialize;
use std::collections::BTreeSet;
use std::process::ExitCode;

/// Where a capability stands. Serialised lowercase: `absent | stub | implemented | gated`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the Phase-0 registry is empty by definition; Phase 1 registers the first row"
    )
)]
pub enum Status {
    /// Not started.
    Absent,
    /// Present as a placeholder that does not compute the real thing.
    Stub,
    /// Computes the real thing; its gate evidence is not (yet) complete.
    Implemented,
    /// Its 4-way hash and its oracle differential both passed in the current tree.
    Gated,
}

/// One ledger row. Field names are the JSON keys of `prompt.md` §8.
#[derive(Debug, Clone, Serialize)]
pub struct Capability {
    /// Stable dotted id, e.g. `layout.treemap.squarified`.
    pub id: &'static str,
    /// Delivery tier.
    pub tier: u8,
    /// Pipeline stage: ingest, topology, analysis, layout, post, scale.
    pub stage: &'static str,
    /// Geometry kind it emits, for stages that emit geometry.
    pub geometry: Option<&'static str>,
    /// Where it stands.
    pub status: Status,
    /// The reference it is differentially tested against.
    pub oracle: &'static str,
    /// Outcome of that differential, e.g. `byte-equal/1000 seeds`.
    pub oracle_diff: &'static str,
    /// Outcome of the 4-way hash gate: `equal` or why not.
    pub hash_4way: &'static str,
    /// Node count past which it stops being usable. Required.
    pub scale_ceiling: u64,
    /// What happens past the ceiling. Required.
    pub degradation: &'static str,
    /// Its Ponytail marker, or the reason none is owed. Required.
    pub ponytail: &'static str,
    /// Time complexity, stated and held.
    pub complexity: &'static str,
}

/// Every registered capability. Empty in Phase 0: no capability exists yet.
pub fn registry() -> Vec<Capability> {
    Vec::new()
}

/// Every reason a row may not stand as written; empty means the ledger is honest.
pub fn problems(rows: &[Capability]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut found = Vec::new();
    for row in rows {
        if !seen.insert(row.id) {
            found.push(format!("{}: duplicate id", row.id));
        }
        for (field, value) in [
            ("stage", row.stage),
            ("degradation", row.degradation),
            ("ponytail", row.ponytail),
            ("complexity", row.complexity),
        ] {
            if value.trim().is_empty() {
                found.push(format!("{}: required field `{field}` is empty", row.id));
            }
        }
        if row.scale_ceiling == 0 {
            found.push(format!("{}: scale_ceiling is 0", row.id));
        }
        if row.status == Status::Gated
            && (row.hash_4way != "equal" || row.oracle_diff.trim().is_empty())
        {
            found.push(format!(
                "{}: gated without a passing 4-way hash and oracle diff",
                row.id
            ));
        }
    }
    found
}

/// `capabilities --json` and/or `--check`.
pub fn run(json: bool, check: bool) -> ExitCode {
    if !json && !check {
        eprintln!("capabilities: pass --json, --check, or both");
        return ExitCode::from(2);
    }
    let rows = registry();
    if json {
        match serde_json::to_string_pretty(&rows) {
            Ok(text) => println!("{text}"),
            Err(err) => {
                eprintln!("capabilities: serialising the ledger: {err}");
                return ExitCode::from(2);
            }
        }
    }
    if check {
        let found = problems(&rows);
        for problem in &found {
            println!("  {problem}");
        }
        println!(
            "capabilities --check: {} rows, {} problems",
            rows.len(),
            found.len()
        );
        if !found.is_empty() {
            return ExitCode::from(1);
        }
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(status: Status) -> Capability {
        Capability {
            id: "layout.example",
            tier: 1,
            stage: "layout",
            geometry: Some("Point"),
            status,
            oracle: "d3-hierarchy@3.1.2",
            oracle_diff: "byte-equal/1000 seeds",
            hash_4way: "equal",
            scale_ceiling: 200_000,
            degradation: "none",
            ponytail: "none: exact arithmetic",
            complexity: "O(n)",
        }
    }

    #[test]
    fn an_honest_gated_row_passes() {
        assert!(problems(&[row(Status::Gated)]).is_empty());
    }

    #[test]
    fn gated_without_hash_equality_or_oracle_diff_is_refused() {
        let mut unhashed = row(Status::Gated);
        unhashed.hash_4way = "diverged";
        let mut undiffed = row(Status::Gated);
        undiffed.oracle_diff = "";
        assert_eq!(problems(&[unhashed]).len(), 1);
        assert_eq!(problems(&[undiffed]).len(), 1);
    }

    #[test]
    fn empty_required_fields_and_zero_ceiling_are_refused() {
        let mut bare = row(Status::Implemented);
        bare.ponytail = " ";
        bare.degradation = "";
        bare.scale_ceiling = 0;
        assert_eq!(problems(&[bare]).len(), 3);
    }

    #[test]
    fn duplicate_ids_are_refused() {
        assert_eq!(problems(&[row(Status::Stub), row(Status::Stub)]).len(), 1);
    }

    #[test]
    fn status_serialises_to_the_four_ledger_words() {
        let words = [
            Status::Absent,
            Status::Stub,
            Status::Implemented,
            Status::Gated,
        ]
        .map(|s| serde_json::to_string(&s).expect("serialisable"));
        assert_eq!(
            words,
            ["\"absent\"", "\"stub\"", "\"implemented\"", "\"gated\""]
        );
    }

    #[test]
    fn the_phase_0_registry_is_empty_and_serialises_to_an_empty_array() {
        assert!(registry().is_empty());
        assert_eq!(
            serde_json::to_string(&registry()).expect("serialisable"),
            "[]"
        );
    }
}
