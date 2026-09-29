//! The campaign's other two arms, read back from the JavaScript harnesses that measured
//! them: `harness/oracle-tick-bench.mjs` (this repo's own `tick()`, d3-force) and
//! `harness/wasm-tick-bench.mjs` (the same motor compiled to wasm32, through the JS SDK).
//!
//! The crossover is the phase's headline deliverable, and it is a comparison of three
//! numbers, so an arm that did not run must appear as **not measured with the reason** —
//! never as zero, and never omitted. A crossover table that quietly showed only the arm
//! this process can measure would be the one dishonest shape this report could take.

use super::largest_fitting;
use std::path::Path;

/// Where `harness/oracle-tick-bench.mjs --json` writes, and where this reads it from.
pub const ORACLE_JSON: &str = "target/bench/oracle-tick.json";

/// Where `harness/wasm-tick-bench.mjs --json` writes, and where this reads it from.
pub const WASM_JSON: &str = "target/bench/wasm-tick.json";

/// One arm's crossover cell: measured with its ladder, or absent with the reason.
#[derive(Debug, Clone, PartialEq)]
pub struct ArmReading {
    /// The arm's name as the table prints it.
    pub arm: &'static str,
    /// The largest `N` that fit the budget, or `None` when every size was over it.
    pub crossover: Option<u32>,
    /// Every `(n, tick ms)` the arm measured, in ascending `n`.
    pub ladder: Vec<(u32, f64)>,
    /// Why this cell is empty, when it is.
    pub absent: Option<String>,
}

impl ArmReading {
    /// An arm that ran: its crossover and the ladder it was read from.
    pub fn measured(arm: &'static str, crossover: Option<u32>, ladder: &[(u32, f64)]) -> Self {
        let mut ladder = ladder.to_vec();
        ladder.sort_by_key(|(n, _)| *n);
        Self {
            arm,
            crossover,
            ladder,
            absent: None,
        }
    }

    /// An arm that did not, and the command or reason that says so in the table.
    pub fn absent(arm: &'static str, why: &str) -> Self {
        Self {
            arm,
            crossover: None,
            ladder: Vec::new(),
            absent: Some(why.to_owned()),
        }
    }

    /// The cell's crossover, recomputed from the ladder it was given.
    pub fn crossover_of(&self, budget_ms: f64) -> Option<u32> {
        largest_fitting(&self.ladder, budget_ms)
    }
}

/// The `(n, tick ms)` rows of a harness JSON file, or the refusal naming the file.
pub fn read_arm_json(path: &Path) -> Result<Vec<(u32, f64)>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let rows = value
        .get("rows")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("{}: no `rows` array", path.display()))?;
    rows.iter()
        .map(|row| {
            let n = row.get("n").and_then(serde_json::Value::as_u64);
            let ms = row.get("tick_ms").and_then(serde_json::Value::as_f64);
            match (n, ms) {
                (Some(n), Some(ms)) if n <= u64::from(u32::MAX) => Ok((n as u32, ms)),
                _ => Err(format!(
                    "{}: a row is missing `n` or `tick_ms`",
                    path.display()
                )),
            }
        })
        .collect()
}

/// Reads an arm's JSON if the harness wrote it, or the reason its cell is empty.
pub fn read_or_absent(arm: &'static str, path: &str, command: &str) -> ArmReading {
    let path = crate::runner::workspace_root().join(path);
    if !path.exists() {
        return ArmReading::absent(
            arm,
            &format!("{} is absent — run `{command}`", path.display()),
        );
    }
    match read_arm_json(&path) {
        Ok(ladder) => {
            let reading = ArmReading::measured(arm, None, &ladder);
            let crossover = reading.crossover_of(super::FRAME_BUDGET_MS);
            ArmReading::measured(arm, crossover, &ladder)
        }
        Err(why) => ArmReading::absent(arm, &format!("{why} — run `{command}`")),
    }
}

/// The two JavaScript arms, in the order the table prints them, each with its command.
pub fn other_arms() -> Vec<ArmReading> {
    vec![
        read_or_absent(
            "wasm32",
            WASM_JSON,
            "node harness/wasm-tick-bench.mjs --json target/bench/wasm-tick.json",
        ),
        read_or_absent(
            "TypeScript oracle",
            ORACLE_JSON,
            "node harness/oracle-tick-bench.mjs --json target/bench/oracle-tick.json",
        ),
    ]
}

/// The three-arm crossover table: the headline deliverable, wins and losses alike.
pub fn arms_markdown(arms: &[ArmReading], budget_ms: f64) -> String {
    let mut out = format!(
        "| arm | largest N fitting {budget_ms} ms | ladder (n, tick ms) |\n|---|---:|---|\n"
    );
    for arm in arms {
        let (cell, ladder) = match &arm.absent {
            Some(why) => (format!("not measured — {why}"), "—".to_owned()),
            None => (
                arm.crossover_of(budget_ms)
                    .map_or("none".into(), |n| n.to_string()),
                format!("{:?}", arm.ladder),
            ),
        };
        out += &format!("| {} | {cell} | {ladder} |\n", arm.arm);
    }
    out
}
