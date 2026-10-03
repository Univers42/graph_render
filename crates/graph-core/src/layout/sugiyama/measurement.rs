//! The crossing-count dump `docs/decisions/sugiyama-heuristics.md`'s dagre differential is
//! read from: the 7 DAG fixtures plus a synthetic sweep, our own counts whole. Split out of
//! `mod.rs` for the 300-line house limit; test-only, `#[ignore]`d, nothing reads the file
//! but `harness/oracle-layouts.mjs --dag`.

use super::*;
use crate::index::index_model;
use crate::records::build::{edge, node};
use crate::synthetic::Mulberry32;
use graph_contract::canonical_json::{Value, parse};
use std::fmt::Write as _;

const FIXTURES: [(&str, &str); 7] = [
    (
        "chain",
        include_str!("../../../../../fixtures/dag/chain.json"),
    ),
    (
        "diamond",
        include_str!("../../../../../fixtures/dag/diamond.json"),
    ),
    (
        "cyclic",
        include_str!("../../../../../fixtures/dag/cyclic.json"),
    ),
    (
        "multi-span",
        include_str!("../../../../../fixtures/dag/multi-span.json"),
    ),
    (
        "wide-layer",
        include_str!("../../../../../fixtures/dag/wide-layer.json"),
    ),
    (
        "disconnected",
        include_str!("../../../../../fixtures/dag/disconnected.json"),
    ),
    (
        "parallel-arcs",
        include_str!("../../../../../fixtures/dag/parallel-arcs.json"),
    ),
];

fn member<'a>(v: &'a Value, k: &str) -> &'a Value {
    let Value::Object(m) = v else {
        panic!("not an object")
    };
    &m.iter().find(|(key, _)| key == k).expect("member").1
}
fn text(v: &Value, k: &str) -> String {
    match member(v, k) {
        Value::String(s) => s.clone(),
        other => panic!("{k}: {other:?}"),
    }
}
fn array<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    match member(v, k) {
        Value::Array(a) => a,
        other => panic!("{k}: {other:?}"),
    }
}

type Graph = (Vec<String>, Vec<(String, String, String)>);

/// One fixture's `(node ids, (edge id, source, target))`.
fn load(fixture: &str) -> Graph {
    let root = parse(fixture).expect("valid fixture json");
    let nodes = array(&root, "nodes")
        .iter()
        .map(|n| text(n, "id"))
        .collect();
    let edges = array(&root, "edges")
        .iter()
        .map(|e| (text(e, "id"), text(e, "source"), text(e, "target")))
        .collect();
    (nodes, edges)
}

/// A random DAG over `6..26` nodes, edges only `i < j` so it is acyclic and
/// parallel-edge-free by construction, at density 0.2, from `seed`.
fn synthetic_dag(seed: u32) -> Graph {
    let mut rnd = Mulberry32::new(seed);
    let n = 6 + rnd.pick(20) as u32;
    let nodes: Vec<String> = (0..n).map(|i| i.to_string()).collect();
    let mut edges = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            if rnd.next_f64() < 0.2 {
                edges.push((format!("{i}-{j}"), i.to_string(), j.to_string()));
            }
        }
    }
    (nodes, edges)
}

fn topology_of(nodes: &[String], edges: &[(String, String, String)]) -> Topology {
    let n: Vec<_> = nodes.iter().map(|id| node(id, "")).collect();
    let e: Vec<_> = edges.iter().map(|(id, s, t)| edge(id, s, t)).collect();
    index_model(&n, &e).expect("fixture and synthetic graphs always fit")
}

/// Appends `{"name","nodes","edges":[[s,t],...],"our_crossings"}` for one graph.
fn dump_one(out: &mut String, name: &str, nodes: &[String], edges: &[(String, String, String)]) {
    let crossings = crossings_for(&topology_of(nodes, edges));
    write!(out, r#"{{"name":{name:?},"nodes":["#).expect("string write");
    for (i, id) in nodes.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write!(out, "{id:?}").expect("string write");
    }
    out.push_str(r#"],"edges":["#);
    for (i, (_, s, t)) in edges.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write!(out, "[{s:?},{t:?}]").expect("string write");
    }
    write!(out, r#"],"our_crossings":{crossings}}}"#).expect("string write");
}

#[test]
#[ignore = "writes target/dag-crossings.json for the Node oracle differential"]
fn dump_crossing_measurements() {
    let mut out = String::from("[");
    for (i, (name, fixture)) in FIXTURES.into_iter().enumerate() {
        let (nodes, edges) = load(fixture);
        if i > 0 {
            out.push(',');
        }
        dump_one(&mut out, name, &nodes, &edges);
    }
    for seed in 0..230u32 {
        let (nodes, edges) = synthetic_dag(seed);
        if edges.is_empty() {
            continue;
        }
        out.push(',');
        dump_one(&mut out, &format!("synthetic-{seed}"), &nodes, &edges);
    }
    out.push(']');
    let target = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
    std::fs::create_dir_all(&target).expect("mkdir target");
    std::fs::write(target.join("dag-crossings.json"), out).expect("write dump");
}
