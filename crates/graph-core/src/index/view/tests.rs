//! The accessors' documented preconditions (F-20), and the construction facts other
//! modules index on without a check: ascending out/in rows (`Incident::merge`, F-23), a
//! degree per node (`legend.rs`, F-38), and a node count below `u32::MAX` (F-39).

use crate::index::{Topology, empty_model, next_index};
use crate::synthetic::build_synthetic_model;

fn model() -> Topology {
    build_synthetic_model(300.0).expect("fits")
}

#[test]
#[should_panic(expected = "index out of bounds")]
fn incident_on_the_empty_model_panics() {
    let _ = empty_model().incident(0);
}

#[test]
#[should_panic(expected = "index out of bounds")]
fn node_at_the_node_count_panics() {
    let t = model();
    let _ = t.node(t.node_count());
}

#[test]
#[should_panic(expected = "index out of bounds")]
fn edge_at_the_edge_count_panics() {
    let t = model();
    let _ = t.edge(t.edge_count());
}

#[test]
fn out_and_inbound_rows_are_ascending_so_incident_merges_them() {
    let t = model();
    for v in 0..t.node_count() {
        let (out, inbound) = (t.out().row(v), t.inbound().row(v));
        assert!(out.is_sorted() && inbound.is_sorted(), "node {v}");
        let mut want = [out, inbound].concat();
        want.sort_unstable();
        assert_eq!(t.incident(v).collect::<Vec<_>>(), want, "node {v}");
    }
}

#[test]
fn the_degree_column_holds_one_entry_per_node() {
    for t in [empty_model(), model()] {
        assert_eq!(t.nodes().degree.len(), t.node_count() as usize);
        assert_eq!(t.nodes().kind.len(), t.node_count() as usize);
    }
}

#[test]
fn no_set_is_indexed_at_or_past_u32_max() {
    let last = u32::MAX - 1;
    assert_eq!(next_index(last as usize, "nodes"), Ok(last));
    assert!(next_index(u32::MAX as usize, "nodes").is_err());
}
