use super::*;
use crate::runner::sha256_hex;
use std::collections::BTreeSet;

fn cases(seeds: std::ops::Range<u32>) -> Vec<Case> {
    seeds.flat_map(for_seed).collect()
}

fn args<'a>(all: &'a [Case], function: &str) -> Vec<&'a Value> {
    all.iter()
        .filter(|c| c.function == function)
        .map(|c| &c.args)
        .collect()
}

fn share(hits: usize, of: usize) -> f64 {
    hits as f64 / of as f64
}

/// The generator is recorded in the manifest by name; this pins what it writes, so a
/// change to it is a deliberate edit of this digest, never a silent drift.
#[test]
fn the_generator_writes_exactly_the_pinned_cases() {
    let seeds = (0..21).chain([100, 200, 300, 399]);
    let text: String = seeds
        .flat_map(for_seed)
        .map(|c| format!("{} {}\n", c.function, c.args))
        .collect();
    assert_eq!(sha256_hex(text.as_bytes()), PINNED);
}

const PINNED: &str = "6173f8b4c89d6d78f55dd4140dde35e63c85e6829960a731b70f22be1a1d2386";

#[test]
fn a_third_of_id_coordinates_are_adversarial_and_the_rest_vary() {
    let all = cases(0..300);
    let coords: Vec<&str> = args(&all, "makeRecordNodeId")
        .iter()
        .flat_map(|a| ["source", "databaseId", "recordId"].map(|k| a[k].as_str().unwrap_or("")))
        .collect();
    let pooled = coords.iter().filter(|c| ID_POOL.contains(c)).count();
    assert!(
        (0.25..0.5).contains(&share(pooled, coords.len())),
        "{pooled}"
    );
    assert!(coords.iter().collect::<BTreeSet<_>>().len() > 300);
    let parsed = args(&all, "parseNodeId");
    let three = parsed
        .iter()
        .filter(|a| {
            a["nodeId"]
                .as_str()
                .is_some_and(|s| s.matches(':').count() >= 2)
        })
        .count();
    let pooled = parsed
        .iter()
        .filter(|a| ID_POOL.contains(&a["nodeId"].as_str().unwrap_or("?")))
        .count();
    assert!(
        share(three, parsed.len()) > 0.3 && share(pooled, parsed.len()) > 0.25,
        "{three} {pooled}"
    );
}

/// Equality cases: a quarter are clones, and an edit may redraw a field to the same
/// value, so between a fifth and three fifths compare equal.
#[test]
fn neighborhoods_start_mostly_at_a_real_node_and_sometimes_nowhere() {
    let all = cases(0..300);
    let starts = args(&all, "neighborhood");
    let missing = starts.iter().filter(|a| a["id"] == "missing").count();
    assert!(
        (0.05..0.3).contains(&share(missing, starts.len())),
        "{missing}"
    );
    let equal = args(&all, "nodesEqual")
        .iter()
        .filter(|a| a["a"] == a["b"])
        .count();
    assert!((0.2..0.6).contains(&share(equal, 300)), "{equal}");
    let equal = args(&all, "edgesEqual")
        .iter()
        .filter(|a| a["a"] == a["b"])
        .count();
    assert!((0.2..0.6).contains(&share(equal, 300)), "{equal}");
}

#[test]
fn synthetic_sizes_cover_the_special_inputs_and_three_digested_giants() {
    let n = |seed| synthetic_args(seed)["n"].as_str().map(str::to_owned);
    assert_eq!(n(0), Some(hex(f64::NAN)));
    assert_eq!(n(4), Some(hex(2.9)));
    assert_eq!(n(7), Some(hex(-0.0)));
    assert_eq!(n(8), Some(hex(10.0)));
    assert_eq!(n(61), Some(hex(3.0)));
    for (seed, size) in [(100, 1_000.0), (200, 5_000.5), (300, 1e9)] {
        assert_eq!(
            synthetic_args(seed),
            json!({ "n": hex(size), "digest": true })
        );
    }
    assert_eq!(synthetic_args(99)["digest"], false);
}

#[test]
fn group_cases_have_one_plus_seed_mod_400_sources_five_repeats_and_a_dropped_id() {
    let shape = |seed| {
        let nodes = groups_case(seed).args["nodes"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let sources: BTreeSet<String> = nodes.iter().map(|n| n["source"].to_string()).collect();
        (nodes.len(), sources.len())
    };
    assert_eq!(shape(0), (7, 2));
    assert_eq!(shape(300), (307, 302));
    assert_eq!(shape(410), (17, 12));
    let last = groups_case(7).args["nodes"][13].clone();
    assert_eq!(last, json!({ "id": "n0", "source": "dropped" }));
    assert_eq!(groups_case(7).args["nodes"][14], Value::Null);
}
