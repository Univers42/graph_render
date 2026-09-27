//! The tree fingerprint: one SHA-256 over every input a gate result depends on.
//!
//! This file is compiled twice: into graph-cli, and into its build script (`build.rs`
//! includes it by path), so the binary carries the fingerprint of the tree it was built
//! from and a run can refuse to record against any other tree.

use sha2::{Digest, Sha256};
use std::path::Path;

/// Every input a gate result depends on, relative to the workspace root: the Rust
/// sources and their toolchain pins, the harnesses and fixtures, and the TypeScript
/// oracle with its lockfile and compiler settings. Documentation is deliberately not in
/// it: rewording a report does not void a measurement.
///
/// Ponytail: what a path cannot pin. The base images are named by tag (`debian:trixie-
/// slim`, `node:22-slim`), so a registry that moves a tag changes the toolchain without
/// changing this fingerprint, and the oracle's ICU comes with that Node image. Failing
/// input: a re-pulled tag. Direction: an old record counts as current (the dangerous
/// one). Escape hatch: `oracle-diff.json` records the Node, ICU and locale it ran
/// under, and `ge-rust` pins Rust by version, so a reader can compare those by hand.
pub const FINGERPRINTED: [&str; 13] = [
    "Cargo.toml",
    "Cargo.lock",
    ".cargo",
    "docker",
    "crates",
    "harness",
    "fixtures",
    "src",
    "Dockerfile",
    "package.json",
    "package-lock.json",
    "tsconfig.json",
    "tests/ts-extension-loader.mjs",
];

/// SHA-256 over `path NUL sha256(content) LF` for every file under `entries`, in byte
/// order of path. A missing entry is an error: a fingerprint over less than the tree
/// would match a tree it never saw.
pub fn fingerprint_of(root: &Path, entries: &[&str]) -> Result<String, String> {
    let mut files = Vec::new();
    for entry in entries {
        collect(root, &root.join(entry), &mut files)?;
    }
    files.sort();
    let mut listing = String::new();
    for relative in files {
        let bytes = std::fs::read(root.join(&relative))
            .map_err(|e| format!("fingerprint: reading {relative}: {e}"))?;
        listing.push_str(&format!("{relative}\0{}\n", hex_sha256(&bytes)));
    }
    Ok(hex_sha256(listing.as_bytes()))
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

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
