use super::*;
use std::path::PathBuf;

/// A temporary directory this test owns, emptied first.
fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("gm-layout-fixtures-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// `write`'s two files, exactly: one line per seed, each that seed's own `fixture_line`,
/// and a manifest pinning the digest, the seed count, the format and the tree it was
/// written from. The harness reads the manifest and refuses anything else.
#[test]
fn writing_the_fixtures_pins_every_line_and_the_manifest_that_covers_them() {
    let dir = scratch("write");
    let stamp = crate::evidence::Stamp::take().expect("this tree");
    write(3, &dir, &stamp).expect("writes");
    let text = std::fs::read_to_string(dir.join("layouts.jsonl")).expect("layouts.jsonl");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3, "one line per seed");
    for (i, line) in lines.iter().enumerate() {
        let value: Value = serde_json::from_str(line).expect("json line");
        assert_eq!(value, fixture_line(i as u32).expect("valid"), "seed {i}");
    }
    let manifest: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("layout-manifest.json")).expect("manifest"),
    )
    .expect("manifest json");
    assert_eq!(
        manifest["generator"],
        Value::from("graph-cli emit-fixtures: the gate model, graph_core::seeded_model")
    );
    assert_eq!(manifest["format"], 1);
    assert_eq!(manifest["seeds"], 3);
    assert_eq!(
        manifest["sha256"]["layouts.jsonl"],
        file_sha256(&dir.join("layouts.jsonl")).expect("digest")
    );
    assert_eq!(manifest["fingerprint"], stamp.fingerprint());
    assert_eq!(manifest["fingerprinted"], json!(FINGERPRINTED));
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

/// The manifest is a manifest of *these* bytes: a second run writes the same ones, and a
/// file that moved under it is a different file.
#[test]
fn the_manifest_digest_follows_the_bytes_it_covers() {
    let dir = scratch("digest");
    let stamp = crate::evidence::Stamp::take().expect("this tree");
    write(2, &dir, &stamp).expect("writes");
    let read = |name: &str| -> Value {
        serde_json::from_str(&std::fs::read_to_string(dir.join(name)).expect(name)).expect(name)
    };
    let first = read("layout-manifest.json");
    write(2, &dir, &stamp).expect("writes again");
    assert_eq!(
        read("layout-manifest.json"),
        first,
        "same seeds, same bytes"
    );
    std::fs::write(dir.join("layouts.jsonl"), "{}").expect("tamper");
    assert_ne!(
        read("layout-manifest.json")["sha256"]["layouts.jsonl"],
        file_sha256(&dir.join("layouts.jsonl")).expect("digest")
    );
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

/// A run that cannot write is a could-not-run, not an empty manifest: the harness would
/// otherwise read "0 seeds" as agreement.
#[test]
fn a_directory_that_does_not_exist_is_refused_rather_than_written_around() {
    let dir = scratch("missing").join("no-such-dir");
    let stamp = crate::evidence::Stamp::take().expect("this tree");
    let err = write(1, &dir, &stamp).expect_err("no such directory");
    assert!(err.contains("layouts.jsonl"), "{err}");
}

#[test]
fn a_seeds_line_carries_a_tree_and_both_layouts_arrays_the_same_length() {
    let line = fixture_line(7).expect("valid");
    let tree = &line["tree"];
    assert!(tree["children"].is_array());
    let n = graph_core::gate_node_count(7) as usize;
    for face in ["tidy", "treemap"] {
        let x = line[face]["x"].as_array().expect("x");
        assert_eq!(x.len(), n, "{face}");
    }
    assert_eq!(line["treemap"]["w"].as_array().expect("w").len(), n);
}

/// The gate model's remix scrambles hierarchy orientation freely, so every seed in this
/// small range draws two or more real roots (checked directly, not assumed).
#[test]
fn the_virtual_root_carries_no_id_and_no_weight_when_there_are_two_roots() {
    for seed in 0..10 {
        let line = fixture_line(seed).expect("valid");
        assert!(line["tree"]["id"].is_null(), "seed {seed}: {line}");
        assert!(line["tree"]["weight"].is_null(), "seed {seed}: {line}");
    }
}

#[test]
fn every_real_node_in_the_tree_has_a_finite_weight_and_a_dense_index_id() {
    let line = fixture_line(3).expect("valid");
    let mut seen = std::collections::BTreeSet::new();
    fn walk(node: &serde_json::Value, seen: &mut std::collections::BTreeSet<u64>) {
        if let Some(id) = node["id"].as_u64() {
            seen.insert(id);
            assert!(node["weight"].as_f64().expect("finite").is_finite());
        }
        for child in node["children"].as_array().expect("children") {
            walk(child, seen);
        }
    }
    walk(&line["tree"], &mut seen);
    let n = graph_core::gate_node_count(3) as u64;
    assert_eq!(seen, (0..n).collect());
}

/// Two seeds' fixtures must not be byte-identical: the seed reaches the tree and both
/// layouts, not just the node count.
#[test]
fn two_seeds_six_hundred_apart_share_a_node_count_but_draw_different_fixtures() {
    let a = fixture_line(11).expect("valid");
    let b = fixture_line(611).expect("valid");
    assert_eq!(
        graph_core::gate_node_count(11),
        graph_core::gate_node_count(611)
    );
    assert_ne!(a, b);
}
