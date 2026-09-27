//! The case list: per seed, one or more cases for each of the 17 oracle functions (plus
//! `hashString` for H4 and the H9 group arm), and the hand-written adversarial id pairs.

use super::generate::{self, Graph, Rng};
use super::pools::ID_POOL;
use super::wire::hex;
use graph_core::EdgeKind;
use serde_json::{Value, json};

/// One input line of `cases.jsonl`.
pub struct Case {
    pub function: &'static str,
    pub args: Value,
}

fn case(function: &'static str, args: Value) -> Case {
    Case { function, args }
}

/// Graphs are defined once per seed and referenced by name, so the file carries each
/// graph once however many functions read it.
fn define(name: String, graph: &Graph) -> Case {
    case(
        "graph",
        json!({ "name": name, "nodes": graph.0, "edges": graph.1 }),
    )
}

/// Every case for seed `seed`.
pub fn for_seed(seed: u32) -> Vec<Case> {
    let mut rng = Rng::new(seed);
    let previous = generate::graph(&mut rng, 24);
    let next = generate::mutate(&mut rng, &previous);
    let (g, h) = (format!("g{seed}"), format!("h{seed}"));
    let mut cases = vec![define(g.clone(), &previous), define(h.clone(), &next)];
    cases.extend(id_cases(&mut rng));
    cases.extend(model_cases(&mut rng, &g, &h, &previous));
    cases.push(case("buildSyntheticModel", synthetic_args(seed)));
    if seed.is_multiple_of(10) {
        cases.push(groups_case(seed));
    }
    if seed == 0 {
        cases.push(case("emptyModel", json!({})));
    }
    cases
}

/// An id coordinate: a third from the adversarial pool, the rest random strings.
fn coord(rng: &mut Rng) -> String {
    if rng.below(3) == 0 {
        rng.pick(&ID_POOL).to_owned()
    } else {
        generate::any_string(rng)
    }
}

fn id_cases(rng: &mut Rng) -> Vec<Case> {
    let (s, d, r) = (coord(rng), coord(rng), coord(rng));
    let parse_input = match rng.below(3) {
        0 => format!("{s}:{d}:{r}"),
        1 => rng.pick(&ID_POOL).to_owned(),
        _ => generate::any_string(rng),
    };
    let (a, b) = (coord(rng), coord(rng));
    let kind = EdgeKind::ALL[rng.below(5)].as_str();
    vec![
        case(
            "makeRecordNodeId",
            json!({ "source": s, "databaseId": d, "recordId": r }),
        ),
        case("makeNoteNodeId", json!({ "noteId": coord(rng) })),
        case("makeTagNodeId", json!({ "tagValue": coord(rng) })),
        case(
            "makeEdgeId",
            edge_id_args(&a, &b, kind, &coord(rng), rng.below(2) == 0),
        ),
        case("parseNodeId", json!({ "nodeId": parse_input })),
        case(
            "edgeKindFromType",
            json!({ "type": generate::wire_type(rng) }),
        ),
        case("hashString", json!({ "value": coord(rng) })),
    ]
}

fn edge_id_args(source: &str, target: &str, kind: &str, label: &str, directed: bool) -> Value {
    json!({ "source": source, "target": target, "kind": kind, "label": label, "directed": directed })
}

fn model_cases(rng: &mut Rng, g: &str, h: &str, graph: &Graph) -> Vec<Case> {
    let ids: Vec<String> = graph.0.iter().map(|n| n.id.clone()).collect();
    let start = if ids.is_empty() || rng.below(10) == 0 {
        "missing".to_owned()
    } else {
        ids[rng.below(ids.len())].clone()
    };
    let depth = [0, 1, 1, 2, 3, 1000][rng.below(6)];
    let lengths: Vec<usize> = (0..6).map(|_| [0, 0, 1, 3][rng.below(4)]).collect();
    let mut cases = vec![
        case("applyDegreeWeights", json!({ "graph": g })),
        case("indexModel", json!({ "graph": g })),
        case("diffGraph", json!({ "previous": g, "next": h })),
        case("isEmptyPatch", json!({ "lengths": lengths })),
        case("deriveLegend", json!({ "graph": g })),
        case(
            "neighborhood",
            json!({ "graph": g, "id": start, "depth": depth }),
        ),
        case(
            "neighborhoodEdges",
            json!({ "graph": g, "id": start, "depth": depth }),
        ),
    ];
    let pool = graph.0.len() + 2;
    let a = generate::node(rng, pool);
    let b = if rng.below(4) == 0 {
        a.clone()
    } else {
        generate::edit_node(rng, &a)
    };
    cases.push(case("nodesEqual", json!({ "a": a, "b": b })));
    let x = generate::edge(rng, pool, &ids);
    let y = if rng.below(4) == 0 {
        x.clone()
    } else {
        generate::edit_edge(rng, &x, &ids)
    };
    cases.push(case("edgesEqual", json!({ "a": x, "b": y })));
    cases
}

/// `n` for `buildSyntheticModel`: small models mostly, the non-finite and fractional
/// inputs its own comments call out, and three large ones compared by digest.
fn synthetic_args(seed: u32) -> Value {
    let special = [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -4.0,
        2.9,
        0.0,
        1.0,
        -0.0,
    ];
    let (n, digest) = match seed {
        0..=7 => (special[seed as usize], false),
        100 => (1_000.0, true),
        200 => (5_000.5, true),
        300 => (1e9, true),
        _ => (f64::from(2 + seed % 60), false),
    };
    json!({ "n": hex(n), "digest": digest })
}

/// H9: `1 + seed % 400` distinct sources over as many nodes plus a few repeats, so
/// every seed from 256 up crosses the oracle's `Uint8Array` group width.
fn groups_case(seed: u32) -> Case {
    let sources = 1 + seed % 400;
    let nodes: Vec<Value> = (0..sources + 5)
        .map(|i| json!({ "id": format!("n{i}"), "source": format!("s{}", i % sources) }))
        .collect();
    case("layoutGroups", json!({ "nodes": nodes }))
}

/// `fixtures/adversarial-ids.json` pairs, each as undirected and directed, both ways.
pub fn adversarial(pairs: &[(String, String)]) -> Vec<Case> {
    let mut cases = Vec::new();
    for (a, b) in pairs {
        for directed in [false, true] {
            for (s, t) in [(a, b), (b, a)] {
                let args = edge_id_args(s, t, "relation", "", directed);
                cases.push(case("makeEdgeId", args));
            }
        }
    }
    cases
}

#[cfg(test)]
mod tests {
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

    const PINNED: &str = "9300bdf6bcddd0639d7d104d7c51a84587e1cd48714700ab77e4365a9d6020a8";

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
    fn group_cases_have_one_plus_seed_mod_400_sources_over_five_more_nodes() {
        let shape = |seed| {
            let nodes = groups_case(seed).args["nodes"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let sources: BTreeSet<String> = nodes.iter().map(|n| n["source"].to_string()).collect();
            (nodes.len(), sources.len())
        };
        assert_eq!(shape(0), (6, 1));
        assert_eq!(shape(300), (306, 301));
        assert_eq!(shape(410), (16, 11));
    }
}
