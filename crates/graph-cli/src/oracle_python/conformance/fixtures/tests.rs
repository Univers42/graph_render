//! The fixture set's own tests: the properties both arms depend on and neither arm checks,
//! because each is a property of the fixture rather than of a layout.

use super::{Fixture, GATE_CAP, GATE_SEEDS, all, bipartite, dag, gate, lesmis, line, tree};
use graph_core::layout::basic_3d::sphere;
use graph_core::run_with;
use serde_json::Value;

fn set() -> Vec<Fixture> {
    all().expect("the fixture set")
}

/// Every fixture's node ids sort byte-wise into their own list order — the mapping the whole
/// contract rests on. A fixture that failed here would compare node `i` against a different
/// node on one arm, and every metric below would still be computed.
#[test]
fn every_fixture_is_in_byte_order() {
    for fixture in set() {
        let mut sorted: Vec<&str> = fixture.nodes.iter().map(|n| n.id.as_str()).collect();
        sorted.sort();
        let listed: Vec<&str> = fixture.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(sorted, listed, "{}: not in byte order", fixture.name);
    }
}

/// Every edge end is a node of its own fixture, and every node id appears once: without both
/// the dense index in `conformance.jsonl` means nothing.
#[test]
fn every_edge_end_is_a_node_and_every_node_is_once() {
    for fixture in set() {
        let ids: Vec<&str> = fixture.nodes.iter().map(|n| n.id.as_str()).collect();
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(
            unique.len(),
            ids.len(),
            "{}: a node id is twice",
            fixture.name
        );
        for e in &fixture.edges {
            assert!(ids.contains(&e.source.as_str()), "{}: source", fixture.name);
            assert!(ids.contains(&e.target.as_str()), "{}: target", fixture.name);
        }
    }
}

/// The topology keeps the list order, so coordinate `i` is the node the fixture listed at
/// `i`. Read off the snapshot, which is the only place the order is a table.
#[test]
fn the_topology_keeps_the_fixture_order() {
    for fixture in set() {
        let layout = graph_core::registry::find("layout.grid").expect("grid is registered");
        let run = run_with(&fixture.nodes, &fixture.edges, layout.id, layout.run)
            .unwrap_or_else(|e| panic!("{}: {e}", fixture.name));
        for (position, record) in fixture.nodes.iter().enumerate() {
            let at = run.snapshot.parts().node_ids.get(position as u32);
            assert_eq!(
                at,
                Some(record.id.as_str()),
                "{}: node {position}",
                fixture.name
            );
        }
    }
}

/// The gate seeds are under the stated cap, so the cap is a bound and not a truncation, and
/// a reader can see that from the tree rather than from this comment.
#[test]
fn the_gate_seeds_are_under_the_cap() {
    for seed in 0..GATE_SEEDS {
        let fixture = gate(seed).expect("the gate model");
        assert!(fixture.nodes.len() as u32 <= GATE_CAP, "seed {seed}");
    }
}

/// Each of the four named fixtures is the shape it claims, at the size it claims — a matrix
/// whose "rooted tree" row was over a triangle would look exactly as convincing.
#[test]
fn the_named_fixtures_are_the_four_shapes() {
    let k = bipartite();
    assert_eq!((k.nodes.len(), k.edges.len()), (14, 48));
    assert_eq!(
        (
            dag().expect("dag").nodes.len(),
            dag().expect("dag").edges.len()
        ),
        (4, 4)
    );
    let tree = tree().expect("tree");
    assert_eq!((tree.nodes.len(), tree.edges.len()), (15, 15));
    // The root's own edges survive the re-order, which is what sorting the node list had to
    // preserve: the fixture's node order is byte order and the tree's shape is the edges.
    // The root is a `source` here — `tree-balanced.json` spells half its edges child-first.
    assert_eq!(
        tree.edges.iter().filter(|e| e.source == "r").count(),
        2,
        "the tree lost its root's own edges"
    );
    assert!(
        tree.edges.iter().all(|e| e.source != e.target),
        "a self-loop"
    );
    let lesmis = lesmis().expect("lesmis");
    assert_eq!((lesmis.nodes.len(), lesmis.edges.len()), (77, 254));
}

/// The set admits both a z column and its absence, so a `not run: no z column` cell in the
/// matrix is about the layout and never about the fixture.
#[test]
fn the_set_admits_both_a_z_column_and_its_absence() {
    let fixture = lesmis().expect("lesmis");
    let planar = registry_run(&fixture, "layout.grid").expect("grid");
    assert!(planar.z.is_none(), "a 2D layout must have no z column");
    let solid = registry_run(&fixture, sphere::ID).expect("sphere");
    assert_eq!(
        solid.z.as_ref().map(Vec::len),
        Some(77),
        "sphere's z column"
    );
}

/// The fixture line states the mapping, and it states it as the identity it is: SciGraphs
/// index `i` is the motor id that sorts `i`-th, which is `nodes[i]`.
#[test]
fn the_line_states_the_mapping_it_relies_on() {
    let fixture = bipartite();
    let line = line(&fixture);
    let nodes: Vec<&str> = line["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(Value::as_str)
        .collect::<Option<Vec<_>>>()
        .expect("node names");
    let mapping: Vec<(u64, String)> = line["mapping"]
        .as_array()
        .expect("mapping")
        .iter()
        .map(|pair| {
            let pair = pair.as_array().expect("a pair");
            (
                pair[0].as_u64().expect("an index"),
                pair[1].as_str().expect("a name").to_string(),
            )
        })
        .collect();
    assert_eq!(line["n"].as_u64(), Some(14));
    assert_eq!(mapping.len(), nodes.len());
    for (index, name) in mapping.iter().enumerate() {
        assert_eq!(index as u64, name.0, "mapping is not in index order");
        assert_eq!(
            nodes[name.0 as usize],
            name.1.as_str(),
            "mapping disagrees with the node order"
        );
    }
}

/// Every edge endpoint in the line is a real index. A `u32::MAX` seat would come out of
/// `line`'s fallback and would index nothing on the far side.
#[test]
fn every_line_endpoint_indexes_a_node() {
    for fixture in set() {
        let line = line(&fixture);
        let n = line["n"].as_u64().expect("n") as u32;
        for key in ["source", "target"] {
            let ends: Vec<u64> = line[key]
                .as_array()
                .expect("ends")
                .iter()
                .map(|v| v.as_u64().expect("an index"))
                .collect();
            assert_eq!(ends.len(), fixture.edges.len(), "{}: {key}", fixture.name);
            assert!(
                ends.iter().all(|end| *end < n as u64),
                "{}: {key}",
                fixture.name
            );
        }
    }
}

fn registry_run(
    fixture: &Fixture,
    id: &'static str,
) -> Result<graph_contract::binary::SnapshotParts, String> {
    let layout = graph_core::registry::find(id).ok_or_else(|| format!("{id}: gone"))?;
    run_with(&fixture.nodes, &fixture.edges, layout.id, layout.run)
        .map(|run| run.snapshot.into_parts())
        .map_err(|e| e.to_string())
}
