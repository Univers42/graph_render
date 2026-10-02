//! Recorded gate results: what the ledger reads instead of hand-typed claims.
//!
//! A gate writes `<gates>/<name>.json` carrying its result and the fingerprint of the
//! tree it ran on. The ledger accepts a record only while that fingerprint is the
//! current tree's, so an edit to any input a gate depends on voids the evidence until
//! the gate is re-run. `<gates>` is `target/gates`, or `$GM_GATES_DIR` (the tests use a
//! temporary one, so `cargo test` never overwrites a real run's record).
//!
//! A run is pinned to one tree from build to record: it takes a [`Stamp`] before it
//! does anything, which refuses unless the tree is the one this binary was built from,
//! and [`write`] refuses unless the tree is still that one. An edit at any point in
//! between leaves no record, never a record of one tree's results under another's name.
//!
//! A record that did not pass never replaces one that did ([`Outcome::Refused`), and only
//! a record from the same tree counts as "one that did": a passing record another tree
//! left behind is already void to the ledger, so it must not stand in the way of this
//! tree's run. That refusal is a warning and not a failure to run: the gate ran, judged
//! and has an exit code, which is the thing its caller reports. Only a record that is
//! *missing* — because the tree moved or the file could not be written — leaves a
//! verdict unbacked, and that is the one outcome callers read as "could not run"
//! ([`record`]). The write is atomic, so a reader never sees half a record.

pub use crate::fingerprint::FINGERPRINTED;
use crate::fingerprint::fingerprint_of;
use crate::runner::workspace_root;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Overrides where records are read and written.
pub const GATES_ENV: &str = "GM_GATES_DIR";

/// The fingerprint of the tree this binary was built from (`build.rs`).
pub const BUILT_FROM: &str = env!("GM_BUILD_FINGERPRINT");

/// Where records live.
pub fn gates_dir() -> PathBuf {
    std::env::var_os(GATES_ENV).map_or_else(
        || workspace_root().join("target").join("gates"),
        PathBuf::from,
    )
}

/// The current tree's fingerprint (see [`FINGERPRINTED`]).
pub fn tree_fingerprint() -> Result<String, String> {
    fingerprint_of(&workspace_root(), &FINGERPRINTED)
}

/// The tree a run started on, checked to be the tree its binary was built from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp(String);

impl Stamp {
    /// Takes the stamp: first thing in a run that records.
    pub fn take() -> Result<Self, String> {
        Self::against(BUILT_FROM, tree_fingerprint()?)
    }

    fn against(built: &str, now: String) -> Result<Self, String> {
        if now != built {
            return Err(format!(
                "graph-cli was built from tree {built:.12}… but the tree is now {now:.12}…: rebuild before recording"
            ));
        }
        Ok(Self(now))
    }

    /// The fingerprint the run is pinned to.
    pub fn fingerprint(&self) -> &str {
        &self.0
    }

    /// `Ok` while the tree is still the stamped one.
    pub fn still_current(&self) -> Result<(), String> {
        self.unchanged(&tree_fingerprint()?)
    }

    fn unchanged(&self, now: &str) -> Result<(), String> {
        if now != self.0 {
            return Err(format!(
                "the tree changed during the run ({:.12}… → {now:.12}…): not recorded",
                self.0
            ));
        }
        Ok(())
    }
}

/// What became of a run's record, and what a caller may read out of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Written to this path.
    Recorded(PathBuf),
    /// Refused: a *passing* record of the same name stands, so this run's — which did
    /// not pass — was not written. The gate ran and judged either way, so the run keeps
    /// its exit code and the ledger keeps the stronger claim.
    Refused(String),
    /// Not written, and nothing stands in its place: the tree moved since the stamp, the
    /// body was not a record, or the file could not be written. The run's verdict would
    /// be unbacked, so this is the one outcome that is a failure to run.
    Failed(String),
}

/// Writes `body` plus `gate` and the stamped fingerprint as `<gates>/<name>.json`, and
/// reports what became of it (see [`Outcome`]).
pub fn write(stamp: &Stamp, name: &str, body: Value) -> Outcome {
    match stamp.still_current() {
        Ok(()) => write_to(&gates_dir(), name, body, stamp.fingerprint().to_owned()),
        Err(err) => Outcome::Failed(err),
    }
}

/// Records `body` under `name`, printing a refusal on stderr as a warning, and returns
/// `Err` only when nothing at all was written for a reason that is the run's own
/// ([`Outcome::Failed`]).
///
/// Every gate records through this, so a gate that ran and failed says so (exit 1) and
/// a gate that could not be recorded at all says that too (exit 2), whatever its record
/// name.
pub fn record(stamp: &Stamp, name: &str, body: Value) -> Result<(), String> {
    handled(write(stamp, name, body))
}

/// [`write`]'s outcome as a caller reads it: the one shape every gate handles alike.
fn handled(outcome: Outcome) -> Result<(), String> {
    match outcome {
        Outcome::Recorded(_) => Ok(()),
        Outcome::Refused(why) => {
            eprintln!("warning: {why}");
            Ok(())
        }
        Outcome::Failed(err) => Err(err),
    }
}

/// Writes `body` as `<dir>/<name>.json`, unless it would replace a *passing* record of
/// the same name **from this tree** with one that did not pass.
///
/// A record is evidence, and the run that writes one is not always the run that is
/// right about it: a negative control, a short `--seeds` sweep, or a run against a tree
/// that has since regressed all write the same `<name>.json` a full honest run did, and
/// each would leave the ledger holding the weaker claim while the gate's own exit code
/// was the only warning. The ledger reads these files long after the run that made them,
/// so a record that passed is kept: a later failing run is
/// [`refused`](Outcome::Refused) and the passing one stands. A later *passing* run does
/// replace it, which is what re-running the gate for a fix must do.
///
/// Three rules, and each one is a way the guard used to read as green when it was not:
///
/// - `pass` must be a **boolean**. A body with no `pass`, a `null` one, or the string
///   `"false"` is not a verdict at all, and keying the guard on the literal
///   `false` let every one of those take the write branch and overwrite a passing record.
/// - the standing record must be from **this** fingerprint. A passing record left by
///   another tree is already void to the ledger (`verdict::current` refuses it), so it
///   must not stand in the way of this tree's failing run — and refusing the write would
///   leave exactly that void record as the file the ledger reads.
/// - the write is **atomic** (temporary file in the same directory, then `rename`), so a
///   concurrent reader or a kill mid-write sees the old record or the new one and never
///   a truncated one that [`read_from`] could only report as a parse error.
fn write_to(dir: &Path, name: &str, mut body: Value, fingerprint: String) -> Outcome {
    if !body.is_object() {
        return Outcome::Failed("a gate record is a JSON object".into());
    }
    if !body["pass"].is_boolean() {
        return Outcome::Failed(format!(
            "{name}: a gate record carries a boolean `pass`, and this body does not"
        ));
    }
    let object = body
        .as_object_mut()
        .expect("checked to be a JSON object two lines above");
    object.insert("gate".into(), Value::from(name));
    object.insert("fingerprint".into(), Value::from(fingerprint));
    let path = dir.join(format!("{name}.json"));
    if body["pass"] == Value::Bool(false)
        && let Ok(Some(existing)) = read_from(dir, name)
        && existing["pass"] == Value::Bool(true)
        && existing["fingerprint"] == body["fingerprint"]
    {
        return Outcome::Refused(format!(
            "{name}: not recorded — the existing record passed and this run did \
             not, so the passing record stands (re-run the gate to replace it)"
        ));
    }
    if let Err(err) = std::fs::create_dir_all(dir) {
        return Outcome::Failed(format!("{}: {err}", dir.display()));
    }
    let text = match serde_json::to_string_pretty(&body) {
        Ok(text) => text,
        Err(err) => return Outcome::Failed(err.to_string()),
    };
    let temporary = dir.join(format!("{name}.json.{}.tmp", std::process::id()));
    if let Err(err) = std::fs::write(&temporary, text + "\n") {
        let _ = std::fs::remove_file(&temporary);
        return Outcome::Failed(format!("{}: {err}", temporary.display()));
    }
    match std::fs::rename(&temporary, &path) {
        Ok(()) => Outcome::Recorded(path),
        Err(err) => {
            let _ = std::fs::remove_file(&temporary);
            Outcome::Failed(format!("{}: {err}", path.display()))
        }
    }
}

/// `<gates>/<name>.json`, or `None` when no run has recorded it.
pub fn read(name: &str) -> Result<Option<Value>, String> {
    read_from(&gates_dir(), name)
}

fn read_from(dir: &Path, name: &str) -> Result<Option<Value>, String> {
    let path = dir.join(format!("{name}.json"));
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| format!("{}: {e}", path.display())),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("{}: {err}", path.display())),
    }
}

#[cfg(test)]
mod tests;
