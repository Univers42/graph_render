use super::*;
use crate::index::index_model;
use crate::layout::sugiyama::acyclic::Arcs;
use crate::layout::sugiyama::layering::{DUMMY_BUDGET, assign_layers};
use crate::layout::sugiyama::ordering::Ordering;
use crate::records::build::{edge, node};
use crate::records::{EdgeRecord, NodeRecord};

/// `(offsets, pts)` for `nodes`/`edges`, edges in the order given.
fn paths(nodes: &[NodeRecord], edges: &[EdgeRecord]) -> Paths {
    let t = index_model(nodes, edges).expect("fits");
    let acyclic = Acyclic::of(&t);
    let list = Arcs::new(&t, &acyclic).grouped();
    let layer = assign_layers(&list);
    let layering = Layering::build(&list, &layer, DUMMY_BUDGET);
    let num_layers = layering.layer_of.iter().copied().max().map_or(0, |m| m + 1);
    let ordering = Ordering::build(&layering, num_layers);
    let coords = Coords::build(&ordering, &layering, t.node_count());
    let routing = Routing {
        layering: &layering,
        coords: &coords,
        acyclic: &acyclic,
        spacing: LAYER_SPACING,
    };
    edge_paths(&routing)
}

#[test]
fn a_direct_edge_first_and_last_has_a_zero_width_offset_pair() {
    // Both edges are Direct (span 1): every offset equals the one before it.
    let n = ["a", "b", "c"].map(|id| node(id, ""));
    let e = [edge("ab", "a", "b"), edge("bc", "b", "c")];
    let p = paths(&n, &e);
    assert_eq!(
        p.offsets,
        [0, 0, 0],
        "first and last edge both zero-interior"
    );
    assert!(p.pts.is_empty());
}

#[test]
fn a_max_span_edge_owns_one_point_per_dummy_source_to_target() {
    // a->d spans layers 0..3: 2 dummies, walked in ascending (unreversed) order.
    let n = ["a", "b", "c", "d"].map(|id| node(id, ""));
    let e = [
        edge("ab", "a", "b"),
        edge("bc", "b", "c"),
        edge("cd", "c", "d"),
        edge("ad", "a", "d"),
    ];
    let p = paths(&n, &e);
    assert_eq!(
        p.offsets,
        [0, 0, 0, 0, 2],
        "the last edge owns the only points"
    );
    assert_eq!(p.pts.len(), 4, "2 dummies × (x, y)");
    assert!(
        p.pts[1] < p.pts[3],
        "y rises from the first dummy to the second"
    );
}

#[test]
fn a_reversed_max_span_edge_still_runs_source_to_target() {
    // d->a closes the cycle a->b->c->d->a: it reverses, so its dummy chain (built
    // tail=a to head=d) is walked back to front to still start at d and end at a.
    let n = ["a", "b", "c", "d"].map(|id| node(id, ""));
    let e = [
        edge("ab", "a", "b"),
        edge("bc", "b", "c"),
        edge("cd", "c", "d"),
        edge("da", "d", "a"),
    ];
    let p = paths(&n, &e);
    let (start, end) = (p.offsets[3] as usize, p.offsets[4] as usize);
    let ys: Vec<f32> = p.pts[2 * start..2 * end]
        .chunks(2)
        .map(|xy| xy[1])
        .collect();
    assert!(ys.windows(2).all(|w| w[0] > w[1]), "descends: {ys:?}");
}

// Structural invariants (`prompts/phase-05-sugiyama.md:124`): the gate names `cargo
// test -p graph-core sugiyama_invariants` for "acyclic after FAS, layers monotonic,
// dummy chains contiguous" (RED, observed: ran green over 0 tests, nothing on disk
// matched the name). Closes the gap over the public `run`, the 6 fixtures and a
// synthetic sweep.
use crate::index::Topology;
use crate::layout::sugiyama::run;
use crate::synthetic::Mulberry32;
use graph_contract::canonical_json::{Value, parse};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
use graph_contract::notes::NoteCode;

const DAG_FIXTURES: [&str; 7] = [
    include_str!("../../../../../../fixtures/dag/chain.json"),
    include_str!("../../../../../../fixtures/dag/diamond.json"),
    include_str!("../../../../../../fixtures/dag/cyclic.json"),
    include_str!("../../../../../../fixtures/dag/multi-span.json"),
    include_str!("../../../../../../fixtures/dag/wide-layer.json"),
    include_str!("../../../../../../fixtures/dag/disconnected.json"),
    include_str!("../../../../../../fixtures/dag/parallel-arcs.json"),
];
type DagGraph = (Vec<String>, Vec<(String, String, String)>);

fn dag_member<'a>(v: &'a Value, k: &str) -> &'a Value {
    let Value::Object(m) = v else {
        panic!("not an object")
    };
    &m.iter().find(|(key, _)| key == k).expect("member").1
}
fn dag_text(v: &Value, k: &str) -> String {
    match dag_member(v, k) {
        Value::String(s) => s.clone(),
        other => panic!("{k}: {other:?}"),
    }
}
fn dag_array<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    match dag_member(v, k) {
        Value::Array(a) => a,
        other => panic!("{k}: {other:?}"),
    }
}
fn dag_edge(e: &Value) -> (String, String, String) {
    (
        dag_text(e, "id"),
        dag_text(e, "source"),
        dag_text(e, "target"),
    )
}
/// One fixture's `(node ids, (edge id, source, target))`.
fn load_dag(fixture: &str) -> DagGraph {
    let root = parse(fixture).expect("valid fixture json");
    let nn = dag_array(&root, "nodes").iter().map(|n| dag_text(n, "id"));
    let ee = dag_array(&root, "edges").iter().map(dag_edge);
    (nn.collect(), ee.collect())
}
/// A random `6..26`-node DAG, edges only `i < j`, density 0.2, from `seed`.
fn synthetic_dag(seed: u32) -> DagGraph {
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
fn dag_topology(nodes: &[String], edges: &[(String, String, String)]) -> Topology {
    let n: Vec<_> = nodes.iter().map(|id| node(id, "")).collect();
    let e: Vec<_> = edges.iter().map(|(id, s, t)| edge(id, s, t)).collect();
    index_model(&n, &e).expect("fixture and synthetic graphs always fit")
}

/// Every non-loop edge: acyclic orientation holds (tail, reversed-aware, at a
/// strictly lower Y than head), and every hop of its route steps exactly one
/// [`LAYER_SPACING`] — except a `dag.dummy_budget_exceeded` edge, exempted from the
/// hop-size check by the phase's documented degradation (still checked for
/// orientation).
fn assert_invariants(nodes: &[String], edges: &[(String, String, String)]) {
    let t = dag_topology(nodes, edges);
    let geometry = run(&t, LAYER_SPACING).expect("sugiyama never fails");
    let NodeGeometry::Point { y, .. } = &geometry.nodes else {
        panic!("sugiyama always emits Point nodes")
    };
    let EdgeGeometry::Polyline(paths) = &geometry.edges else {
        panic!("sugiyama always emits Polyline edges")
    };
    let reversed: Vec<u32> = geometry
        .notes
        .iter()
        .filter(|n| n.code == NoteCode::EdgeReversed)
        .map(|n| n.index)
        .collect();
    let over_budget: Vec<u32> = geometry
        .notes
        .iter()
        .filter(|n| n.code == NoteCode::DummyBudgetExceeded)
        .map(|n| n.index)
        .collect();
    let index_of = |id: &str| nodes.iter().position(|n| n == id).expect("known id") as u32;
    for (e, (_, s, tgt)) in edges.iter().enumerate() {
        if s == tgt {
            continue; // self-loop: no arc to orient
        }
        let e = e as u32;
        let (source, target) = (index_of(s), index_of(tgt));
        let (tail, head) = if reversed.contains(&e) {
            (target, source)
        } else {
            (source, target)
        };
        assert!(
            y[tail as usize] < y[head as usize],
            "edge {e} ({s}->{tgt}): orientation violated"
        );
        if over_budget.contains(&e) {
            continue; // documented degradation: straight, span may exceed one hop
        }
        let start = paths.offsets[e as usize] as usize;
        let end = paths.offsets[e as usize + 1] as usize;
        let mut walk = vec![y[source as usize]];
        walk.extend(paths.pts[2 * start..2 * end].chunks(2).map(|xy| xy[1]));
        walk.push(y[target as usize]);
        for hop in walk.windows(2) {
            let step = (hop[1] - hop[0]).abs();
            assert_eq!(step, LAYER_SPACING, "edge {e} ({s}->{tgt}): hop {hop:?}");
        }
    }
}

#[test]
fn sugiyama_invariants() {
    for fixture in DAG_FIXTURES {
        let (nodes, edges) = load_dag(fixture);
        assert_invariants(&nodes, &edges);
    }
    for seed in 0..64u32 {
        let (nodes, edges) = synthetic_dag(seed);
        if !edges.is_empty() {
            assert_invariants(&nodes, &edges);
        }
    }
}
