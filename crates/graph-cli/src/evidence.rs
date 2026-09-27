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

/// Writes `body` plus `gate` and the stamped fingerprint as `<gates>/<name>.json`,
/// unless the tree moved since the stamp was taken.
pub fn write(stamp: &Stamp, name: &str, body: Value) -> Result<PathBuf, String> {
    stamp.still_current()?;
    write_to(&gates_dir(), name, body, stamp.fingerprint().to_owned())
}

fn write_to(
    dir: &Path,
    name: &str,
    mut body: Value,
    fingerprint: String,
) -> Result<PathBuf, String> {
    let object = body
        .as_object_mut()
        .ok_or("a gate record is a JSON object")?;
    object.insert("gate".into(), Value::from(name));
    object.insert("fingerprint".into(), Value::from(fingerprint));
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join(format!("{name}.json"));
    let text = serde_json::to_string_pretty(&body).map_err(|e| e.to_string())?;
    std::fs::write(&path, text + "\n").map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
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
mod tests {
    use super::*;
    use crate::runner::sha256_hex;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gm-evidence-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).expect("temp dir");
        dir
    }

    #[test]
    fn the_fingerprint_moves_with_content_names_and_new_files_only() {
        let dir = scratch("fp");
        std::fs::write(dir.join("a.txt"), "one").expect("write");
        std::fs::write(dir.join("sub/b.txt"), "two").expect("write");
        let entries = ["a.txt", "sub"];
        let first = fingerprint_of(&dir, &entries).expect("fingerprint");
        assert_eq!(first, fingerprint_of(&dir, &entries).expect("again"));
        std::fs::write(dir.join("sub/b.txt"), "tw0").expect("write");
        let edited = fingerprint_of(&dir, &entries).expect("edited");
        assert_ne!(first, edited);
        std::fs::write(dir.join("sub/c.txt"), "").expect("write");
        assert_ne!(edited, fingerprint_of(&dir, &entries).expect("added"));
        assert!(fingerprint_of(&dir, &["missing"]).is_err());
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn the_listing_is_path_nul_digest_lines_in_byte_order() {
        let dir = scratch("listing");
        std::fs::write(dir.join("sub/b"), "x").expect("write");
        std::fs::write(dir.join("a"), "").expect("write");
        let empty = sha256_hex(b"");
        let x = sha256_hex(b"x");
        let want = sha256_hex(format!("a\0{empty}\nsub/b\0{x}\n").as_bytes());
        assert_eq!(fingerprint_of(&dir, &["sub", "a"]), Ok(want));
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn a_record_reads_back_as_written_and_only_absence_is_none() {
        let dir = scratch("records");
        let body = serde_json::json!({ "seeds": 3, "pass": true });
        let path = write_to(&dir, "gate", body, "f".into()).expect("written");
        assert_eq!(path, dir.join("gate.json"));
        let record = read_from(&dir, "gate").expect("readable").expect("present");
        assert_eq!(
            record,
            serde_json::json!({ "seeds": 3, "pass": true, "gate": "gate", "fingerprint": "f" })
        );
        assert_eq!(read_from(&dir, "absent"), Ok(None));
        std::fs::write(dir.join("torn.json"), "{").expect("write");
        assert!(read_from(&dir, "torn").is_err());
        std::fs::create_dir(dir.join("dir.json")).expect("dir");
        assert!(read_from(&dir, "dir").is_err(), "unreadable is not absent");
        assert!(write_to(&dir, "list", serde_json::json!([]), "f".into()).is_err());
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn a_stamp_needs_the_built_tree_and_goes_stale_when_the_tree_moves() {
        let stamp = Stamp::against("abc", "abc".into()).expect("same tree");
        assert_eq!(stamp.fingerprint(), "abc");
        assert_eq!(stamp.unchanged("abc"), Ok(()));
        let stale = Stamp("0".repeat(64))
            .still_current()
            .expect_err("not this tree");
        assert!(stale.contains("changed during the run"), "{stale}");
        let moved = stamp.unchanged("abd").expect_err("moved");
        assert!(moved.contains("changed during the run"), "{moved}");
        let rebuilt = Stamp::against("abc", "abd".into()).expect_err("other tree");
        assert!(rebuilt.contains("rebuild before recording"), "{rebuilt}");
    }

    #[test]
    fn the_real_tree_is_the_one_this_binary_was_built_from() {
        let fingerprint = tree_fingerprint().expect("every root exists");
        assert_eq!(fingerprint.len(), 64);
        let stamp = Stamp::take().expect("built from this tree");
        assert_eq!(stamp.fingerprint(), BUILT_FROM);
        assert_eq!(stamp.still_current(), Ok(()));
    }
}
