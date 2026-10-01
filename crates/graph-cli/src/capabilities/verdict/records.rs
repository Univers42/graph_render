//! Every gate record in the gates directory, read by the name its own file carries.
//!
//! One rule replaces a list: a row's `oracle_record` names a file, and the file is looked
//! up. That is what lets the six Graphviz differentials — and every engine added after
//! them — be read without a hand-written arm per engine in the ledger's reader
//! (`super::oracle_record`), and it is the same for the igraph, spring, closed-form and
//! round-trip records.
//!
//! Two records are *not* files of their own and are read by their real producer instead:
//! `hashgate.json`, which the transport row's C20 tally lives inside, and the negative
//! controls, which [`crate::hashgate::Knob::ALL`] walks as a `const`.
//!
//! Ponytail: reads the whole directory rather than the names the ledger cares about.
//! Failing input: a stray `.json` in `target/gates` that is not a gate record. Direction:
//! it can only ever *refuse* — a name resolves to a record that must still carry this
//! tree's fingerprint, 1000 seeds, `pass` and a case for the row's own function — so a
//! file that is not a record backs nothing. Direction: none that weakens a claim.
//! Escape hatch: the gates directory is `$GM_GATES_DIR` (the tests use a temporary one),
//! and an absent directory is no records rather than an error.

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

/// `<dir>/<name>.json` for every record in `dir`, by the name in its file name. An absent
/// directory is an empty map: no run has recorded anything yet, which is every gate's
/// state before its first run, not a failure to read.
///
/// Takes the directory rather than reading [`crate::evidence::gates_dir`] itself, so a
/// test reads a directory it staged instead of the process's.
pub(super) fn all(dir: &Path) -> Result<BTreeMap<String, Value>, String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(err) => return Err(format!("{}: {err}", dir.display())),
    };
    let mut found = BTreeMap::new();
    for entry in entries {
        let path = entry.map_err(|e| format!("{}: {e}", dir.display()))?.path();
        // A name is the file's stem. The reading order of `read_dir` is the filesystem's
        // and is not fixed, so nothing may depend on it: every record lands in a map keyed
        // by its own name and is only ever looked up by that name.
        if let Some(name) = stem(&path) {
            found.insert(name, read_one(&path)?);
        }
    }
    Ok(found)
}

/// `<dir>/<name>.json`'s name, or `None` for anything that is not one — a subdirectory, or
/// a file some other tool left in the gates directory.
fn stem(path: &Path) -> Option<String> {
    if path.extension()? != "json" {
        return None;
    }
    Some(path.file_stem()?.to_str()?.to_owned())
}

/// One record file, parsed. A file that is not JSON is an error rather than a skipped
/// record: a record the ledger cannot read is not a record it may report as absent.
fn read_one(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests;
