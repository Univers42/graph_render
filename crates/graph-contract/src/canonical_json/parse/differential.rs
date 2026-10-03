//! The differential test for the reader itself: today's parser, frozen in
//! [`reference`], against the one that ships with the two shared scratch stacks. The output
//! must be the identical `Value` for every input and the identical `JsonError` for every
//! refusal, down to the byte offset.
//!
//! The ingest differential (`graph_wasm::ingest::differential`) is not enough on its own:
//! ingest looks a member up by name, so a repeated key that should be refused reads back as
//! the first value and the records still match. This module compares the trees.

use super::mutate::{ALL_EDITS, Rng};
use super::{MAX_DEPTH, Value, parse};
use crate::canonical_json::JsonError;

/// The repository's fixture directory, from this crate's manifest.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures");

/// Mutation attempts per run; the floor asserted below is 2000 applied. The four
/// structural edits cannot apply to a bare literal, so a fifth of the attempts are lost
/// to texts with no member in them.
const MUTATIONS: usize = 3200;

/// The fixed seed (D3: the only randomness here is a seeded generator).
const SEED: u32 = 0x5EED_1CE0;

/// Above this many members in one object the reader switches its duplicate-key check from
/// scanning the members to building a set of them; the corpus has to cross that line.
const WIDE: usize = 48;

#[test]
fn the_reader_agrees_with_the_frozen_one_on_every_fixture_document() {
    let documents = fixture_documents();
    assert!(
        !documents.is_empty(),
        "no document under {FIXTURES} to read"
    );
    for text in &documents {
        agree("fixture", text);
    }
}

#[test]
fn the_reader_agrees_with_the_frozen_one_on_seeded_mutations() {
    let pool = corpus();
    let mut rng = Rng::new(SEED);
    let mut applied = 0usize;
    let mut refused = 0usize;
    let mut per_edit = [0usize; 7];
    for i in 0..MUTATIONS {
        let edit = ALL_EDITS[i % ALL_EDITS.len()];
        let seed = &pool[rng.below(pool.len())];
        let Some(mutant) = super::mutate::apply(seed, edit, &mut rng) else {
            continue;
        };
        applied += 1;
        per_edit[i % ALL_EDITS.len()] += 1;
        if !agree(&format!("mutation {i} {edit:?}"), &mutant) {
            refused += 1;
        }
    }
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

/// The control arm: a harness that never fires proves nothing, so it is handed a tree it
/// must reject. Fails if the comparison ever calls two different readers' answers the same.
#[test]
fn the_comparison_rejects_a_reader_that_diverges() {
    let ok = parse("{\"a\":[1,2]}").expect("valid");
    assert!(
        disagreement(&Ok(ok.clone()), &Ok(ok.clone())).is_none(),
        "identical values were read as a divergence"
    );
    assert!(
        disagreement(&Ok(ok.clone()), &Ok(Value::Null)).is_some(),
        "a changed value was read as agreement"
    );
    let order: Vec<(String, Value)> = vec![("a".to_owned(), Value::Null)];
    assert!(
        disagreement(&Ok(ok.clone()), &Ok(Value::Object(order))).is_some(),
        "a changed member order was read as agreement"
    );
    assert!(
        disagreement(&Ok(ok), &Err(JsonError::Syntax { at: 0, what: "x" })).is_some(),
        "a refusal was read as agreement with a value"
    );
    let fault = JsonError::Syntax { at: 7, what: "x" };
    assert!(
        disagreement(&Err(fault.clone()), &Err(fault.clone())).is_none(),
        "the same refusal was read as a divergence"
    );
    assert!(
        disagreement(
            &Err(JsonError::Syntax { at: 7, what: "x" }),
            &Err(JsonError::Syntax { at: 8, what: "x" }),
        )
        .is_some(),
        "a different offset was read as agreement"
    );
}

/// The mutation corpus: the small documents below, each a corner of the grammar, five
/// copies each so the fixture-sized texts cannot crowd them out; then every fixture.
fn corpus() -> Vec<String> {
    let mut pool = Vec::new();
    for seed in small_documents() {
        for _ in 0..5 {
            pool.push(seed.clone());
        }
    }
    pool.extend(fixture_documents());
    pool
}

/// Every `.json` under [`FIXTURES`], sorted, so the corpus is the same set on every machine.
/// Nothing is filtered: a reader must agree with the frozen one on a refusal too.
fn fixture_documents() -> Vec<String> {
    let mut paths = Vec::new();
    collect_json(std::path::Path::new(FIXTURES), 0, &mut paths);
    paths.sort();
    paths
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
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

/// Every shape the reader accepts, plus the ones it must refuse: escapes, a surrogate pair
/// and an unpaired one, every number spelling, the empty containers, an object with a
/// repeated key, one wider than the set-building threshold, nesting at [`MAX_DEPTH`], and
/// whitespace between every token.
fn small_documents() -> Vec<String> {
    let mut out = vec![
        "null".to_owned(),
        "true".to_owned(),
        "false".to_owned(),
        "0".to_owned(),
        "-0.5e+10".to_owned(),
        "1E-9".to_owned(),
        "".to_owned(),
        "   ".to_owned(),
        "\"\"".to_owned(),
        "\"\\\"\\\\\\/\\b\\f\\n\\r\\t\"".to_owned(),
        "\"\\u0041\\u00e9\\u20ac\\ud83c\\udf3f\"".to_owned(),
        "\"\\ud800\"".to_owned(),
        "\"\\udc00x\"".to_owned(),
        "\"\\uD83C\\uDF3F\"".to_owned(),
        "\"raw\ttab\"".to_owned(),
        "[]".to_owned(),
        "{}".to_owned(),
        "[ ] , ".to_owned(),
        "[[[[[[[[[[]]]]]]]]]]".to_owned(),
        r#"{"a":1,"a":2}"#.to_owned(),
        r#"{"a":1,"b":2,"a":3}"#.to_owned(),
        r#"{"a":{"b":{"c":[1,[2,[3]]]}}}"#.to_owned(),
        r#" { "a" : [ 1 , 2 ] , "b" : { } } "#.to_owned(),
    ];
    // One object narrower than the threshold and one wider, so both arms of the duplicate
    // key check are read by every corpus run.
    out.push(members(WIDE - 2, false));
    out.push(members(WIDE, false));
    out.push(members(WIDE + 20, true));
    // Nesting exactly at the limit and one past it.
    out.push(nest(MAX_DEPTH as usize - 1));
    out.push(nest(MAX_DEPTH as usize + 1));
    out
}

fn members(count: usize, repeat_last: bool) -> String {
    let mut out = String::from("{");
    for i in 0..count {
        if i > 0 {
            out.push(',');
        }
        out.push_str("\"k");
        out.push_str(&i.to_string());
        out.push_str("\":");
        out.push_str(&i.to_string());
    }
    if repeat_last && count > 2 {
        out.push_str(",\"k1\":99");
    }
    out.push('}');
    out
}

fn nest(depth: usize) -> String {
    format!("{}0{}", "[".repeat(depth), "]".repeat(depth))
}

/// Fails with both readers' answers named, on the text that produced them. Returns whether
/// the text was accepted, so the corpus can count refusals without reading it twice.
fn agree(name: &str, text: &str) -> bool {
    let new = parse(text);
    let old = super::reference::parse(text);
    if let Some(why) = disagreement(&new, &old) {
        panic!("{name}: {why}\ntext: {}", preview(text));
    }
    new.is_ok()
}

/// `None` when the two readers agree on the text; the first difference when they do not.
fn disagreement(new: &Result<Value, JsonError>, old: &Result<Value, JsonError>) -> Option<String> {
    match (new, old) {
        (Ok(a), Ok(b)) if a == b => None,
        (Ok(a), Ok(b)) => Some(format!("values differ: {a:?} vs {b:?}")),
        (Err(a), Err(b)) if a == b => None,
        (Err(a), Err(b)) => Some(format!("different refusals: {a:?} vs {b:?}")),
        (new, old) => Some(format!("one read, the other refused: {new:?} vs {old:?}")),
    }
}

/// The first 400 characters, so a failure names its input without printing a megabyte.
fn preview(text: &str) -> String {
    text.chars().take(400).collect()
}
