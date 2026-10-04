//! Legend counts (`src/core/model/legend.ts`) — **counts and bucketing only** (H7).
//!
//! The oracle's `deriveLegend` also calls `databaseColor` and emits OKLCH. Colour is
//! presentation and stays in TypeScript: nothing here returns, or needs, a colour. A
//! database entry's `label` is its `id` in the oracle, so only the id is carried.

use crate::columns::{NodeColumns, NodeKind};
use crate::index::Topology;
use core::cmp::Reverse;

/// One database: its id and how many nodes carry it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatabaseCount<'a> {
    /// Database id (also the oracle's `label`).
    pub id: &'a str,
    /// Nodes in it.
    pub count: u32,
}

/// One tag hub: the node and its incident edge count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TagCount {
    /// Dense index of the `tag` node.
    pub node: u32,
    /// Its adjacency length.
    pub count: u32,
}

/// `Legend`, minus colour.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LegendCounts<'a> {
    /// Databases, most nodes first; ties keep first-seen order.
    pub databases: Vec<DatabaseCount<'a>>,
    /// Tag hubs, most edges first; ties keep node order.
    pub tags: Vec<TagCount>,
    /// Node kinds present, in first-seen order, with their counts.
    pub kinds: Vec<(NodeKind, u32)>,
}

/// `deriveLegend` (`legend.ts:29-48`), counts only. Both sorts are stable (D5) —
/// `sort_by_key` keeps equal keys in order, as `Array.prototype.sort` does —
/// descending by count.
pub fn derive_legend(topology: &Topology) -> LegendCounts<'_> {
    let mut databases: Vec<_> = topology
        .by_database()
        .map(|(id, nodes)| DatabaseCount {
            id,
            count: wire_count(nodes.len()),
        })
        .collect();
    databases.sort_by_key(|d| Reverse(d.count));
    let columns = topology.nodes();
    let mut tags: Vec<_> = (0..)
        .zip(&columns.kind)
        .filter(|&(_, &kind)| kind == NodeKind::Tag)
        .map(|(node, _)| TagCount {
            node,
            count: tag_degree(columns, node),
        })
        .collect();
    tags.sort_by_key(|t| Reverse(t.count));
    let mut kinds: Vec<(NodeKind, u32)> = Vec::with_capacity(NodeKind::ALL.len());
    for &kind in &columns.kind {
        match kinds.iter_mut().find(|(seen, _)| *seen == kind) {
            Some((_, count)) => *count += 1,
            None => kinds.push((kind, 1)),
        }
    }
    LegendCounts {
        databases,
        tags,
        kinds,
    }
}

/// `len` as the wire's `u32` (D6). A database's members are a slice of the `node_ids`
/// row, so `len() <= node_count()`, and a node index is below `u32::MAX` by construction
/// (`index::next_index` refuses that one index), so the `expect` cannot fire for any
/// topology a caller can build — it says so where a silent wrap would not.
fn wire_count(len: usize) -> u32 {
    u32::try_from(len).expect("a database's members fit u32: node_count() is below u32::MAX")
}

/// Node `node`'s degree. Checked, because `node` walks `kind` and `degree` side by side:
/// a degree column one push short of `kind` would otherwise read the next node's degree,
/// or run off the end of the column.
fn tag_degree(columns: &NodeColumns, node: u32) -> u32 {
    columns
        .degree
        .get(node as usize)
        .copied()
        .expect("every node has a degree: the columns are filled in one pass")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::index_model;
    use crate::records::build::{edge, kind, node};

    #[test]
    fn databases_sort_by_count_keeping_first_seen_order_on_ties() {
        let nodes = [
            node("a", "db1"),
            node("b", "db2"),
            node("c", "db2"),
            node("d", "db3"),
            node("e", ""),
        ];
        let t = index_model(&nodes, &[]).expect("fits");
        let legend = derive_legend(&t);
        let dbs: Vec<_> = legend.databases.iter().map(|d| (d.id, d.count)).collect();
        assert_eq!(dbs, [("db2", 2), ("db1", 1), ("db3", 1)]);
    }

    #[test]
    fn tags_count_incident_edges_and_sort_stably() {
        let nodes = [
            kind("t1", NodeKind::Tag),
            kind("t2", NodeKind::Tag),
            node("r", ""),
            kind("t3", NodeKind::Tag),
        ];
        let edges = [edge("x", "r", "t2"), edge("y", "t3", "t3")];
        let t = index_model(&nodes, &edges).expect("fits");
        let tags = derive_legend(&t).tags;
        let got: Vec<_> = tags.iter().map(|t| (t.node, t.count)).collect();
        assert_eq!(got, [(3, 2), (1, 1), (0, 0)]);
    }

    #[test]
    fn kinds_are_counted_in_first_seen_order() {
        let nodes = [
            kind("n", NodeKind::Note),
            node("r", ""),
            kind("m", NodeKind::Note),
            kind("d", NodeKind::Database),
        ];
        let t = index_model(&nodes, &[]).expect("fits");
        let kinds = derive_legend(&t).kinds;
        let want = [
            (NodeKind::Note, 2),
            (NodeKind::Record, 1),
            (NodeKind::Database, 1),
        ];
        assert_eq!(kinds, want);
        assert_eq!(
            derive_legend(&index_model(&[], &[]).expect("fits")),
            LegendCounts::default()
        );
    }

    /// F-39: the count is narrowed with a check, and the check names what guarantees it.
    /// `derive_legend` itself cannot reach it — a database's members are a slice of the
    /// `node_ids` row, so the count is below `u32::MAX` — so the guard is pinned here.
    #[test]
    #[should_panic(expected = "a database's members fit u32")]
    fn a_member_count_past_the_u32_index_space_is_refused() {
        assert_eq!(wire_count(usize::MAX), 0);
    }

    #[test]
    fn a_member_count_inside_the_u32_index_space_is_written_as_is() {
        assert_eq!(wire_count(0), 0);
        assert_eq!(wire_count(u32::MAX as usize), u32::MAX);
    }

    /// F-38: `derive_legend` walks `kind` and `degree` together, so a `degree` column one
    /// push short must be refused rather than read as the next node's degree.
    #[test]
    #[should_panic(expected = "every node has a degree")]
    fn a_tag_past_the_degree_column_is_refused() {
        let mut columns = NodeColumns::with_capacity(4);
        columns.kind.push(NodeKind::Tag);
        assert_eq!(tag_degree(&columns, 0), 0);
    }

    #[test]
    fn a_tag_inside_the_degree_column_reads_its_own_degree() {
        let mut columns = NodeColumns::with_capacity(2);
        columns.kind.push(NodeKind::Record);
        columns.degree.push(7);
        columns.kind.push(NodeKind::Tag);
        columns.degree.push(3);
        assert_eq!(tag_degree(&columns, 1), 3);
    }
}
