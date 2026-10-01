//! What the recorded gate runs say about one row: its `hash_4way` and `oracle_diff`
//! strings, derived from evidence — `Ok` when the run backs the row, `Err` with the
//! reason when it does not.

use crate::evidence;
use crate::hashgate::{Knob, LAYOUT, TRANSPORT};
use serde_json::Value;
use std::collections::BTreeMap;

mod records;

/// Fewest seeds a gate run may cover and still back a `gated` row (`prompt.md` §7).
pub const MIN_SEEDS: u64 = 1000;

/// The `oracle_record` of a row whose differential is the hash gate's own C20 tally
/// rather than a TypeScript or round-trip record: the per-seed count of the seeds where
/// the real ABI reached the retained shim's bytes, in `hashgate.json`.
pub const TRANSPORT_RECORD: &str = "wasm-transport";

/// The recorded gate runs, and the tree they must have run on.
pub struct Evidence {
    /// The current tree's fingerprint.
    pub fingerprint: String,
    /// `hashgate.json`: the honest 4-way run.
    pub hashgate: Option<Value>,
    /// Every negative control's record, by name, in `Knob::ALL` order.
    pub controls: Vec<(&'static str, Option<Value>)>,
    /// Every other record in the gates directory, by the name its own file carries.
    pub by_name: BTreeMap<String, Value>,
}

impl Evidence {
    /// Reads every record from the gates directory.
    pub fn load() -> Result<Self, String> {
        let controls = Knob::ALL
            .iter()
            .map(|knob| Ok((knob.record(), evidence::read(knob.record())?)))
            .collect::<Result<_, String>>()?;
        Ok(Self {
            fingerprint: evidence::tree_fingerprint()?,
            hashgate: evidence::read("hashgate")?,
            controls,
            by_name: records::all(&evidence::gates_dir())?,
        })
    }

    /// The oracle record a row names. One rule, not a list: a row's `oracle_record` is a
    /// record's name and is looked up as one, so the six Graphviz differentials
    /// (`oracle-<engine>.json`, from `oracle_python::graphviz`'s own `ENGINES` table) and
    /// every engine, differential and hand oracle added after them are read without an arm
    /// here. The one record that is not a file of its own is the transport row's, whose
    /// differential is the hash gate's C20 tally and lives inside `hashgate.json`.
    fn oracle_record(&self, name: &str) -> Option<&Value> {
        if name == TRANSPORT_RECORD {
            return self.hashgate.as_ref();
        }
        self.by_name.get(name)
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
    let control = red_control(e, stage)?;
    Ok(format!(
        "equal/{seeds} seeds ({stage} stage; negative control {control} red)"
    ))
}

/// The first negative control that ran on the current tree and went red on `stage`, or
/// every control's reason for not backing it.
fn red_control(e: &Evidence, stage: &str) -> Result<&'static str, String> {
    let mut why = Vec::new();
    for (name, record) in &e.controls {
        match current(e, record.as_ref(), name) {
            Err(err) => why.push(err),
            Ok(run) if run["pass"] != Value::Bool(false) => {
                why.push(format!("{name} did not go red"));
            }
            Ok(run) if !diverged(run, stage) => {
                why.push(format!("{name} did not go red on the {stage} stage"));
            }
            Ok(_) => return Ok(name),
        }
    }
    if why.is_empty() {
        why.push("no control is registered".into());
    }
    Err(format!(
        "no negative control backs the {stage} stage: {}",
        why.join("; ")
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

/// The oracle verdict over `functions` from the record `name`: a passing run in which
/// every one of them had cases and no unexplained mismatch.
pub fn oracle_diff(e: &Evidence, name: &str, functions: &[&str]) -> Result<String, String> {
    if name == TRANSPORT_RECORD {
        return transport(e);
    }
    let run = current(e, e.oracle_record(name), name)?;
    let seeds = seeds_of(run, name)?;
    if run["pass"] != Value::Bool(true) {
        return Err(format!("{name} did not pass"));
    }
    let (mut cases, mut declared) = (0, 0);
    for function in functions {
        let counts = &run["functions"][*function];
        let n = counts["cases"].as_u64().unwrap_or(0);
        if n == 0 {
            return Err(format!("{name} ran no {function} case"));
        }
        if counts["unexplained"].as_u64() != Some(0) {
            return Err(format!("{name}: {function} has unexplained mismatches"));
        }
        cases += n;
        declared += counts["declared"].as_u64().unwrap_or(0);
    }
    let known = if declared == 0 {
        String::new()
    } else {
        format!(", {declared} declared divergences")
    };
    if run["tolerance"] == Value::Bool(true) {
        return Ok(format!(
            "within measured ceiling of the oracle/{seeds} seeds ({cases} cases)"
        ));
    }
    Ok(format!("byte-equal/{seeds} seeds ({cases} cases{known})"))
}

/// C20: the transport row's differential — the hash gate's own tally of the seeds where
/// the real ABI's snapshot reached the retained shim's bytes. Named explicitly, so a
/// tally about some other pair of stages is refused rather than read as this one.
fn transport(e: &Evidence) -> Result<String, String> {
    let run = current(e, e.hashgate.as_ref(), "hashgate")?;
    let seeds = seeds_of(run, "hashgate")?;
    let tally = run
        .get("transport")
        .ok_or("no transport tally: hashgate recorded no C20 count")?;
    for (key, want) in [("stage", TRANSPORT), ("reference", LAYOUT)] {
        let found = tally[key].as_str().unwrap_or("");
        if found.is_empty() {
            return Err(format!("no transport tally: no {key} in hashgate's count"));
        }
        if found != want {
            return Err(format!(
                "hashgate: transport tally is {key} {found}, not {want}"
            ));
        }
    }
    let equal = tally["equal"].as_u64().unwrap_or(0);
    if equal != seeds {
        return Err(format!(
            "hashgate: {TRANSPORT} matched the shim on {equal} of {seeds} seeds"
        ));
    }
    Ok(format!("byte-equal/{seeds} seeds ({seeds} cases)"))
}
