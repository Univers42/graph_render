//! The ignored test behind [`super`]: writes both graphs' stage snapshots to
//! `target/sugiyama-stages.json` for the reference half to be diffed against.
//!
//! **Run alone and read by a human.** `--ignored` with a name filter, nothing else; the file
//! is a measurement, not an artefact the test suite checks.

use super::{Stages, stages};
use crate::index::index_model;
use crate::index::Topology;
use crate::records::build::{edge, node};
use graph_contract::canonical_json::{Value, parse};
use std::fmt::Write as _;

const DIAMOND: &str = include_str!("../../../../../../fixtures/dag/diamond.json");
const LESMIS: &str = include_str!("../../../../../../fixtures/scigraphs/lesmis.json");

/// The two graphs the conformance row's numbers are read off: the diamond, the smallest
/// graph with a real layering choice, and the gallery graph, where every stage has room.
const FIXTURES: [(&str, &str); 2] = [("dag-diamond", DIAMOND), ("lesmis", LESMIS)];

/// One fixture as the reference arm builds it: dense index `i` is the node listed at
/// position `i`. The diamond names its nodes, the gallery file numbers them, and a number
/// that is not its own position would make "index `i`" mean two things — the conformance
/// fixture reader refuses that too (`conformance/fixtures/named.rs:28-46`), so this does.
fn load(text: &str) -> Topology {
    let root = parse(text).expect("fixture json");
    let listed = array(&root, "nodes");
    let ids: Vec<String> = listed
        .iter()
        .map(|n| match field(n, "id") {
            Value::String(name) => name.clone(),
            Value::Number(at) => at.clone(),
            other => panic!("node id: {other:?}"),
        })
        .collect();
    for (position, id) in ids.iter().enumerate() {
        if let Ok(at) = id.parse::<usize>() {
            assert_eq!(at, position, "node id {id} is not its own position");
        }
    }
    let nodes: Vec<_> = ids.iter().map(|id| node(id, "")).collect();
    let edges: Vec<_> = array(&root, "edges")
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let (source, target) = ends(entry, &ids);
            edge(&format!("e{index}"), &ids[source], &ids[target])
        })
        .collect();
    index_model(&nodes, &edges).expect("fixture graphs fit")
}

/// One edge's dense ends, from either spelling: an index pair (the gallery file) or named
/// endpoints (the repo fixtures).
fn ends(entry: &Value, ids: &[String]) -> (usize, usize) {
    let named = |key: &str| {
        let name = text_of(entry, key);
        ids.iter().position(|id| *id == name).expect("named endpoint")
    };
    match entry {
        Value::Array(pair) => {
            let at = pair
                .iter()
                .map(|v| match v {
                    Value::Number(text) => text.parse::<usize>().expect("index"),
                    other => panic!("edge end: {other:?}"),
                })
                .collect::<Vec<_>>();
            (at[0], at[1])
        }
        _ => (named("source"), named("target")),
    }
}

fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    let Value::Object(members) = value else {
        panic!("not an object")
    };
    &members
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, v)| v)
        .expect("member")
}

fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    match field(value, key) {
        Value::Array(items) => items,
        other => panic!("{key}: {other:?}"),
    }
}

fn text_of(value: &Value, key: &str) -> String {
    match field(value, key) {
        Value::String(text) => text.clone(),
        other => panic!("{key}: {other:?}"),
    }
}

/// `{"name":…,"nodes":n,"arcs":[[tail,head],…],"reversed":r,"layer_of":[…],`n"num_dummies":d,"order":[[…],…],"crossings":c,"x":[…]}` for
/// one fixture. Written by hand, as the crossing dump beside it is: no serialiser in
/// graph-core, and the shape is read by a diff rather than by code.
fn dump_one(out: &mut String, name: &str, snapshot: &Stages) {
    let n = snapshot.layer_of.len() - snapshot.num_dummies as usize;
    write!(
        out,
        r#"{{"name":{name:?},"nodes":{n},"arcs":[{}],"reversed":{},"layer_of":[{}],"num_dummies":{},"order":[{}],"crossings":{},"x":[{}]}}"#,
        pairs(&snapshot.arcs),
        snapshot.reversed,
        join(snapshot.layer_of.iter().map(|v| v.to_string())),
        snapshot.num_dummies,
        join(snapshot.order.iter().map(|row| format!("[{}]", join(row.iter().map(|v| v.to_string()))))),
        snapshot.crossings,
        join(snapshot.x.iter().map(|v| format!("{v:?}"))),
    )
    .expect("string write");
}

fn pairs(arcs: &[(u32, u32)]) -> String {
    join(
        arcs.iter()
            .map(|(tail, head)| format!("[{tail},{head}]")),
    )
}

fn join(items: impl Iterator<Item = String>) -> String {
    items.collect::<Vec<_>>().join(",")
}

#[test]
#[ignore = "writes target/sugiyama-stages.json for the SciGraphs stage diff"]
fn dump_stage_measurements() {
    // An array of snapshots, one per fixture, so each `dump_one` is a whole JSON value.
    let mut out = String::from("[");
    for (position, (name, text)) in FIXTURES.into_iter().enumerate() {
        if position > 0 {
            out.push(',');
        }
        dump_one(&mut out, name, &stages(&load(text)));
    }
    out.push(']');
    let target = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
    std::fs::create_dir_all(&target).expect("mkdir target");
    std::fs::write(target.join("sugiyama-stages.json"), out).expect("write dump");
}