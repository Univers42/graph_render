//! `index_columns` against `index_model` over the same graph: the memo must give every row
//! the handle interning its text would, whatever order or repetition the table holds.

use super::*;
use crate::columns::NodeKind;
use crate::edgekind::EdgeKind;
use crate::index::index_model;
use crate::records::{EdgeRecord, NodeRecord};

/// Strings shared across fields and rows: node 7's label is node 9's id, interned two rows
/// before the id is, and edge labels repeat node ids.
fn records() -> (Vec<NodeRecord>, Vec<EdgeRecord>) {
    let nodes = (0..12u32).map(|i| NodeRecord {
        id: format!("n{i}"),
        kind: NodeKind::ALL[i as usize % 4],
        database_id: (i % 4 != 0).then(|| format!("db{}", i % 3)),
        source: ["pg", "md"][i as usize % 2].into(),
        label: if i == 7 {
            "n9".into()
        } else {
            format!("L{}", i % 5)
        },
        group: (i % 2 == 0).then(|| "g".into()),
        weight: f64::from(i) * 0.25,
        version: f64::from(i % 3),
        has_note: i % 2 == 1,
        icon: (i % 3 == 0).then(|| format!("i{}", i % 2)),
    });
    let edges = (0..20u32).map(|j| EdgeRecord {
        id: format!("e{j}"),
        source: format!("n{}", j % 12),
        target: format!("n{}", (j * 5 + 1) % 12),
        kind: EdgeKind::ALL[j as usize % 5],
        label: if j % 3 == 0 {
            format!("n{}", j % 12)
        } else {
            String::new()
        },
        strength: f64::from(j) / 20.0,
        directed: j % 2 == 0,
        record_id: (j % 2 == 1).then(|| format!("r{}", j % 4)),
        child_first: j % 3 == 0,
    });
    (nodes.collect(), edges.collect())
}

fn node_cells(node: &NodeRecord, entry: &mut impl FnMut(&str) -> u32) -> NodeCells {
    NodeCells {
        id: entry(&node.id),
        kind: entry(node.kind.as_str()),
        database_id: node.database_id.as_deref().map(&mut *entry),
        source: entry(&node.source),
        label: entry(&node.label),
        group: node.group.as_deref().map(&mut *entry),
        weight: node.weight,
        version: node.version,
        has_note: node.has_note,
        icon: node.icon.as_deref().map(&mut *entry),
    }
}

fn edge_cells(
    edge: &EdgeRecord,
    rows: &[NodeRecord],
    entry: &mut impl FnMut(&str) -> u32,
) -> EdgeCells {
    let row = |id: &str| rows.iter().position(|n| n.id == id).expect("a node") as u32;
    EdgeCells {
        id: entry(&edge.id),
        source_row: row(&edge.source),
        target_row: row(&edge.target),
        kind: entry(edge.kind.as_str()),
        label: entry(&edge.label),
        strength: edge.strength,
        directed: edge.directed,
        record_id: edge.record_id.as_deref().map(&mut *entry),
        child_first: edge.child_first,
    }
}

/// The rows over a table whose entries `entry` hands out.
fn rows(entry: &mut impl FnMut(&str) -> u32) -> (Vec<NodeCells>, Vec<EdgeCells>) {
    let (nodes, edges) = records();
    let node_rows = nodes.iter().map(|n| node_cells(n, entry)).collect();
    let edge_rows = edges.iter().map(|e| edge_cells(e, &nodes, entry)).collect();
    (node_rows, edge_rows)
}

/// One entry per cell, in row order: every repeat its own entry.
fn per_cell() -> Doc {
    let mut table = Vec::new();
    let (nodes, edges) = rows(&mut |text| {
        table.push(text.to_owned());
        (table.len() - 1) as u32
    });
    Doc {
        table,
        nodes,
        edges,
    }
}

/// Each distinct string once, in the reverse of first use: no entry order matches the
/// arena's slot order.
fn deduped_reversed() -> Doc {
    let mut table: Vec<String> = Vec::new();
    for text in per_cell().table {
        if !table.contains(&text) {
            table.push(text);
        }
    }
    table.reverse();
    let at = |text: &str| table.iter().position(|t| t == text).expect("listed") as u32;
    let (nodes, edges) = rows(&mut |text| at(text));
    Doc {
        table,
        nodes,
        edges,
    }
}

/// Every field of both topologies. Derived `Debug` prints contents, never capacity, so equal
/// strings mean an equal arena, columns, CSRs, database index and note count. Stricter than
/// `==` on floats, as "the same build" should be: `-0.0` and `0.0` print differently.
fn same(a: &Topology, b: &Topology) -> bool {
    format!("{a:?}") == format!("{b:?}")
}

#[test]
fn every_handle_matches_index_model_whatever_the_table_order() {
    let (nodes, edges) = records();
    let model = index_model(&nodes, &edges).expect("fits");
    assert_eq!((model.node_count(), model.edge_count()), (12, 20));
    for (name, doc) in [
        ("per cell", per_cell()),
        ("deduped, reversed", deduped_reversed()),
    ] {
        let columns = doc.index().expect("every id is fresh");
        assert!(same(&model, &columns), "{name}");
    }
}

#[test]
fn the_comparison_sees_two_rows_trade_labels() {
    // The negative control: `same` must not pass a document whose rows differ.
    let (nodes, edges) = records();
    let model = index_model(&nodes, &edges).expect("fits");
    let mut doc = per_cell();
    let (a, b) = (doc.nodes[1].label, doc.nodes[2].label);
    (doc.nodes[1].label, doc.nodes[2].label) = (b, a);
    assert!(!same(&model, &doc.index().expect("still valid")));
}

#[test]
fn the_comparison_sees_a_field_no_row_reads_back() {
    // The note count is in no row: the field-by-field comparison this replaced never saw it.
    let (nodes, edges) = records();
    let model = index_model(&nodes, &edges).expect("fits");
    let mut columns = per_cell().index().expect("every id is fresh");
    columns.notes += 1;
    assert!(!same(&model, &columns));
}
