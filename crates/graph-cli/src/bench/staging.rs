//! Where a negative control's staged harness copy is written, and the test that pins it.
//!
//! The Phase 9 tick harnesses' own negative controls copy a harness, change one pinned
//! constant in the copy and run *that*, so a self-check that cannot fail fails the test
//! rather than passing vacuously. The copy has to sit where its own `../src` and
//! `../crates` imports still resolve (its `./` sibling imports are rewritten to `../harness/`), and it must not sit anywhere the tree fingerprint
//! can see: `harness/` is a fingerprinted entry, so a copy written there is a file the
//! listing names for as long as it exists, and `evidence::tests` — same test binary,
//! running in parallel — read a tree the binary was not built from and failed. They
//! passed when run alone and failed under `cargo test --workspace`, which is the worst
//! possible shape for a fingerprint test.

use std::path::PathBuf;

/// A temporary copy of `name` with `from` replaced by `to`, removed when dropped.
pub fn harness_copy_with_in(name: &str, from: &str, to: &str) -> Mutant {
    let original = crate::runner::workspace_root().join("harness").join(name);
    let copy = crate::runner::harness_mutant(name);
    let text = std::fs::read_to_string(&original).expect("the harness is readable");
    assert!(
        text.contains(from),
        "the harness no longer holds `{from}`: the negative control would pass vacuously"
    );
    let dir = copy.parent().expect("the staging dir");
    std::fs::create_dir_all(dir).expect("staging dir");
    // `target/` is a sibling of `harness/`, so `../src` still resolves from the copy but a
    // harness-local `./x.mjs` does not: it is pointed back at `harness/`. Without this the
    // copy died on ERR_MODULE_NOT_FOUND and every negative control passed vacuously.
    let staged = text
        .replacen(from, to, 1)
        .replace("from \"./", "from \"../harness/");
    std::fs::write(&copy, staged).expect("the copy is writable");
    Mutant(copy)
}

/// The harness copy a negative control made, removed whether or not it was run again.
pub struct Mutant(pub PathBuf);

impl Mutant {
    pub fn cleanup(&self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

impl Drop for Mutant {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// A staged mutant must move neither the tree fingerprint nor this binary's verdict on
/// its own tree. Both halves are the same assertion read two ways, and both failed
/// together when the copy was staged under `harness/`.
#[test]
fn a_staged_mutant_moves_neither_the_fingerprint_nor_another_tests_verdict() {
    let root = crate::runner::workspace_root();
    let before = crate::evidence::tree_fingerprint().expect("the real tree fingerprints");
    let mutant = harness_copy_with_in(
        "wasm-tick-bench.mjs",
        "const SETTLE_TICKS = 112;",
        "const SETTLE_TICKS = 111;",
    );
    assert!(
        mutant.0.exists(),
        "the staged copy is what the control runs, so it must be there"
    );
    assert!(
        !crate::fingerprint::is_fingerprinted(&root, &mutant.0),
        "{} is inside a fingerprinted entry: staging it moves the tree",
        mutant.0.display()
    );
    assert_eq!(
        crate::evidence::tree_fingerprint().expect("the tree still fingerprints"),
        before,
        "a staged mutant must not move the tree fingerprint"
    );
    assert_eq!(
        crate::evidence::Stamp::take()
            .expect("built from this tree")
            .fingerprint(),
        before,
        "a staged mutant must not make this binary's own tree look like another"
    );
    mutant.cleanup();
}

/// Two live mutants of one harness are two files: dropping one leaves the other runnable.
#[test]
fn two_mutants_of_one_harness_do_not_share_a_path() {
    let stage = || {
        harness_copy_with_in(
            "wasm-tick-bench.mjs",
            "const SETTLE_TICKS = 112;",
            "const SETTLE_TICKS = 111;",
        )
    };
    let first = stage();
    let second = stage();
    assert_ne!(first.0, second.0);
    drop(first);
    assert!(second.0.exists(), "dropping one mutant deleted the other");
}
