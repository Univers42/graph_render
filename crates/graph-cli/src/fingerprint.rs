//! The tree fingerprint: one SHA-256 over every input a gate result depends on.
//!
//! This file is compiled twice: into graph-cli, and into its build script (`build.rs`
//! includes it by path), so the binary carries the fingerprint of the tree it was built
//! from and a run can refuse to record against any other tree.

use sha2::{Digest, Sha256};
use std::path::Path;

/// The shell wrappers a gate result depends on, and the whole of that set: the two
/// roots, and everything either of them sources or runs.
///
/// `scripts/orch/gr` builds the binary every gate row runs; it sources `image.sh`
/// (which picks and builds the image from `docker/`'s recipes and pins `GM_NODE_IMAGE`),
/// `scratch.sh` (which resolves `$GM_SCRATCH` and the pinned references the oracle
/// reads) and `docker-env.sh` (which finds the daemon). `scripts/scigraphs-conformance.sh`
/// is itself a gate and writes `target/gates/scigraphs-conformance.json`.
///
/// Ponytail: where this boundary stops, and what that gets wrong. It is the transitive
/// closure of *those two roots* and nothing else. A blanket `scripts` entry is
/// deliberately not taken: `scripts/orch/queue.txt`, `scripts/orch/gate.sh`,
/// `scripts/orch/mutants.sh` and the `studio-*.sh` wrappers change hourly and would void
/// every recorded run on an edit that cannot move a motor gate's bytes. Failing input: a
/// new gate wrapper, or a new `source` line inside one of the five. Direction: an edit
/// to it leaves a record reading as current — the dangerous direction, since the whole
/// point of the fingerprint is that it does not. Escape hatch: this is a named list, not
/// a directory, so the next wrapper is added here on purpose, by whoever adds it.
pub const GATE_SCRIPTS: [&str; 5] = [
    "scripts/orch/gr",
    "scripts/orch/image.sh",
    "scripts/orch/scratch.sh",
    "scripts/orch/docker-env.sh",
    "scripts/scigraphs-conformance.sh",
];

/// Every input a gate result depends on, relative to the workspace root: the Rust
/// sources and their toolchain pins, the harnesses and fixtures, the TypeScript
/// oracle with its lockfile and compiler settings, and the [`GATE_SCRIPTS`]. Documentation
/// is deliberately not in it: rewording a report does not void a measurement.
///
/// Ponytail: what a path cannot pin. The base images are named by tag (`debian:trixie-
/// slim`, `node:22-slim`), so a registry that moves a tag changes the toolchain without
/// changing this fingerprint, and the oracle's ICU comes with that Node image. Failing
/// input: a re-pulled tag. Direction: an old record counts as current (the dangerous
/// one). Escape hatch: `oracle-diff.json` records the Node, ICU and locale it ran
/// under, and `ge-rust` pins Rust by version, so a reader can compare those by hand.
pub const FINGERPRINTED: [&str; 18] = [
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
    "scripts/orch/gr",
    "scripts/orch/image.sh",
    "scripts/orch/scratch.sh",
    "scripts/orch/docker-env.sh",
    "scripts/scigraphs-conformance.sh",
];

/// Whether `path` lies inside one of [`FINGERPRINTED`].
///
/// A file written there — even a transient one a test stages and removes — is a file
/// the listing names, so the fingerprint moves for as long as it exists. A binary built
/// before the write then refuses its own tree (`Stamp::take`), and a fingerprint taken
/// while it exists misses a file deleted before its bytes are read. Every path a run
/// writes transiently must therefore be outside this set, which is what
/// [`crate::runner::harness_mutant`] stages into.
///
/// Test-only, and this file is also compiled into `build.rs`, which computes the
/// fingerprint and never stages anything.
#[cfg(test)]
pub fn is_fingerprinted(root: &Path, path: &Path) -> bool {
    FINGERPRINTED
        .iter()
        .any(|entry| path.starts_with(root.join(entry)))
}

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

/// Every file under `path`, by workspace-relative path with `/` separators.
///
/// A **symlink is refused, not followed** (`symlink_metadata`, never `metadata`).
/// Following one is how a fingerprint hangs instead of failing: a link back into an
/// ancestor directory recurses forever, and a link out of the tree hashes bytes that are
/// not the tree's. Refusing is the fail-closed direction — the run stops with a message
/// naming the path rather than recording a digest it cannot vouch for.
fn collect(root: &Path, path: &Path, files: &mut Vec<String>) -> Result<(), String> {
    let meta = std::fs::symlink_metadata(path)
        .map_err(|e| format!("fingerprint: {}: {e}", path.display()))?;
    if meta.file_type().is_symlink() {
        return Err(format!(
            "fingerprint: {} is a symlink: refusing to follow it",
            path.display()
        ));
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::workspace_root;

    /// A tree holding one file per entry of `entries`, each with its own distinct bytes.
    fn tree(name: &str, entries: &[&str]) -> PathBuf {
        let root = std::env::temp_dir().join(format!("gm-fingerprint-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for (n, entry) in entries.iter().enumerate() {
            let path = root.join(entry);
            std::fs::create_dir_all(path.parent().expect("a parent")).expect("temp dir");
            std::fs::write(&path, format!("original bytes of {entry} #{n}")).expect("writable");
        }
        root
    }

    /// The negative control for every assertion below: an edit **outside** the list moves
    /// nothing. Without it, "the digest moved" proves only that some file was read.
    #[test]
    fn an_edit_outside_the_list_moves_nothing() {
        let entries = ["scripts/orch/gr", "scripts/orch/queue.txt"];
        let root = tree("outside", &entries);
        let listed: &[&str] = &["scripts/orch/gr"];
        let before = fingerprint_of(&root, listed).expect("listed files hash");
        std::fs::write(root.join("scripts/orch/queue.txt"), "edited").expect("writable");
        assert_eq!(
            fingerprint_of(&root, listed).expect("listed files hash"),
            before,
            "an orchestration file outside the boundary must not void a record"
        );
        std::fs::remove_dir_all(&root).expect("removable");
    }

    /// Every gate wrapper is inside the boundary, so editing one of them voids today's
    /// records instead of leaving them reading as current.
    #[test]
    fn an_edit_to_any_gate_wrapper_moves_the_fingerprint() {
        let root = tree("wrappers", &GATE_SCRIPTS);
        let before = fingerprint_of(&root, &GATE_SCRIPTS).expect("the wrappers hash");
        for entry in GATE_SCRIPTS {
            std::fs::write(root.join(entry), "an edit that changes a gate's result")
                .expect("writable");
            let after = fingerprint_of(&root, &GATE_SCRIPTS).expect("the wrappers hash");
            assert_ne!(after, before, "{entry} is fingerprinted");
        }
        std::fs::remove_dir_all(&root).expect("removable");
    }

    /// The boundary as the tree states it: every wrapper is covered, and the rest of
    /// `scripts/` is deliberately not, so a hourly orchestration edit does not void
    /// every recorded run.
    #[test]
    fn the_boundary_is_the_wrappers_and_not_the_rest_of_scripts() {
        let root = workspace_root();
        for entry in GATE_SCRIPTS {
            assert!(
                root.join(entry).is_file(),
                "{entry} is named by the fingerprint but is not in the tree"
            );
            assert!(
                is_fingerprinted(&root, &root.join(entry)),
                "{entry} is a gate wrapper and must be fingerprinted"
            );
        }
        for outside in ["scripts/orch/queue.txt", "scripts/orch/gate.sh"] {
            assert!(
                !is_fingerprinted(&root, &root.join(outside)),
                "{outside} changes hourly and must not void a record"
            );
        }
    }

    /// A symlink is refused rather than followed: a loop would otherwise recurse until
    /// the gate is killed, and a link out of the tree would hash bytes that are not the
    /// tree's.
    #[test]
    fn a_symlink_under_a_fingerprinted_path_is_refused_not_followed() {
        let root = tree("symlink", &["crates/a.rs"]);
        std::os::unix::fs::symlink("../crates", root.join("crates/loop")).expect("a link");
        let err = fingerprint_of(&root, &["crates"]).expect_err("a loop is refused");
        assert!(err.contains("is a symlink"), "{err}");
        std::fs::remove_dir_all(&root).expect("removable");
    }
}
