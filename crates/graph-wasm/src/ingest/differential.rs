//! The differential test for the open path: today's reader, frozen, against the one that
//! ships. `ingest::read` consumes the parsed tree by value now (moving each string out of
//! its `Value` and dropping each record's element as it goes, instead of borrowing the
//! tree and `.to_owned()`ing every field), which is a change of ownership, not of meaning.
//! This module is the proof that it is only that: the same `Result` — records, or the
//! same `IngestError` with the same text — for the same bytes.
//!
//! Two corpora. Every document under `fixtures/` that the frozen reader accepts, unchanged.
//! And [`MUTATIONS`] seeded mutations of those plus the small documents in
//! [`small_seeds`], across eight edits: a flipped byte, a truncation, a member deleted,
//! renamed or duplicated, a value's type swapped, an `id` duplicated, an endpoint renamed.
//! The seed is fixed and the generator is `mulberry32`, so a failure names an index that
//! reproduces the same bytes on any machine.

mod mutate;
mod reference;

use crate::ingest::{IngestError, read};
use graph_core::{EdgeRecord, NodeRecord};
use mutate::{ALL_EDITS, Rng};

/// The repository's fixture directory, from this crate's manifest.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures");

/// Mutation attempts per run. The brief's floor is 2000 *applied* mutations, so this sits
/// above it and the test asserts the applied count, not the attempted one.
const MUTATIONS: usize = 2400;

/// The fixed seed (D3: the only randomness in this crate is a seeded generator).
const SEED: u32 = 0x5EED_1CE0;

/// What a pair of readers returned: the whole `Result`, records or refusal.
type Records = (Vec<NodeRecord>, Vec<EdgeRecord>);
type Read = Result<Records, IngestError>;

#[test]
fn the_reader_agrees_with_the_frozen_one_on_every_accepted_fixture() {
    let documents = fixture_documents();
    assert!(
        !documents.is_empty(),
        "no document under {FIXTURES} is an ingest document"
    );
    for bytes in &documents {
        agree(&preview(bytes), bytes);
    }
}

#[test]
fn the_reader_agrees_with_the_frozen_one_on_seeded_mutations() {
    let pool = mutation_pool();
    let mut rng = Rng::new(SEED);
    let mut applied = 0usize;
    let mut refused = 0usize;
    let mut per_edit = [0usize; 8];
    for i in 0..MUTATIONS {
        let edit = ALL_EDITS[i % ALL_EDITS.len()];
        let seed = String::from_utf8_lossy(&pool[rng.below(pool.len())]).into_owned();
        let Some(mutant) = mutate::apply(&seed, edit, &mut rng) else {
            continue;
        };
        applied += 1;
        per_edit[i % ALL_EDITS.len()] += 1;
        if !agree(&format!("mutation {i} {edit:?}"), mutant.as_bytes()) {
            refused += 1;
        }
    }
    // A corpus that only parses proves nothing about the refusal paths, and one where an
    // edit never applied proves nothing about that edit.
    assert!(
        applied >= 2000,
        "only {applied} of {MUTATIONS} mutations applied"
    );
    assert!(refused >= 100, "only {refused} mutations were refused");
    assert!(
        per_edit.iter().all(|n| *n > 0),
        "an edit never applied: {per_edit:?}"
    );
}

/// The control arm: a harness that never fires proves nothing, so it is handed a pair it
/// must reject. This fails if `disagreement` ever returns `None` for two readers that
/// differ — the exact bug that would make the differential test above vacuous.
#[test]
fn the_comparison_rejects_a_reader_that_diverges() {
    let ok = reference::read(small_seeds()[0].as_bytes()).expect("a valid seed");
    let accepted = Ok(ok.clone());
    let mut moved = ok.clone();
    moved.0[0].label.push('!');
    assert!(
        disagreement(&accepted, &Ok(moved)).is_some(),
        "a changed label was read as agreement"
    );
    let mut dropped = ok.clone();
    dropped.1.clear();
    assert!(
        disagreement(&accepted, &Ok(dropped)).is_some(),
        "a lost edge was read as agreement"
    );
    let refused: Read = Err(IngestError::Shape("a: different".to_owned()));
    assert!(
        disagreement(&accepted, &refused).is_some(),
        "a refusal was read as agreement with accepted records"
    );
    assert!(
        disagreement(&refused, &refused).is_none(),
        "the same refusal was read as a divergence"
    );
}

/// The mutation corpus: five copies of each small document, so the 100 kB fixture cannot
/// crowd the structural edits out of the budget, then every accepted fixture.
fn mutation_pool() -> Vec<Vec<u8>> {
    let mut pool = Vec::new();
    for seed in small_seeds() {
        for _ in 0..5 {
            pool.push(seed.clone().into_bytes());
        }
    }
    pool.extend(fixture_documents());
    pool
}

/// Every `.json` under [`FIXTURES`] the frozen reader accepts, in sorted path order so the
/// corpus is the same set on every machine. Filtered by the frozen reader, not the new
/// one: a change that starts refusing a fixture must fail the test, not shrink the corpus.
fn fixture_documents() -> Vec<Vec<u8>> {
    let mut paths = Vec::new();
    collect_json(std::path::Path::new(FIXTURES), 0, &mut paths);
    paths.sort();
    paths
        .iter()
        .filter_map(|path| std::fs::read(path).ok())
        .filter(|bytes| reference::read(bytes).is_ok())
        .collect()
}

fn collect_json(dir: &std::path::Path, depth: u32, out: &mut Vec<std::path::PathBuf>) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_json(&path, depth + 1, out);
        } else if path.extension().is_some_and(|e| e == "json") {
            out.push(path);
        }
    }
}

/// Small ingest documents, each a different corner of the shape: a node with every
/// optional member filled and one left null, an edge with `child_first` and one without
/// (a pre-p3 document), a duplicate id, a dangling endpoint, a document whose `nodes` is
/// an object rather than an array.
fn small_seeds() -> Vec<String> {
    let node = |id: &str| {
        format!(
            r#"{{"id":"{id}","kind":"record","database_id":"db","source":"pg","label":"L{id}","group":"G","weight":0.5,"version":0.0,"has_note":false,"icon":"icon:map"}}"#
        )
    };
    let bare = |id: &str| {
        format!(
            r#"{{"id":"{id}","kind":"note","database_id":null,"source":"pg","label":"L{id}","group":null,"weight":1.0,"version":0.0,"has_note":true,"icon":null}}"#
        )
    };
    let edge = |id: &str, from: &str, to: &str| {
        format!(
            r#"{{"id":"{id}","source":"{from}","target":"{to}","kind":"relation","label":"","strength":0.5,"directed":false,"record_id":null,"child_first":false}}"#
        )
    };
    let doc = |nodes: &[String], edges: &[String]| {
        format!(
            r#"{{"version":1,"nodes":[{}],"edges":[{}]}}"#,
            nodes.join(","),
            edges.join(",")
        )
    };
    vec![
        doc(&[node("a"), bare("b")], &[edge("e", "a", "b")]),
        doc(&[node("a"), node("b")], &[]),
        doc(&[], &[]),
        r#"{"version":1,"nodes":[]}"#.to_owned(),
        r#"{"version":2,"nodes":[],"edges":[]}"#.to_owned(),
        doc(&[node("a"), node("a")], &[]),
        doc(&[node("a")], &[edge("e", "a", "ghost")]),
        doc(
            &[node("a"), node("b")],
            &[edge("e", "a", "b").replace(r#","child_first":false"#, "")],
        ),
        r#"{"version":1,"nodes":{"a":1},"edges":[]}"#.to_owned(),
        r#"{"version":1,"nodes":[{"id":"a"}],"edges":[]}"#.to_owned(),
        // Two members of one record of the wrong type each, so the order the refusals come
        // out in is pinned: which of the two is named is a property of the reader, and a
        // single-member mutation can never see it.
        doc(
            &[node("a")],
            &[edge("e", "a", "a")
                .replace(r#""directed":false"#, r#""directed":"x""#)
                .replace(r#""child_first":false"#, r#""child_first":"y""#)],
        ),
        doc(
            &[node("a")
                .replace(r#""id":"a""#, r#""id":1"#)
                .replace(r#""source":"pg""#, r#""source":2"#)],
            &[],
        ),
    ]
}

/// Fails with both readers' answers named, on the bytes that produced them. Returns
/// whether the document was accepted, so the mutation corpus can count its refusals
/// without reading every mutant twice.
fn agree(name: &str, bytes: &[u8]) -> bool {
    let new = read(bytes);
    let old = reference::read(bytes);
    if let Some(why) = disagreement(&new, &old) {
        panic!("{name}: {why}\nbytes: {}", preview(bytes));
    }
    new.is_ok()
}

/// `None` when the two readers agree on the bytes; the first difference when they do not.
fn disagreement(new: &Read, old: &Read) -> Option<String> {
    match (new, old) {
        (Ok(a), Ok(b)) if a == b => None,
        (Ok(a), Ok(b)) => Some(format!(
            "records differ at node {}, edge {} ({} nodes/{} edges vs {} / {})",
            first(&a.0, &b.0).unwrap_or(a.0.len()),
            first(&a.1, &b.1).unwrap_or(a.1.len()),
            a.0.len(),
            a.1.len(),
            b.0.len(),
            b.1.len()
        )),
        (Err(a), Err(b)) if a == b => None,
        (Err(a), Err(b)) => Some(format!("different refusals: {a:?} vs {b:?}")),
        (new, old) => Some(format!(
            "one accepted, the other refused: {new:?} vs {old:?}"
        )),
    }
}

/// The index of the first record that differs, or `None` when they are all equal.
fn first<T: PartialEq>(a: &[T], b: &[T]) -> Option<usize> {
    a.iter().zip(b).position(|(x, y)| x != y)
}

/// The first 400 bytes, lossy, so a failure names its input without printing a megabyte.
fn preview(bytes: &[u8]) -> String {
    String::from_utf8_lossy(&bytes[..bytes.len().min(400)]).into_owned()
}
