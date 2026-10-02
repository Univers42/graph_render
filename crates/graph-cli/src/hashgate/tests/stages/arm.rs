//! The tests that drive the **shipped** wasm arm: `harness/wasm-run.mjs` under Node,
//! over the real `graph_wasm.wasm` `build_wasm` makes. Split out of `stages.rs` by the
//! house's 300-line limit and because they are the only tests in this module that need a
//! second program and a real artifact — everything beside them is pure Rust over
//! `graph_core::registry`.
//!
//! What they hold together: **the arm's stage resolution is one derivation, and a stage
//! it cannot hash is refused by name.** The two arms of the gate derive their stage
//! knowledge from different places — this module from `graph_core::registry::LAYOUTS`,
//! the harness from `gm_layout_count`/`gm_layout_id` — so the only thing keeping them
//! honest is that they agree, and the only thing keeping the arm honest is that it
//! refuses a name it does not have rather than hashing something else under it.

use super::super::super::LAYOUT;
use super::super::super::stages::stages as stage_ids;
use crate::runner::{build_wasm, node_harness, run_lines};
use std::path::Path;

/// The real wasm artifact — the same build `hashgate` makes before it drives the arm, so
/// these tests never read a stale or hand-made module. One per test, not one per
/// invocation: `build_wasm` is a cargo build, and paying for it nine times to test nine
/// stage names is not what this test is about.
fn artifact() -> std::path::PathBuf {
    build_wasm(&[]).expect("the wasm artifact")
}

/// `wasm-run.mjs <wasm> hash 1 <stage>`, once: its stdout lines on success, or the failure
/// text `run_lines` builds from the child's status and stderr.
fn hashed(wasm: &Path, stage: &str) -> Result<Vec<String>, String> {
    let mut command = node_harness(wasm).expect("node on PATH");
    command.args(["hash", "1", stage]);
    run_lines(&mut command)
}

#[test]
fn the_wasm_arm_can_hash_every_stage_the_gate_asks_for() {
    // The two arms derive their stage knowledge from different sources — this module from
    // `graph_core::registry`, the harness from `gm_layout_count`/`gm_layout_id` — so the
    // one thing that keeps them honest is that they agree. Reads the shipped
    // `harness/wasm-run.mjs` `stages` mode, so it needs the real wasm artifact; it is the
    // same build `hashgate` makes before it drives the arm.
    let mut command = node_harness(&artifact()).expect("node on PATH");
    command.arg("stages");
    let offered: Vec<String> = run_lines(&mut command).expect("the arm's stage list");
    let offered: Vec<&str> = offered
        .iter()
        .map(String::as_str)
        .filter(|l| !l.trim().is_empty())
        .collect();
    for stage in stage_ids() {
        assert!(
            offered.contains(&stage),
            "the wasm arm cannot hash {stage}; it offers {offered:?}"
        );
    }
    // And nothing more: an extra id is a stage the native arm has no bytes for.
    assert_eq!(offered.len(), stage_ids().len(), "{offered:?}");
}

fn assert_refused(wasm: &Path, stage: &str) {
    let refused = hashed(wasm, stage).expect_err("an unknown stage is refused, never hashed");
    assert!(
        refused.contains(&format!("unknown stage {stage}")),
        "{stage}: {refused}"
    );
    assert!(
        refused.contains("exited exit status: 2"),
        "{stage}: could-not-run is exit 2, never a raw trap: {refused}"
    );
}

fn assert_hashable(wasm: &Path, stage: &str) {
    assert!(hashed(wasm, stage).is_ok(), "{stage} should be hashable");
}

#[test]
fn a_stage_the_arm_cannot_hash_is_refused_however_it_is_named() {
    // The three shim-backed stages live in a JS object literal, and an object literal
    // inherits from `Object.prototype`: `STAGE_BYTES["toString"]` used to answer with
    // `Object.prototype.toString` rather than `undefined`, so a stage named after a
    // prototype member was *hashed* — `toString` printed a digest of the string
    // `"[object Undefined]"` and exited 0, `__proto__` died on `bytesOf is not a
    // function` and exited 1. Either way a stage the module does not have produced a
    // verdict instead of a refusal, which is the one thing the gate's own stage list must
    // never do: a green line for a stage nobody ran. Every one of these is refused by
    // name, with the harness's own exit 2 for "could not run".
    let wasm = artifact();
    for stage in [
        "toString",
        "constructor",
        "__proto__",
        "hasOwnProperty",
        "valueOf",
        "layout.not.registered",
    ] {
        assert_refused(&wasm, stage);
    }
    // And the control, in the other direction: the same shape of name that the shim
    // table *does* carry is hashed, so the refusals above are about the name being
    // unknown and not about the harness refusing everything.
    let hashed_grid = hashed(&wasm, LAYOUT).expect("the shim-backed stage is hashable");
    assert_eq!(hashed_grid.len(), 1, "one line per seed: {hashed_grid:?}");
    assert!(
        hashed_grid[0].starts_with(&format!("{LAYOUT} 0 ")),
        "{hashed_grid:?}"
    );
    assert_hashable(&wasm, "topology");
    assert_hashable(&wasm, "transport.wasm.columnar");
}
