//! What the recorded gate runs say about one row: its `hash_4way` and `oracle_diff`
//! strings, derived from evidence — `Ok` when the run backs the row, `Err` with the
//! reason when it does not.

use crate::evidence;
use serde_json::Value;

/// Fewest seeds a gate run may cover and still back a `gated` row (`prompt.md` §7).
pub const MIN_SEEDS: u64 = 1000;

/// The recorded gate runs, and the tree they must have run on.
pub struct Evidence {
    /// The current tree's fingerprint.
    pub fingerprint: String,
    /// `hashgate.json`: the honest 4-way run.
    pub hashgate: Option<Value>,
    /// `hashgate-control.json`: the negative control.
    pub control: Option<Value>,
    /// `oracle-diff.json`: the TypeScript arm's verdict.
    pub oracle: Option<Value>,
}

impl Evidence {
    /// Reads every record from the gates directory.
    pub fn load() -> Result<Self, String> {
        Ok(Self {
            fingerprint: evidence::tree_fingerprint()?,
            hashgate: evidence::read("hashgate")?,
            control: evidence::read("hashgate-control")?,
            oracle: evidence::read("oracle-diff")?,
        })
    }
}

/// `record`, if it exists and ran on the current tree.
fn current<'a>(e: &Evidence, record: Option<&'a Value>, name: &str) -> Result<&'a Value, String> {
    let record = record.ok_or(format!("no {name} record: run the gate"))?;
    if record["fingerprint"].as_str() != Some(e.fingerprint.as_str()) {
        return Err(format!(
            "{name} record is from another tree: re-run the gate"
        ));
    }
    Ok(record)
}

fn seeds_of(record: &Value, name: &str) -> Result<u64, String> {
    let seeds = record["seeds"]
        .as_u64()
        .ok_or(format!("{name} record has no seed count"))?;
    if seeds < MIN_SEEDS {
        return Err(format!("{name} ran {seeds} seeds, need {MIN_SEEDS}"));
    }
    Ok(seeds)
}

/// The 4-way hash verdict for `stage`: an honest pass on every seed of it, and a
/// negative control that went red on the same tree.
pub fn hash_4way(e: &Evidence, stage: &str) -> Result<String, String> {
    let run = current(e, e.hashgate.as_ref(), "hashgate")?;
    let seeds = seeds_of(run, "hashgate")?;
    if run["pass"] != Value::Bool(true) {
        return Err("hashgate did not pass".into());
    }
    if run["equal"][stage].as_u64() != Some(seeds) {
        return Err(format!(
            "hashgate: stage {stage} not 4-way equal on all {seeds} seeds"
        ));
    }
    let control = current(e, e.control.as_ref(), "hashgate-control")?;
    if control["pass"] != Value::Bool(false) {
        return Err("the negative control did not go red".into());
    }
    if !diverged(control, stage) {
        return Err(format!(
            "the negative control did not go red on the {stage} stage"
        ));
    }
    Ok(format!(
        "equal/{seeds} seeds ({stage} stage; negative control red)"
    ))
}

/// Whether the control saw `stage` differ on at least one of its seeds: a control that
/// went red on another stage says nothing about this one.
fn diverged(control: &Value, stage: &str) -> bool {
    match (control["equal"][stage].as_u64(), control["seeds"].as_u64()) {
        (Some(equal), Some(seeds)) => equal < seeds,
        _ => false,
    }
}

/// The oracle verdict over `functions`: a passing run in which every one of them had
/// cases and no unexplained mismatch.
pub fn oracle_diff(e: &Evidence, functions: &[&str]) -> Result<String, String> {
    let run = current(e, e.oracle.as_ref(), "oracle-diff")?;
    let seeds = seeds_of(run, "oracle-diff")?;
    if run["pass"] != Value::Bool(true) {
        return Err("oracle-diff did not pass".into());
    }
    let (mut cases, mut declared) = (0, 0);
    for function in functions {
        let counts = &run["functions"][*function];
        let n = counts["cases"].as_u64().unwrap_or(0);
        if n == 0 {
            return Err(format!("oracle-diff ran no {function} case"));
        }
        if counts["unexplained"].as_u64() != Some(0) {
            return Err(format!(
                "oracle-diff: {function} has unexplained mismatches"
            ));
        }
        cases += n;
        declared += counts["declared"].as_u64().unwrap_or(0);
    }
    let known = if declared == 0 {
        String::new()
    } else {
        format!(", {declared} declared divergences")
    };
    Ok(format!("byte-equal/{seeds} seeds ({cases} cases{known})"))
}
