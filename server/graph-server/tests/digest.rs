//! Row `svc-digest` (`docs/contract/service-api.md` "Verdict", condition 7): the seam the
//! service calls, on every row of the committed digest manifest.
//!
//! One row is `(fixture, source, layout, post)`. The row is run through `graph_server::motor`
//! — `graph_wasm::service`, the functions the wasm exports call too — and the SHA-256 of the
//! binary snapshot is compared with the manifest's, the way `graph-cli hashgate` hashes. A
//! refusal is pinned as `refused:<code>` and compared as a string, so a motor that starts
//! refusing is as loud as a motor that starts moving.
//!
//! What else the condition asks of the manifest, asserted here so the manifest cannot quietly
//! stop covering something:
//!
//! - both readers (`studio`, `contract`) appear, and every fixture's rows agree on one;
//! - every layout and POST id whose committed cap admits a fixture appears for it, computed
//!   from `Caps::committed()` and the motor's registries, so dropping a row fails;
//! - a fixture carries a connected component in spectral's 257–700 node window;
//! - the canonical JSON face reads back to the very bytes the binary face holds.
//!
//! The negative control is `GM_SVC_DIGEST_BREAK=1`, which flips one bit of every snapshot
//! before it is hashed (`seam::broken`): the digest rows then fail, and nothing else has to.
//!
//! `emit_the_manifest` is ignored and needs `GM_SVC_DIGEST_EMIT=1`. It is how a hash changes,
//! by hand and in the diff: it runs the rows, writes the digests it saw, and a human reads
//! the diff before committing it. It is not part of the gate.

mod common;

// The four children sit beside the fixtures rather than beside this file, so the `#[path]` is
// explicit: a crate root resolves a submodule in its own directory, and a bare `mod seam;`
// here would want `tests/seam.rs`. `crates/graph-core/tests/memory.rs:20` walks around the
// same trap, and names it.
#[path = "digest/component.rs"]
mod component;
#[path = "digest/coverage.rs"]
mod coverage;
#[path = "digest/manifest.rs"]
mod manifest;
#[path = "digest/seam.rs"]
mod seam;

use graph_server::caps::Caps;
use graph_server::motor;
use manifest::{COMPONENT_CEILING, COMPONENT_FLOOR, Entry, PATH};
use std::collections::BTreeSet;

/// The fixture the LOBPCG window is checked on: one component of 400 nodes.
const N400: &str = "server/graph-server/tests/digest/n400.json";

/// Every row of the manifest produces the digest the manifest pins.
#[test]
fn every_row_hashes_to_its_committed_digest() {
    let entries = manifest::entries();
    assert!(!entries.is_empty(), "{PATH} has no rows");
    let mut moved = Vec::new();
    for entry in &entries {
        let outcome = seam::run_row(entry, &common::fixture(&entry.fixture));
        let hashed = outcome.hash();
        if hashed != entry.hash {
            moved.push(format!(
                "{}: manifest {} got {hashed}",
                entry.line(),
                entry.hash
            ));
        }
    }
    assert!(
        moved.is_empty(),
        "{} of {} rows moved:\n{}",
        moved.len(),
        entries.len(),
        moved.join("\n")
    );
}

/// Both readers are covered, and each fixture is read by the reader its rows name.
#[test]
fn both_readers_are_covered() {
    let entries = manifest::entries();
    let sources: BTreeSet<&str> = entries
        .iter()
        .map(|entry| manifest::name_of(entry.source))
        .collect();
    assert_eq!(
        sources.iter().copied().collect::<Vec<&str>>(),
        vec!["contract", "studio"],
        "the manifest must cover both `source=` values, not {sources:?}"
    );
    for (fixture, rows) in coverage::by_fixture(&entries) {
        let readers: BTreeSet<&str> = rows
            .iter()
            .map(|entry| manifest::name_of(entry.source))
            .collect();
        assert_eq!(
            readers.len(),
            1,
            "{fixture}: its rows name more than one reader, {readers:?}"
        );
    }
    let fixtures: BTreeSet<&str> = entries.iter().map(|entry| entry.fixture.as_str()).collect();
    assert!(
        fixtures.len() >= 2,
        "the manifest reads {fixtures:?}: one reader's shape cannot check the other"
    );
}

/// Every id the service would answer a request for on this fixture is pinned: the layouts
/// whose cap admits it, exactly, and every POST pass whose cap admits it.
#[test]
fn every_id_whose_cap_admits_a_fixture_is_pinned() {
    let caps = Caps::committed().expect("the committed caps table parses");
    let entries = manifest::entries();
    for (fixture, rows) in coverage::by_fixture(&entries) {
        let bytes = common::fixture(fixture);
        let source = rows[0].source;
        let size = coverage::size(source, &bytes).unwrap_or_else(|why| panic!("{fixture}: {why}"));
        let layouts: BTreeSet<String> = rows
            .iter()
            .filter(|entry| entry.post.is_none())
            .map(|entry| entry.layout.clone())
            .collect();
        let wanted = coverage::admitting(motor::layout_ids(), &caps, size);
        assert_eq!(
            layouts, wanted,
            "{fixture} at {size:?}: the pinned layouts differ"
        );
        let posts: BTreeSet<String> = rows.iter().filter_map(|entry| entry.post.clone()).collect();
        let missing: Vec<String> = coverage::admitting(motor::post_ids(), &caps, size)
            .difference(&posts)
            .cloned()
            .collect();
        assert!(
            missing.is_empty(),
            "{fixture} at {size:?}: POST passes not pinned: {missing:?}"
        );
    }
}

/// A fixture in the manifest holds a connected component in the LOBPCG window, and it is the
/// one this test names: `n400.json`, one component of 400 nodes.
#[test]
fn a_pinned_fixture_holds_a_component_in_the_lobpcg_window() {
    let entries = manifest::entries();
    assert!(
        entries.iter().any(|entry| entry.fixture == N400),
        "{PATH} pins no {N400}: spectral's LOBPCG branch has no fixture to run on"
    );
    let largest = component::largest_component(&common::fixture(N400))
        .unwrap_or_else(|why| panic!("{N400}: {why}"));
    assert!(
        (COMPONENT_FLOOR..=COMPONENT_CEILING).contains(&largest),
        "{N400}: its largest component holds {largest} nodes, outside {COMPONENT_FLOOR}..={COMPONENT_CEILING}"
    );
}

/// The canonical JSON face reads back to the binary face's bytes, for one layout row and one
/// POST row of every fixture. Caveat: two rows per fixture, not all 94 — the round trip is the
/// same code over a different snapshot, and running every row twice would double the gate for
/// no new branch.
#[test]
fn the_json_face_round_trips_to_the_same_bytes() {
    let entries = manifest::entries();
    let mut checked = 0;
    for (fixture, rows) in coverage::by_fixture(&entries) {
        for entry in [first_layout(&rows), first_post(&rows)]
            .into_iter()
            .flatten()
        {
            let outcome = seam::run_row(entry, &common::fixture(fixture));
            let bytes = outcome
                .bytes()
                .unwrap_or_else(|why| panic!("{fixture}: {why}"));
            seam::json_round_trips(bytes).unwrap_or_else(|why| panic!("{}: {why}", entry.line()));
            checked += 1;
        }
    }
    assert_eq!(checked, 4, "two fixtures of two rows each: got {checked}");
}

/// The first row of `rows` that runs no POST pass.
fn first_layout<'a>(rows: &[&'a Entry]) -> Option<&'a Entry> {
    rows.iter().copied().find(|entry| entry.post.is_none())
}

/// The first row of `rows` that runs a POST pass.
fn first_post<'a>(rows: &[&'a Entry]) -> Option<&'a Entry> {
    rows.iter().copied().find(|entry| entry.post.is_some())
}

/// Rewrites the manifest from what the motor produces now, in its own order. Ignored, and
/// refused without `GM_SVC_DIGEST_EMIT=1`, so no gate row can edit the file it reads.
#[test]
#[ignore = "run by hand: GM_SVC_DIGEST_EMIT=1 cargo test --test digest -- --ignored"]
fn emit_the_manifest() {
    assert_eq!(
        std::env::var("GM_SVC_DIGEST_EMIT").as_deref(),
        Ok("1"),
        "set GM_SVC_DIGEST_EMIT=1: a plain run must not rewrite the manifest"
    );
    let previous = manifest::rows_to_emit();
    let entries: Vec<Entry> = previous
        .iter()
        .map(|entry| {
            let outcome = seam::run_row(entry, &common::fixture(&entry.fixture));
            Entry {
                hash: outcome.hash(),
                ..entry.clone()
            }
        })
        .collect();
    manifest::write(&entries);
    eprintln!("wrote {} rows to {PATH}", entries.len());
}
