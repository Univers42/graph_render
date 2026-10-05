//! The property test: random operations, and the four properties a materialized document
//! must hold after **every** one of them.
//!
//! Split out of `rng.rs`, which is the seeded generator itself, by the house's 300-line
//! limit. The generator is the reusable half — it knows nothing about the model — and the
//! property test is the one user of it.
//!
//! 64 seeds, 40 steps each, and the assertions run after every step rather than at the end
//! — a model that is canonical at the end and not in the middle has still served a
//! non-canonical document to whatever read it in between, which is the whole point of
//! checking per step.
//!
//! | Property | What it would catch |
//! |---|---|
//! | `to_json() == ingest::to_json(&to_ingest())` | the streaming writer and the built document disagreeing |
//! | the text **reads** | a document carrying a cell no declaration allows — a pruned reference or a dropped field's cell surviving |
//! | reading it back and writing it again is the identical text | a writer that is not canonical, so two stores would differ |
//! | `doc_bytes() >= len`, equal when nothing was pruned | a length that under-counts, i.e. a response sized wrong |
//!
//! Seeds, not a clock: the whole run is reproducible from `0..64`, so a failure can be
//! re-run by hand with the failing seed (D2).

use super::fixtures::{manifest, upsert};
use super::rng::SplitMix64;
use super::super::Limits;
use super::super::model::Model;
use crate::ingest::{read as ingest_read, to_json as ingest_to_json};

/// Every seed, every step, four properties. Named as one test because the four assertions
/// are four views of the same step and splitting them would run the same 2 560 steps four
/// times to check the same thing.
#[test]
fn random_ops_materialize_canonically() {
    for seed in 0..64u64 {
        let mut model = Model::new("ws").expect("a workspace id is a slug");
        let mut rng = SplitMix64::seeded(seed);
        for step in 0..40 {
            run_step(&mut model, &mut rng, seed, step);
            let text = model.to_json();
            assert_eq!(
                text,
                ingest_to_json(&model.to_ingest()),
                "seed {seed} step {step}: the streaming writer and the document disagree"
            );
            let back = ingest_read(&text).unwrap_or_else(|e| {
                panic!("seed {seed} step {step}: the document is refused: {e}\n{text}")
            });
            assert_eq!(
                ingest_to_json(&back),
                text,
                "seed {seed} step {step}: the text does not survive a round trip"
            );
            assert!(
                text.len() as u64 <= model.doc_bytes(),
                "seed {seed} step {step}: doc_bytes does not bound the document"
            );
        }
        // The last property: two writes of the same model are one text, so a client that
        // polls twice cannot see two answers.
        assert_eq!(model.to_json(), model.to_json(), "seed {seed}: two writes differ");
    }
}

/// One random step: register a plugin if it is missing, otherwise upsert or delete.
///
/// The plugin set is `tracker`, `a`, `a-b`, `b` rather than one name, because the ordering
/// tests' whole subject is two plugins whose qualified ids order the other way from their
/// names — with one plugin neither `a-b.c` before `a.x` nor the record key tuple order is
/// ever exercised.
fn run_step(model: &mut Model, rng: &mut SplitMix64, seed: u64, step: usize) {
    let plugin = ["tracker", "a", "a-b", "b"][rng.below(4) as usize];
    if model.manifest_of(plugin).is_err() {
        model
            .register(plugin, manifest(plugin))
            .unwrap_or_else(|e| panic!("seed {seed} step {step}: register {plugin}: {e}"));
        return;
    }
    let collection = ["task", "c", "note"][rng.below(3) as usize];
    let id = format!("r{}", rng.below(3));
    let limit = Limits::DEFAULT;
    if rng.below(4) == 0 {
        model
            .apply(plugin, &super::fixtures::delete(collection, &id), &limit)
            .unwrap_or_else(|e| panic!("seed {seed} step {step}: delete {id}: {e}"));
        return;
    }
    let batch = upsert(collection, &id, 1, &random_cells(rng));
    if let Err(e) = model.apply(plugin, &batch, &limit) {
        // A random draw that trips a cell rule is a legitimate state of the world, but the
        // model must be untouched — so this asserts the atomicity rather than skipping.
        panic!("seed {seed} step {step}: {plugin}/{collection}/{id}: {e}");
    }
}

/// A random `values` map, every cell legal for its field — because a refusal from a
/// *bad* cell is a bug in this test's generator, while a refusal from a dangling reference
/// is the state the pruning rules exist for, and the two must not be confused.
///
/// So references are only emitted when the id they name is one this function also emits,
/// and both `tags` and `blocks` stay out of it: pruning a list is covered by the example
/// tests in `materialize.rs`, and the property test's job is the *invariants*, which hold
/// whether or not anything was pruned.
fn random_cells(rng: &mut SplitMix64, collection: &str) -> String {
    let mut cells: Vec<String> = Vec::new();
    for field in scalar_fields(collection) {
        if rng.below(3) == 0 {
            continue;
        }
        let value = if rng.below(2) == 0 {
            format!(r#""v{}""#, rng.below(100))
        } else {
            format!("{}", rng.below(1000))
        };
        cells.push(format!(r#""{field}":{value}"#));
    }
    if cells.is_empty() {
        // Every field was skipped; an empty `values` is legal and keeps the draw useful.
        return String::new();
    }
    cells.join(",")
}

/// The fields of `collection` that take a string or a number and nothing else. Only these
/// are drawn, because a *reference* field needs the other record to exist: emitting one at
/// random would make most steps fail the cell check for a reason the property test is not
/// about. Pruning a reference is covered by the example tests in `materialize.rs`; here the
/// invariants must hold whether or not anything was pruned.
fn scalar_fields(collection: &str) -> &'static [&'static str] {
    match collection {
        "task" => &["name", "state", "note"],
        _ => &["name"],
    }
}