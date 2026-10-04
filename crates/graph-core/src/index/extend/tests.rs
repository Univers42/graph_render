use super::*;
use crate::columns::NodeKind;
use crate::index::extend::columns::BatchRefusal;
use crate::index::{empty_model, index_model};
use crate::records::build::{edge, node};
use crate::rng::Mt19937;
use crate::stage::topology_bytes;

mod columns;

type Batch = (Vec<NodeRecord>, Vec<EdgeRecord>);

fn pick(rng: &mut Mt19937, n: u32) -> u32 {
    rng.next_u32() % n
}

/// Node `i`. Its label may name a node not admitted yet, so the arena meets an id as an
/// ordinary string before it meets it as an id.
fn random_node(rng: &mut Mt19937, i: u32) -> NodeRecord {
    NodeRecord {
        id: format!("n{i}"),
        kind: NodeKind::ALL[pick(rng, 4) as usize],
        database_id: (pick(rng, 3) > 0).then(|| format!("db{}", pick(rng, 5))),
        source: format!("s{}", pick(rng, 7)),
        label: format!("n{}", pick(rng, 60)),
        group: (pick(rng, 2) == 0).then(|| format!("g{}", pick(rng, 4))),
        weight: rng.next_f64(),
        version: f64::from(pick(rng, 9)),
        has_note: pick(rng, 2) == 1,
        icon: (pick(rng, 4) == 0).then(|| "i".into()),
    }
}

/// Edge `i` between two of the first `nodes` nodes: loops and parallel edges included.
fn random_edge(rng: &mut Mt19937, i: u32, nodes: u32) -> EdgeRecord {
    let kind = EdgeKind::ALL[pick(rng, 5) as usize];
    EdgeRecord {
        id: format!("e{i}"),
        source: format!("n{}", pick(rng, nodes)),
        target: format!("n{}", pick(rng, nodes)),
        kind,
        label: format!("e{}", pick(rng, 40)),
        strength: rng.next_f64(),
        directed: pick(rng, 2) == 1,
        record_id: (pick(rng, 3) == 0).then(|| format!("n{}", pick(rng, 9))),
        child_first: kind == EdgeKind::Hierarchy && pick(rng, 2) == 1,
    }
}

/// A strict stream of 1 to 8 batches drawn from `seed`; a batch may be empty.
fn stream(seed: u32) -> Vec<Batch> {
    let mut rng = Mt19937::new(seed);
    let (mut nodes, mut edges) = (0, 0);
    let count = 1 + pick(&mut rng, 8);
    (0..count)
        .map(|_| {
            let batch_nodes = pick(&mut rng, 13);
            let ns = (nodes..nodes + batch_nodes).map(|i| random_node(&mut rng, i));
            let ns: Vec<_> = ns.collect();
            nodes += batch_nodes;
            let batch_edges = if nodes == 0 { 0 } else { pick(&mut rng, 21) };
            let es = (edges..edges + batch_edges).map(|i| random_edge(&mut rng, i, nodes));
            let es: Vec<_> = es.collect();
            edges += batch_edges;
            (ns, es)
        })
        .collect()
}

fn bytes(t: &Topology) -> Vec<u8> {
    topology_bytes(t).expect("finite")
}

/// `t` against `index_model` over every record of `batches`, in batch order.
fn assert_matches_rebuild(t: &Topology, batches: &[Batch], what: &str) {
    let nodes: Vec<_> = batches.iter().flat_map(|b| b.0.iter().cloned()).collect();
    let edges: Vec<_> = batches.iter().flat_map(|b| b.1.iter().cloned()).collect();
    let rebuilt = index_model(&nodes, &edges).expect("fits");
    assert_eq!(
        bytes(t),
        bytes(&rebuilt),
        "{what}: the topology stage's bytes"
    );
    assert_eq!(t.stats(), rebuilt.stats(), "{what}");
    for n in &nodes {
        assert_eq!(t.node_index(&n.id), rebuilt.node_index(&n.id), "{what}");
    }
    for e in &edges {
        assert_eq!(t.edge_index(&e.id), rebuilt.edge_index(&e.id), "{what}");
    }
}

#[test]
fn extend_matches_index_model() {
    for seed in 0..96 {
        let batches = stream(seed);
        let mut from_first = index_model(&batches[0].0, &batches[0].1).expect("fits");
        let mut from_empty = empty_model();
        from_empty
            .extend(&batches[0].0, &batches[0].1)
            .expect("strict");
        for k in 1..batches.len() {
            let (nodes, edges) = &batches[k];
            from_first.extend(nodes, edges).expect("strict");
            from_empty.extend(nodes, edges).expect("strict");
            let what = format!("seed {seed}, batch {k}");
            assert_matches_rebuild(&from_first, &batches[..=k], &what);
            assert_matches_rebuild(&from_empty, &batches[..=k], &what);
        }
    }
}

fn refusals() -> [(&'static str, Batch, ExtendError); 5] {
    use ExtendError::{EdgeId, Endpoint, NodeId};
    let c = || node("c", "db");
    [
        (
            "node id in the graph",
            (vec![c(), node("a", "")], vec![]),
            NodeId { index: 1 },
        ),
        (
            "node id twice in the batch",
            (vec![c(), c()], vec![]),
            NodeId { index: 1 },
        ),
        (
            "edge id in the graph",
            (vec![c()], vec![edge("e2", "a", "c"), edge("e1", "c", "b")]),
            EdgeId { index: 1 },
        ),
        (
            "edge id twice in the batch",
            (vec![], vec![edge("e2", "a", "b"), edge("e2", "b", "a")]),
            EdgeId { index: 1 },
        ),
        (
            "endpoint in neither",
            (
                vec![c()],
                vec![edge("e2", "c", "a"), edge("e3", "c", "ghost")],
            ),
            Endpoint { index: 1 },
        ),
    ]
}

#[test]
fn extend_refusal_leaves_topology_unchanged() {
    let first: Batch = (
        vec![node("a", "db"), node("b", "")],
        vec![edge("e1", "a", "b")],
    );
    let base = index_model(&first.0, &first.1).expect("fits");
    let next: Batch = (
        vec![node("c", "db")],
        vec![edge("e2", "c", "a"), edge("e3", "c", "c")],
    );
    for (name, (nodes, edges), refusal) in refusals() {
        let mut t = base.clone();
        assert_eq!(t.extend(&nodes, &edges), Err(refusal), "{name}");
        assert_eq!(bytes(&t), bytes(&base), "{name}: the bytes");
        assert_eq!(t.strings().len(), base.strings().len(), "{name}: the arena");
        // The same batch through the columnar path: the same refusal, the same untouched
        // topology, and the next valid batch still lands on the rebuild — so the refused
        // rows claimed no id and no arena slot on either path.
        let mut columns = base.clone();
        let doc = columns::Doc::of(&nodes, &edges);
        assert_eq!(
            doc.append(&mut columns),
            Err(BatchRefusal::Extend(refusal)),
            "{name}: the columns path"
        );
        assert_eq!(bytes(&columns), bytes(&base), "{name}: the columns bytes");
        assert_eq!(
            columns.strings().len(),
            base.strings().len(),
            "{name}: the columns arena"
        );
        columns::Doc::of(&next.0, &next.1)
            .append(&mut columns)
            .expect("the next valid batch, in columns");
        assert_matches_rebuild(&columns, &[first.clone(), next.clone()], name);
        t.extend(&next.0, &next.1).expect("the next valid batch");
        assert_matches_rebuild(&t, &[first.clone(), next.clone()], name);
    }
}

#[test]
fn a_batch_that_could_overflow_a_count_is_refused_before_anything_is_counted_in() {
    let max = u64::from(u32::MAX);
    let held = Load {
        strings: max - 3,
        bytes: 10,
        nodes: 5,
        edges: ADJACENCY_LIMIT - 1,
    };
    let refused = |batch: Load| held.check(batch).err();
    let arena = Some(ExtendError::Capacity {
        what: "string arena",
    });
    let one = |load: Load| Load { nodes: 1, ..load };
    assert_eq!(
        refused(one(Load {
            strings: 4,
            ..Load::default()
        })),
        arena
    );
    assert_eq!(
        refused(one(Load {
            bytes: max - 9,
            ..Load::default()
        })),
        arena
    );
    let nodes = Load {
        nodes: max - 4,
        ..Load::default()
    };
    assert_eq!(
        refused(nodes),
        Some(ExtendError::Capacity { what: "node index" })
    );
    let edges = Load {
        edges: 2,
        ..Load::default()
    };
    assert_eq!(
        refused(edges),
        Some(ExtendError::Capacity { what: "adjacency" })
    );
    assert_eq!(
        refused(Load {
            strings: 3,
            bytes: max - 10,
            nodes: max - 5,
            edges: 1
        }),
        None
    );
    let full = Load {
        edges: ADJACENCY_LIMIT + 1,
        ..held
    };
    assert_eq!(
        full.check(Load::default()),
        Ok(()),
        "a batch with no edge adds no adjacency"
    );
}

#[test]
fn the_batch_load_counts_every_string_as_new() {
    let mut n = node("abc", "db");
    n.icon = Some("ic".into());
    let mut e = edge("e", "abc", "abc");
    e.record_id = Some("abc".into());
    let load = Load::of_batch(&[n.clone(), n], &[e]);
    let node_bytes = 3 + 2 + 2 + 4 + 2;
    let want = Load {
        strings: 2 * 5 + 3,
        bytes: 2 * node_bytes + 1 + 3,
        nodes: 2,
        edges: 1,
    };
    assert_eq!(load, want);
}

#[test]
fn an_empty_batch_changes_nothing() {
    let base = index_model(&[node("a", "db")], &[edge("e", "a", "a")]).expect("fits");
    let mut t = base.clone();
    assert_eq!(t.extend(&[], &[]), Ok(()));
    assert_eq!(bytes(&t), bytes(&base));
    assert_eq!(t.strings().len(), base.strings().len());
}
