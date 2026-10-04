use super::*;
use crate::index::index_model;
use crate::records::build::{edge, node};
use petgraph::visit::EdgeRef as _;

fn topology() -> Topology {
    let nodes = [node("a", ""), node("b", ""), node("c", "")];
    let mut ab = edge("ab", "a", "b");
    ab.directed = true;
    ab.strength = 2.0;
    let bc = edge("bc", "b", "c"); // undirected: traversable both ways
    index_model(&nodes, &[ab, bc]).expect("fits")
}

#[test]
fn directed_edges_go_one_way_undirected_edges_go_both() {
    let t = topology();
    let g = CsrDigraph::new(&t);
    let from_a: Vec<_> = g.neighbors(NodeIx(0)).collect();
    let from_b: Vec<_> = g.neighbors(NodeIx(1)).collect();
    let from_c: Vec<_> = g.neighbors(NodeIx(2)).collect();
    assert_eq!(from_a, [NodeIx(1)], "a->b only, directed");
    assert_eq!(from_b, [NodeIx(2)], "b->c out edge");
    assert_eq!(from_c, [NodeIx(1)], "c->b: bc is undirected");
}

#[test]
fn node_identifiers_and_indexing_cover_the_dense_range() {
    let t = topology();
    let g = CsrDigraph::new(&t);
    let ids: Vec<_> = g.node_identifiers().collect();
    assert_eq!(ids, [NodeIx(0), NodeIx(1), NodeIx(2)]);
    assert_eq!(g.node_bound(), 3);
    assert_eq!(g.to_index(NodeIx(2)), 2);
    assert_eq!(g.from_index(2), NodeIx(2));
}

#[test]
fn edge_references_enumerate_every_edge_once_with_its_weight() {
    let t = topology();
    let g = CsrDigraph::new(&t);
    let refs: Vec<_> = g.edge_references().collect();
    assert_eq!(refs.len(), 2);
    assert_eq!((refs[0].source(), refs[0].target()), (NodeIx(0), NodeIx(1)));
    assert_eq!(*refs[0].weight(), 2.0);
}

#[test]
fn petgraph_reuse_counts_the_same_weak_component_as_labeling() {
    let t = topology();
    let g = CsrDigraph::new(&t);
    assert_eq!(petgraph::algo::connected_components(g), 1);
}

#[test]
#[cfg(target_pointer_width = "64")]
#[should_panic(expected = "node index exceeds u32")]
fn an_index_past_the_u32_space_panics_instead_of_wrapping() {
    let t = topology();
    let _ = CsrDigraph::new(&t).from_index(1usize << 32);
}
