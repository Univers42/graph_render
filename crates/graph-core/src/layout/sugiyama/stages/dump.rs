//! The ignored test behind [`super`]: writes both graphs' stage snapshots to
//! `target/sugiyama-stages.json` for the reference half to be diffed against.
//!
//! **Run alone and read by a human.** `--ignored` with a name filter, nothing else; the file
//! is a measurement, not an artefact the test suite checks.

use super::{Stages, stages};
use crate::index::Topology;
use crate::index::index_model;
use crate::records::build::{edge, node};
use graph_contract::canonical_json::{Value, parse};
use std::fmt::Write as _;

const DIAMOND: &str = include_str!("../../../../../../fixtures/dag/diamond.json");
const LESMIS: &str = include_str!("../../../../../../fixtures/scigraphs/lesmis.json");
const TREE: &str = include_str!("../../../../../../fixtures/hierarchy/tree-balanced.json");

/// The three file-backed graphs the conformance row's numbers are read off: the diamond, the
/// smallest graph with a real layering choice; the gallery graph, where every stage has room
/// to go wrong; and the repo's balanced tree, the one fixture whose edges are not spelled in
/// topological order.
const FIXTURES: [(&str, &str); 3] = [
    ("dag-diamond", DIAMOND),
    ("lesmis", LESMIS),
    ("tree-balanced", TREE),
];

/// The gate models, at the seeds the conformance fixture set emits (`gate_model.rs`, seeds
/// 0..19). They are the half of the set where the rows disagree most, and the first two are
/// the smallest graphs that disagree at all.
///
/// **Built from `graph_core::seeded_model` rather than read from `conformance.jsonl`.** The
/// fixture reader renames the gate model's nodes `n0..n{n}` in their own list order, so the
/// dense index of both is the model's list position and the two topologies are the same graph
/// — the node-order contract `conformance/fixtures.rs` states. Regenerating them here keeps
/// this module out of `graph-cli`'s fixture tree, which is where the conformance arm keeps
/// its own copies.
const GATE_SEEDS: [u32; 20] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
];

/// One fixture as the reference arm builds it: **dense index `i` is the node whose id sorts
/// `i`-th in byte order**, which is the rule `conformance/fixtures.rs` states and the
/// conformance reader checks (`sc_fixture.Fixture._check_order`). It is not the same as
/// list order: `tree-balanced.json` lists `r` first, and the conformance fixture re-lists it
/// last for exactly that reason, so a loader that used list order would compare two
/// different graphs under one name.
///
/// The gallery file's ids are integers and are kept: its reader refuses any id that is not
/// its own position (`conformance/fixtures/named.rs:28-46`), so byte order and position
/// agree there and nothing needs remapping.
fn load(text: &str) -> Topology {
    let root = parse(text).expect("fixture json");
    let named: Vec<String> = array(&root, "nodes")
        .iter()
        .map(|n| match field(n, "id") {
            Value::String(name) => name.clone(),
            Value::Number(at) => at.clone(),
            other => panic!("node id: {other:?}"),
        })
        .collect();
    let numbered = named[0].parse::<usize>().is_ok();
    let mut ids = named.clone();
    if !numbered {
        ids.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    }
    // A `name -> dense index` table built once, so the endpoint lookup below is a binary
    // search rather than a scan of `ids` per endpoint (D4: no `HashMap`, ascending key).
    let mut seat: Vec<(&str, usize)> = ids
        .iter()
        .enumerate()
        .map(|(index, id)| (id.as_str(), index))
        .collect();
    seat.sort_unstable_by_key(|(name, _)| name.as_bytes());
    let at = |name: &str| {
        seat.binary_search_by_key(&name.as_bytes(), |(key, _)| key.as_bytes())
            .map(|hit| seat[hit].1)
            .expect("an edge names a node the file lists")
    };
    let nodes: Vec<_> = ids.iter().map(|id| node(id, "")).collect();
    let edges: Vec<_> = array(&root, "edges")
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let (source, target) = ends(entry, &named);
            edge(&format!("e{index}"), &ids[at(&source)], &ids[at(&target)])
        })
        .collect();
    index_model(&nodes, &edges).expect("fixture graphs fit")
}

/// One edge's two endpoint **ids**, from either spelling: an index pair into the file's own
/// node list (the gallery file) or named endpoints (the repo fixtures). Ids, not positions,
/// because the caller renames positions into the dense order.
fn ends(entry: &Value, listed: &[String]) -> (String, String) {
    let at = |position: usize| listed[position].clone();
    let named = |key: &str| text_of(entry, key);
    match entry {
        Value::Array(pair) => {
            let ends: Vec<usize> = pair
                .iter()
                .map(|v| match v {
                    Value::Number(text) => text.parse::<usize>().expect("index"),
                    other => panic!("edge end: {other:?}"),
                })
                .collect();
            (at(ends[0]), at(ends[1]))
        }
        _ => (named("source"), named("target")),
    }
}

fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    let Value::Object(members) = value else {
        panic!("not an object")
    };
    members
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
    join(arcs.iter().map(|(tail, head)| format!("[{tail},{head}]")))
}

fn join(items: impl Iterator<Item = String>) -> String {
    items.collect::<Vec<_>>().join(",")
}

/// One gate model's topology, at the same seed and node count the conformance fixture set
/// emits (`conformance/fixtures/gate_model.rs`).
fn gate(seed: u32) -> Topology {
    let count = crate::gate_node_count(seed);
    let (nodes, edges) = crate::seeded_model(seed, count, crate::REFERENCE_DEGREE);
    index_model(&nodes, &edges).expect("the gate model fits")
}

#[test]
#[ignore = "writes target/sugiyama-stages.json for the SciGraphs stage diff"]
fn dump_stage_measurements() {
    // An array of snapshots, one per fixture, so each `dump_one` is a whole JSON value.
    let graphs: Vec<(String, Topology)> = FIXTURES
        .into_iter()
        .map(|(name, text)| (name.to_string(), load(text)))
        .chain(
            GATE_SEEDS
                .into_iter()
                .map(|s| (format!("gate-{s:02}"), gate(s))),
        )
        .collect();
    let mut out = String::from("[");
    for (position, (name, topology)) in graphs.iter().enumerate() {
        if position > 0 {
            out.push(',');
        }
        dump_one(&mut out, name, &stages(topology));
    }
    out.push(']');
    let target = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
    std::fs::create_dir_all(&target).expect("mkdir target");
    std::fs::write(target.join("sugiyama-stages.json"), out).expect("write dump");
}
