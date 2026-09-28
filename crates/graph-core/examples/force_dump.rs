//! Dumps a topology and one layout's positions as plain text, for the Node-side
//! d3-force comparison script (`scratch/stress.mjs`) to read without re-implementing
//! our synthetic generator or JSON fixture reader in JS. Not part of the graded library
//! surface — a measurement tool for `docs/measurements/phase06-{force,stress}.md`
//! (`P56_SPEC.md`'s branch p6f: "our positions dumped by a test/example").
//!
//! Usage:
//! - `cargo run --release --example force_dump -- fixture <name> <layout>`
//! - `cargo run --release --example force_dump -- seed <seed> <count> <layout>`
//! - `cargo run --release --example force_dump -- path <json-file> <layout>` (same
//!   schema as `fixtures/force/*.json`, any path — the 50-seed synthetic sweep's
//!   generated graphs live under `scratch/synthetic/`, outside the graded fixture set)
//!
//! `<layout>` is `barnes_hut` or `fa2`. Stdout:
//! ```text
//! NODES <n>
//! EDGES <m>
//! <source> <target>       (m lines, dense indices)
//! POSITIONS
//! <x> <y>                 (n lines, node order)
//! ```
//! Stderr gets one timing line (`elapsed_ms=...`), read by the force-size sweep.

use graph_contract::canonical_json::{Value, parse};
use graph_contract::geometry::NodeGeometry;
use graph_core::layout::force::{BarnesHut, ForceParams};
use graph_core::layout::forceatlas2::{Fa2Params, ForceAtlas2};
use graph_core::{
    EdgeKind, EdgeRecord, NodeKind, NodeRecord, REFERENCE_DEGREE, Stage, Topology, index_model,
    seeded_model,
};
use std::time::Instant;
use std::{env, fs};

fn main() {
    let args: Vec<String> = env::args().collect();
    let (topology, layout) = parse_args(&args);
    let start = Instant::now();
    let (x, y) = run_layout(&topology, &layout);
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    eprintln!(
        "layout={layout} nodes={} edges={} elapsed_ms={elapsed_ms}",
        topology.node_count(),
        topology.edge_count()
    );
    print_dump(&topology, &x, &y);
}

fn parse_args(args: &[String]) -> (Topology, String) {
    match args.get(1).map(String::as_str) {
        Some("fixture") => (
            load_json(&format!("fixtures/force/{}.json", args[2])),
            args[3].clone(),
        ),
        Some("path") => (load_json(&args[2]), args[3].clone()),
        Some("seed") => {
            let seed: u32 = args[2].parse().expect("seed");
            let count: u32 = args[3].parse().expect("count");
            let (nodes, edges) = seeded_model(seed, count, REFERENCE_DEGREE);
            (index_model(&nodes, &edges).expect("fits"), args[4].clone())
        }
        _ => {
            eprintln!(
                "usage: force_dump fixture <name> <layout> | force_dump path <file> <layout> | force_dump seed <seed> <count> <layout>"
            );
            std::process::exit(2);
        }
    }
}

fn run_layout(t: &Topology, layout: &str) -> (Vec<f32>, Vec<f32>) {
    let geometry = match layout {
        "barnes_hut" => BarnesHut::run(t, &ForceParams::default()),
        "fa2" => ForceAtlas2::run(t, &Fa2Params::default()),
        other => panic!("unknown layout {other}"),
    }
    .unwrap_or_else(|e| panic!("{layout} on {} nodes: {e}", t.node_count()));
    match geometry.nodes {
        NodeGeometry::Point { x, y } => (x, y),
        _ => panic!("force layouts are point geometry"),
    }
}

fn print_dump(t: &Topology, x: &[f32], y: &[f32]) {
    let edges = t.edges();
    println!("NODES {}", t.node_count());
    println!("EDGES {}", t.edge_count());
    for e in 0..t.edge_count() as usize {
        println!("{} {}", edges.source[e], edges.target[e]);
    }
    println!("POSITIONS");
    for i in 0..x.len() {
        println!("{} {}", x[i], y[i]);
    }
}

fn load_json(path: &str) -> Topology {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let root = parse(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
    let nodes: Vec<NodeRecord> = array(&root, "nodes").iter().map(fixture_node).collect();
    let edges: Vec<EdgeRecord> = array(&root, "edges").iter().map(fixture_edge).collect();
    index_model(&nodes, &edges).unwrap_or_else(|e| panic!("{path}: {e:?}"))
}

fn fixture_node(value: &Value) -> NodeRecord {
    NodeRecord {
        id: text(value, "id"),
        kind: NodeKind::Record,
        database_id: None,
        source: "fixture".into(),
        label: String::new(),
        group: None,
        weight: 0.5,
        version: 0.0,
        has_note: false,
        icon: None,
    }
}

fn fixture_edge(value: &Value) -> EdgeRecord {
    let strength = match member(value, "strength") {
        Value::Number(n) => n.parse().expect("a strength"),
        _ => 1.0,
    };
    EdgeRecord {
        id: text(value, "id"),
        source: text(value, "source"),
        target: text(value, "target"),
        kind: EdgeKind::Relation,
        label: String::new(),
        strength,
        directed: false,
        record_id: None,
        child_first: false,
    }
}

fn member<'a>(value: &'a Value, key: &str) -> &'a Value {
    let Value::Object(members) = value else {
        panic!("not an object: {value:?}");
    };
    members
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
        .unwrap_or_else(|| panic!("no {key}"))
}

fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    match member(value, key) {
        Value::Array(items) => items,
        other => panic!("{key}: {other:?}"),
    }
}

fn text(value: &Value, key: &str) -> String {
    match member(value, key) {
        Value::String(text) => text.clone(),
        other => panic!("{key}: {other:?}"),
    }
}
