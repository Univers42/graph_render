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
            edge_id_args((&a, &b), kind, &coord(rng), rng.below(2) == 0),
        ),
        case("parseNodeId", json!({ "nodeId": parse_input })),
        case(
            "edgeKindFromType",
            json!({ "type": generate::wire_type(rng) }),
        ),
        case("hashString", json!({ "value": coord(rng) })),
    ]
}

fn edge_id_args((source, target): (&str, &str), kind: &str, label: &str, directed: bool) -> Value {
    json!({ "source": source, "target": target, "kind": kind, "label": label, "directed": directed })
}

fn model_cases(rng: &mut Rng, g: &str, h: &str, graph: &Graph) -> Vec<Case> {
    let (start, depth) = walk(rng, graph);
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
    cases.extend(equality_cases(rng, graph));
    cases
}

/// A neighbourhood's start (a node of `graph`, or one that is not there) and depth.
fn walk(rng: &mut Rng, graph: &Graph) -> (String, u32) {
    let start = if graph.0.is_empty() || rng.below(10) == 0 {
        "missing".to_owned()
    } else {
        graph.0[rng.below(graph.0.len())].id.clone()
    };
    (start, [0, 1, 1, 2, 3, 1000][rng.below(6)])
}

/// `nodesEqual` and `edgesEqual` over a value and either its clone or an edit of it.
fn equality_cases(rng: &mut Rng, graph: &Graph) -> [Case; 2] {
    let ids: Vec<String> = graph.0.iter().map(|n| n.id.clone()).collect();
    let pool = graph.0.len() + 2;
    let a = generate::node(rng, pool);
    let b = if rng.below(4) == 0 {
        a.clone()
    } else {
        generate::edit_node(rng, &a)
    };
    let x = generate::edge(rng, pool, &ids);
    let y = if rng.below(4) == 0 {
        x.clone()
    } else {
        generate::edit_edge(rng, &x, &ids)
    };
    [
        case("nodesEqual", json!({ "a": a, "b": b })),
        case("edgesEqual", json!({ "a": x, "b": y })),
    ]
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
/// every seed from 256 up crosses the oracle's `Uint8Array` group width. A last node
/// repeats the first id under a new source: `indexModel` drops it, so only an arm that
/// groups the de-duplicated nodes (as `LayoutController.rebuild` does) sees no extra
/// group.
fn groups_case(seed: u32) -> Case {
    let sources = 1 + seed % 400;
    let mut nodes: Vec<Value> = (0..sources + 5)
        .map(|i| json!({ "id": format!("n{i}"), "source": format!("s{}", i % sources) }))
        .collect();
    nodes.push(json!({ "id": "n0", "source": "dropped" }));
    case("layoutGroups", json!({ "nodes": nodes }))
}

/// `fixtures/adversarial-ids.json` pairs, each as undirected and directed, both ways.
pub fn adversarial(pairs: &[(String, String)]) -> Vec<Case> {
    let mut cases = Vec::new();
    for (a, b) in pairs {
        for directed in [false, true] {
            for (s, t) in [(a, b), (b, a)] {
                let args = edge_id_args((s, t), "relation", "", directed);
                cases.push(case("makeEdgeId", args));
            }
        }
    }
    cases
}

#[cfg(test)]
mod tests;
