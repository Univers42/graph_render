//! Recorded gate results: what the ledger reads instead of hand-typed claims.
//!
//! A gate writes `<gates>/<name>.json` carrying its result and the fingerprint of the
//! tree it ran on. The ledger accepts a record only while that fingerprint is the
//! current tree's, so an edit to any input a gate depends on voids the evidence until
//! the gate is re-run. `<gates>` is `target/gates`, or `$GM_GATES_DIR` (the tests use a
//! temporary one, so `cargo test` never overwrites a real run's record).

use crate::runner::{sha256_hex, workspace_root};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Overrides where records are read and written.
pub const GATES_ENV: &str = "GM_GATES_DIR";

/// Every input a gate result depends on, relative to the workspace root: the Rust
/// sources, the harnesses and fixtures, the TypeScript oracle, and the manifests that
/// pin their toolchains. Documentation is deliberately not in it — rewording a report
/// does not void a measurement.
pub const FINGERPRINTED: [&str; 8] = [
    "Cargo.toml",
    "Cargo.lock",
    "crates",
    "harness",
    "fixtures",
    "src",
    "package.json",
    "tests/ts-extension-loader.mjs",
];

/// Where records live.
pub fn gates_dir() -> PathBuf {
    std::env::var_os(GATES_ENV).map_or_else(
        || workspace_root().join("target").join("gates"),
        PathBuf::from,
    )
}

/// SHA-256 over `path NUL sha256(content) LF` for every fingerprinted file, in byte
/// order of path. A missing root is an error: a fingerprint over less than the tree
/// would match a tree it never saw.
pub fn tree_fingerprint() -> Result<String, String> {
    fingerprint_of(&workspace_root(), &FINGERPRINTED)
}

fn fingerprint_of(root: &Path, entries: &[&str]) -> Result<String, String> {
    let mut files = Vec::new();
    for entry in entries {
        collect(root, &root.join(entry), &mut files)?;
    }
    files.sort();
    let mut listing = String::new();
    for relative in files {
        let bytes = std::fs::read(root.join(&relative))
            .map_err(|e| format!("fingerprint: reading {relative}: {e}"))?;
        listing.push_str(&format!("{relative}\0{}\n", sha256_hex(&bytes)));
    }
    Ok(sha256_hex(listing.as_bytes()))
}

fn collect(root: &Path, path: &Path, files: &mut Vec<String>) -> Result<(), String> {
    let meta =
        std::fs::metadata(path).map_err(|e| format!("fingerprint: {}: {e}", path.display()))?;
    if meta.is_file() {
        let relative = path.strip_prefix(root).map_err(|e| e.to_string())?;
        let text = relative
            .to_str()
            .ok_or_else(|| format!("non-UTF-8 path {}", relative.display()))?;
        files.push(text.replace('\\', "/"));
        return Ok(());
    }
    let listing =
        std::fs::read_dir(path).map_err(|e| format!("fingerprint: {}: {e}", path.display()))?;
    for child in listing {
        let child = child.map_err(|e| format!("fingerprint: {}: {e}", path.display()))?;
        collect(root, &child.path(), files)?;
    }
    Ok(())
}

/// Writes `body` plus `gate` and the current fingerprint as `<gates>/<name>.json`.
pub fn write(name: &str, body: Value) -> Result<PathBuf, String> {
    write_to(&gates_dir(), name, body, tree_fingerprint()?)
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
    fn the_real_tree_fingerprints() {
        let fingerprint = tree_fingerprint().expect("every root exists");
        assert_eq!(fingerprint.len(), 64);
    }
}
