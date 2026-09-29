//! The shared assertion: `want` is in d3's pre-order, the port emits in dense-index
//! order, so each row is looked up by id rather than by position.

use super::super::{build, nodes, points, run, tree};
use super::Row;
use crate::index::Topology;
use crate::records::NodeRecord;

/// Builds a tree from (parent, child) **dense-index** pairs, with ids `v0..v{n-1}` in
/// admission order — the order this crate's `index_model` assigns dense indices, which is
/// also the order the emitted columns come out in.
pub(crate) fn indexed(edges: &[(usize, usize)]) -> (Vec<String>, Vec<crate::records::EdgeRecord>) {
    let n = edges
        .iter()
        .map(|(_, c)| *c)
        .max()
        .expect("at least one edge")
        + 1;
    let ids: Vec<String> = (0..n).map(|i| format!("v{i}")).collect();
    let recs = edges
        .iter()
        .enumerate()
        .map(|(i, (p, c))| tree(&format!("e{i}"), &ids[*p], &ids[*c], "parent_of"))
        .collect();
    (ids, recs)
}

pub(crate) fn assert_indexed(edges: &[(usize, usize)], want: &[Row], label: &str) {
    let (ids, recs) = indexed(edges);
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    assert_golden(&build(&nodes(&refs), &recs), &refs, want, label);
}

/// The ids of `records`, in the order they are admitted — the dense index order the port
/// emits in. The fixture loader preserves file order, so this is the fixture's own node
/// list order.
pub(crate) fn dense_ids(records: &[NodeRecord]) -> Vec<&str> {
    records.iter().map(|n| n.id.as_str()).collect()
}

/// Asserts the layout of `t` against `want`.
///
/// `want` is in d3's **pre-order** (the order the oracle prints, which follows the tree),
/// while the port emits columns in **dense-index** order (the order the nodes are
/// admitted in) — two different orders for every shape here. `dense` gives the id at each
/// dense index so each golden row is looked up by identity, not by position; a test that
/// got the node ordering wrong then fails on a value rather than silently comparing the
/// wrong pair.
pub(crate) fn assert_golden(t: &Topology, dense: &[&str], want: &[Row], label: &str) {
    let (x, y) = points(&run(t).expect(label));
    assert_eq!(x.len(), want.len(), "{label}: node count");
    assert_eq!(dense.len(), want.len(), "{label}: dense list length");
    for (id, wx, wy) in want {
        let i = dense
            .iter()
            .position(|d| d == id)
            .unwrap_or_else(|| panic!("{label}: no dense row for {id}"));
        assert_eq!(x[i].to_bits(), *wx, "{label}: x of {id} (dense {i})");
        assert_eq!(y[i].to_bits(), *wy, "{label}: y of {id} (dense {i})");
    }
}
