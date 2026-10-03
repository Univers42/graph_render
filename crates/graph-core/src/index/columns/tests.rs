use super::*;

mod differential;

impl<S: AsRef<str>> EntryTable for [S] {
    fn entries(&self) -> usize {
        self.len()
    }

    fn bytes(&self) -> usize {
        self.iter().map(|s| s.as_ref().len()).sum()
    }

    fn text(&self, entry: u32) -> Option<&str> {
        self.get(entry as usize).map(AsRef::as_ref)
    }
}

/// A columnar document being built. Every [`entry`](Doc::entry) appends a fresh table entry,
/// repeats included, as the contract allows: two rows naming "a" name two entries.
#[derive(Default)]
struct Doc {
    table: Vec<String>,
    nodes: Vec<NodeCells>,
    edges: Vec<EdgeCells>,
}

impl Doc {
    fn entry(&mut self, text: &str) -> u32 {
        self.table.push(text.into());
        (self.table.len() - 1) as u32
    }

    /// A `record` node row in database `db`, source `pg`, label `L`.
    fn node(&mut self, id: &str) -> &mut NodeCells {
        let cells = NodeCells {
            id: self.entry(id),
            kind: self.entry("record"),
            database_id: Some(self.entry("db")),
            source: self.entry("pg"),
            label: self.entry("L"),
            group: None,
            weight: 0.5,
            version: 0.0,
            has_note: false,
            icon: None,
        };
        self.nodes.push(cells);
        self.nodes.last_mut().expect("just pushed")
    }

    /// An undirected `relation` edge row between two node rows.
    fn edge(&mut self, id: &str, source_row: u32, target_row: u32) -> &mut EdgeCells {
        let cells = EdgeCells {
            id: self.entry(id),
            source_row,
            target_row,
            kind: self.entry("relation"),
            label: self.entry(""),
            strength: 0.5,
            directed: false,
            record_id: None,
            child_first: false,
        };
        self.edges.push(cells);
        self.edges.last_mut().expect("just pushed")
    }

    fn index(&self) -> Result<Topology, ColumnsRefusal> {
        index_columns(
            &self.table[..],
            self.nodes.iter().copied(),
            self.edges.iter().copied(),
        )
    }

    /// The refusal this document earns. `Topology` is not `PartialEq` (it holds a whole
    /// arena and three CSRs), so the error is pulled out and compared on its own.
    fn refusal(&self) -> ColumnsRefusal {
        self.index().expect_err("this document is refused")
    }
}

fn doc(nodes: &[&str], edges: &[(&str, u32, u32)]) -> Doc {
    let mut doc = Doc::default();
    for id in nodes {
        doc.node(id);
    }
    for &(id, source, target) in edges {
        doc.edge(id, source, target);
    }
    doc
}

#[test]
fn a_row_is_the_dense_index_and_the_endpoints_are_already_resolved() {
    let ids = ["a", "b", "c"];
    let t = doc(&ids, &[("e0", 2, 0), ("e1", 0, 2)])
        .index()
        .expect("every id is fresh");
    assert_eq!((t.node_count(), t.edge_count()), (3, 2));
    for (row, id) in (0u32..).zip(ids) {
        assert_eq!(t.node(row).id, id, "row {row}");
        assert_eq!(t.node_index(id), Some(row));
    }
    assert_eq!((t.edge(0).source, t.edge(0).target), ("c", "a"));
    assert_eq!((t.out().row(2).len(), t.inbound().row(0).len()), (1, 1));
}

#[test]
fn the_same_id_under_two_table_entries_is_refused_not_merged() {
    // Two entries with the same bytes: what the arena compares, and what the document's
    // string table can produce by holding the value twice.
    let at = ColumnsRefusal::DuplicateNodeId { row: 2 };
    assert_eq!(doc(&["a", "b", "a"], &[]).refusal(), at);
}

#[test]
fn a_row_naming_an_earlier_rows_id_entry_is_refused_too() {
    // The memo's hit path: the second row reads the handle back instead of interning it.
    let mut doc = doc(&["a", "b"], &[]);
    let first = doc.nodes[0];
    doc.nodes.push(first);
    let at = ColumnsRefusal::DuplicateNodeId { row: 2 };
    assert_eq!(doc.refusal(), at);
}

#[test]
fn a_duplicate_in_the_last_row_is_refused_too() {
    let doc = doc(&["a", "b", "c", "a"], &[("e0", 0, 1)]);
    assert_eq!(doc.refusal(), ColumnsRefusal::DuplicateNodeId { row: 3 });
}

#[test]
fn an_edge_naming_the_row_of_the_second_duplicate_is_refused_not_attached_to_the_first() {
    // Row 2 repeats row 0's id. If the duplicate were dropped — first wins, as
    // `index_model` does — row 2 would vanish, the rows after it would shift down, and
    // this edge would silently point at whatever moved into row 2. Refusing the document is
    // the only answer that keeps "row r is index r" true.
    let doc = doc(&["a", "b", "a", "c"], &[("e0", 0, 1), ("e1", 2, 3)]);
    assert_eq!(doc.refusal(), ColumnsRefusal::DuplicateNodeId { row: 2 });
}

#[test]
fn a_repeated_edge_id_is_refused_and_claims_no_row() {
    let doc = doc(&["a", "b"], &[("e0", 0, 1), ("e0", 1, 0)]);
    assert_eq!(doc.refusal(), ColumnsRefusal::DuplicateEdgeId { row: 1 });
}

#[test]
fn an_endpoint_past_the_last_node_is_refused() {
    let at = ColumnsRefusal::EndpointRow { row: 0 };
    assert_eq!(doc(&["a", "b"], &[("e0", 0, 2)]).refusal(), at);
    assert_eq!(doc(&["a", "b"], &[("e0", 9, 0)]).refusal(), at);
}

#[test]
fn an_empty_document_indexes_to_the_empty_topology() {
    let t = doc(&[], &[]).index().expect("nothing to refuse");
    assert_eq!((t.node_count(), t.edge_count()), (0, 0));
    assert_eq!(t.stats().nodes, 0);
}

/// A table that claims more entries than any buffer could hold, and holds none.
struct Claims(usize);

impl EntryTable for Claims {
    fn entries(&self) -> usize {
        self.0
    }

    fn bytes(&self) -> usize {
        0
    }

    fn text(&self, _: u32) -> Option<&str> {
        None
    }
}

#[test]
fn a_huge_table_under_no_rows_reserves_for_the_rows_not_the_table() {
    // Reserving for the table's 2^40 entries is terabytes; reserving for zero rows is nothing.
    let t = index_columns(&Claims(1 << 40), core::iter::empty(), core::iter::empty())
        .expect("no row names an entry");
    assert_eq!((t.node_count(), t.edge_count()), (0, 0));
}

#[test]
fn the_reservation_is_the_smaller_count_and_saturates() {
    assert_eq!(reserved_entries(100, 2, 1), 15);
    assert_eq!(reserved_entries(4, 2, 1), 4);
    assert_eq!(reserved_entries(9, usize::MAX, usize::MAX), 9);
}

#[test]
fn a_kind_no_kind_is_named_is_refused_with_its_row() {
    let mut nodes = doc(&["a", "b"], &[]);
    nodes.nodes[1].kind = nodes.entry("vertex");
    assert_eq!(nodes.refusal(), ColumnsRefusal::NodeKind { row: 1 });
    let mut edges = doc(&["a", "b"], &[("e0", 0, 1)]);
    edges.edges[0].kind = edges.entry("Relation");
    assert_eq!(edges.refusal(), ColumnsRefusal::EdgeKind { row: 0 });
}

#[test]
fn an_entry_past_the_table_is_refused_before_any_memo_grows() {
    let mut label = doc(&["a"], &[]);
    label.nodes[0].label = 99;
    assert_eq!(label.refusal(), ColumnsRefusal::TableEntry { entry: 99 });
    // Resolved before stored: a memo sized to this entry would be 16 GiB.
    let mut group = doc(&["a"], &[]);
    group.nodes[0].group = Some(u32::MAX - 1);
    let at = ColumnsRefusal::TableEntry {
        entry: u32::MAX - 1,
    };
    assert_eq!(group.refusal(), at);
}

#[test]
fn a_refusal_names_the_row_and_the_reason() {
    let cases = [
        (
            ColumnsRefusal::DuplicateNodeId { row: 7 },
            "node row 7 repeats an id",
        ),
        (
            ColumnsRefusal::DuplicateEdgeId { row: 2 },
            "edge row 2 repeats an id",
        ),
        (
            ColumnsRefusal::EndpointRow { row: 1 },
            "edge row 1 names a node row that is gone",
        ),
        (
            ColumnsRefusal::TableEntry { entry: 4 },
            "string-table entry 4 does not exist",
        ),
        (
            ColumnsRefusal::NodeKind { row: 3 },
            "node row 3 names no node kind",
        ),
        (
            ColumnsRefusal::EdgeKind { row: 5 },
            "edge row 5 names no edge kind",
        ),
        (
            ColumnsRefusal::Capacity(CapacityError { what: "arena" }),
            "arena exceeds the u32 index space",
        ),
    ];
    for (refusal, text) in cases {
        assert_eq!(refusal.to_string(), text);
    }
}
