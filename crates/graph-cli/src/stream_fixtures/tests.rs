//! What the stream fixtures have to be before `force-gate`'s stream stage can be a check
//! at all: the same bytes every time, documents the real ABI reader accepts line by line,
//! and the node counts their shape claims — because a fixture whose lines do not build is a
//! gate that exits 2 and proves nothing, and one that silently grows to the wrong size is a
//! gate over a graph nobody wrote down.

use super::*;
use graph_wasm::service::{self, Source};

/// Every fixture's lines, as the reader sees them.
fn lines_of(name: &str) -> Vec<String> {
    std::fs::read_to_string(fixture_path(name))
        .unwrap_or_else(|e| panic!("{}: {e}; run emit-stream-fixtures", fixture_path(name).display()))
        .lines()
        .map(str::to_owned)
        .collect()
}

/// The node count of `document`, read back through the ABI's own reader rather than the
/// builder's counter: the shape claim is about what a consumer sees.
fn nodes_in(document: &str) -> usize {
    serde_json::from_str::<serde_json::Value>(document).expect("a JSON document")["nodes"]
        .as_array()
        .expect("a nodes array")
        .len()
}

fn edges_in(document: &str) -> usize {
    serde_json::from_str::<serde_json::Value>(document).expect("a JSON document")["edges"]
        .as_array()
        .expect("an edges array")
        .len()
}

/// Emitting twice must give the same bytes: a fixture that carried a clock, a hash order or
/// an allocator address would make `fixtures-fresh` — which regenerates and diffs — fail on
/// a clean tree for no reason, and would make the gate's own digest unrepeatable.
#[test]
fn emitting_twice_gives_identical_bytes() {
    for name in FIXTURES {
        assert_eq!(
            bytes(name).expect("writes"),
            bytes(name).expect("writes again"),
            "{name} is not deterministic"
        );
    }
}

/// The first line builds and every later line extends, through the very functions the wasm
/// arm calls. This is the whole reason the fixtures are JSON v1 rather than a private
/// encoding: a reader disagreement is a fixture that cannot be compared at all.
#[test]
fn every_fixture_builds_line_zero_and_extends_every_later_line() {
    for name in FIXTURES {
        let documents = lines_of(name);
        assert!(documents.len() >= 2, "{name}: no batches to compare");
        let mut topology = service::build(documents[0].as_bytes(), Source::Ingest)
            .unwrap_or_else(|c| panic!("{name} line 0: {}", c.name()));
        for (batch, document) in documents.iter().enumerate().skip(1) {
            service::extend(&mut topology, document.as_bytes())
                .unwrap_or_else(|c| panic!("{name} batch {batch}: {}", c.name()));
        }
    }
}

/// The node counts the three fixtures claim, line by line. `stream-pow2`'s are the
/// interesting ones: the batch that crosses 64 and the one that crosses 128 are what a
/// growth path's capacity handling is tested against, and a fixture that missed either
/// would leave those paths unexercised while still looking like a stream.
#[test]
fn node_counts_match_the_shape_each_fixture_claims() {
    let expected: [(&str, &[usize]); 3] = [
        ("stream-small", &[50, 60, 70, 80, 90, 100, 110, 120, 130]),
        ("stream-hub", &[21, 21, 21, 21, 21, 21]),
        ("stream-pow2", &[60, 70, 70, 128, 129, 149]),
    ];
    for (name, counts) in expected {
        let documents = lines_of(name);
        assert_eq!(documents.len(), counts.len(), "{name}: line count");
        let got: Vec<usize> = documents.iter().map(|d| nodes_in(d)).collect();
        let running: Vec<usize> = counts
            .iter()
            .scan(0usize, |total, want| {
                *total = *total + want;
                Some(*total)
            })
            .collect();
        assert_eq!(got, running, "{name}: nodes per line");
    }
}

/// The hub fixture's two features that are not "nodes arrived": parallel edges under fresh
/// ids, and one self-loop. Both are refused-or-accepted facts about `Topology::extend`, and
/// a fixture that quietly lost either would leave the reader untested on exactly the input
/// it is most likely to get wrong.
#[test]
fn the_hub_fixture_carries_parallel_edges_and_exactly_one_self_loop() {
    let documents = lines_of("stream-hub");
    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut loops = 0;
    for (batch, document) in documents.iter().enumerate() {
        let value: serde_json::Value = serde_json::from_str(document).expect("JSON");
        for edge in value["edges"].as_array().expect("edges") {
            let pair = (
                edge["source"].as_str().expect("source").to_owned(),
                edge["target"].as_str().expect("target").to_owned(),
            );
            if pair.0 == pair.1 {
                loops += 1;
                assert_eq!(batch, 3, "the self-loop belongs to batch 3");
            }
            pairs.push(pair);
        }
    }
    let unique = {
        let mut sorted = pairs.clone();
        sorted.sort();
        sorted.dedup();
        sorted.len()
    };
    assert!(
        pairs.len() - unique >= 3,
        "the hub fixture must carry at least 3 parallel edges, it carries {}",
        pairs.len() - unique
    );
    assert_eq!(loops, 1, "exactly one self-loop");
}

/// `stream-pow2`'s empty batch is a real line, not an absence: an empty document is the one
/// a batch of "no new nodes and no new edges" is, and a reader that refused it would never
/// let the following growth be attempted.
#[test]
fn the_pow2_fixture_has_one_empty_batch_between_the_two_that_grow() {
    let documents = lines_of("stream-pow2");
    let empty: Vec<usize> = documents
        .iter()
        .enumerate()
        .filter(|(_, d)| nodes_in(d) == 0 && edges_in(d) == 0)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(empty, vec![2], "one empty batch, and it is the third line");
}

/// Ids are `n<i>` and `e<j>` in creation order, which is what makes a report that says
/// "batch 2" readable: a reader looking at a divergence opens the fixture and finds the
/// rows the words name. Zero-padded ids would still be deterministic but would read as a
/// different numbering than the force gate prints.
#[test]
fn ids_are_unpadded_decimals_in_creation_order() {
    let documents = lines_of("stream-small");
    let mut nodes = 0;
    let mut edges = 0;
    for document in &documents {
        let value: serde_json::Value = serde_json::from_str(document).expect("JSON");
        for node in value["nodes"].as_array().expect("nodes") {
            assert_eq!(node["id"], serde_json::json!(format!("n{nodes}")));
            nodes += 1;
        }
        for edge in value["edges"].as_array().expect("edges") {
            assert_eq!(edge["id"], serde_json::json!(format!("e{edges}")));
            edges += 1;
        }
    }
    assert_eq!(nodes, 130);
    assert!(edges >= 130, "a ring, then two edges per new node");
}

/// Every batch edge names a node the graph already holds or the batch itself brings —
/// `Topology::extend`'s third refusal is a dangling endpoint, and the fixture is what keeps
/// that refusal for something the gate is actually testing.
#[test]
fn no_batch_edge_names_a_node_that_does_not_exist_yet() {
    for name in FIXTURES {
        let documents = lines_of(name);
        let mut known: Vec<String> = Vec::new();
        for document in &documents {
            let value: serde_json::Value = serde_json::from_str(document).expect("JSON");
            let mut arriving: Vec<String> = Vec::new();
            for edge in value["edges"].as_array().expect("edges") {
                for end in ["source", "target"] {
                    let id = edge[end].as_str().expect("endpoint").to_owned();
                    assert!(
                        known.contains(&id) || arriving.contains(&id),
                        "{name}: {end} {id} names no node"
                    );
                }
            }
            for node in value["nodes"].as_array().expect("nodes") {
                arriving.push(node["id"].as_str().expect("id").to_owned());
            }
            known.extend(arriving);
        }
    }
}

/// The bytes end in a newline and hold no blank line, because a reader that splits on
/// `\n\n` — or a diff that shows a trailing empty line — is a reader this fixture set has
/// just made harder for no reason.
#[test]
fn every_fixture_is_newline_terminated_json_lines() {
    for name in FIXTURES {
        let raw = std::fs::read(fixture_path(name)).expect("readable");
        assert!(raw.ends_with(b"\n"), "{name}: no final newline");
        assert_eq!(
            String::from_utf8_lossy(&raw).matches("\n\n").count(),
            0,
            "{name}: a blank line"
        );
    }
}