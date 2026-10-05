//! The columnar append against the record append: `extend_columns` over a `GMX1` batch must
//! leave the topology byte-identical to `extend` over the same records
//! (`docs/decisions/extend-columns.md`, conditions 4 and 5), and refuse the same rows for the
//! same reasons.
//!
//! The rows here are built from the *records* rather than from wire bytes, because graph-core
//! knows no wire format: the string table is a plain `[String]` lent as an [`EntryTable`],
//! exactly as the whole-document path's tests lend one. What is being compared is the append,
//! not the encoding — `graph-wasm`'s own tests cover the bytes.
//!
//! The refusals are the existing `refusals()` table, not a second one: the columns twin of a
//! JSON refusal is the same batch through the other path, so the table is walked once by
//! `extend_refusal_leaves_topology_unchanged` for both. The rows a record batch cannot spell
//! — the three [`BatchRefusal`] variants beyond `extend`'s four — are rows of that same table,
//! carrying a [`Damage`] the columnar half applies; see the enum for why they must be edited
//! rather than built.

use super::*;
use crate::index::columns::{BatchEdgeCells, batch_load};
use crate::index::columns::{EntryTable, NodeCells};
use crate::index::extend::columns::BatchRefusal;

/// The batch's string table, lent to graph-core. A newtype rather than a second `impl
/// EntryTable for [S]`: the whole-document tests already implement the trait for slices, and
/// two implementations for one type cannot coexist in a crate.
#[derive(Default)]
pub(super) struct Doc {
    table: Vec<String>,
    nodes: Vec<NodeCells>,
    edges: Vec<BatchEdgeCells>,
}

/// The one-cell edit a `GMX1` buffer can carry and no record can spell: a `NodeRecord`'s and
/// an `EdgeRecord`'s kind are enumerations, and neither type indexes a string table at all.
/// So the three refusals [`BatchRefusal`] gives beyond `extend`'s four are unreachable over
/// records — which is why [`Doc::damage`] edits a built batch rather than a fixture building it.
pub(super) enum Damage {
    /// Batch node `index`'s kind entry names no node kind.
    NodeKind(u32),
    /// Batch edge `index`'s kind entry names no edge kind.
    EdgeKind(u32),
    /// Batch edge `index`'s source entry is `entry`, past the end of the table. `u32::MAX` is
    /// what a corrupt index looks like; any value over the table's length refuses the same way.
    SourceEntry { index: u32, entry: u32 },
}

impl Doc {
    /// The records `nodes` and `edges` as a `GMX1` batch: every string an entry, every
    /// endpoint an entry **naming a node id**. An endpoint naming a node the batch does not
    /// carry is interned here on demand, the one thing a batch's endpoints may do.
    pub(super) fn of(nodes: &[NodeRecord], edges: &[EdgeRecord]) -> Doc {
        let mut doc = Self::default();
        for n in nodes {
            let cells = doc.node_cells(n);
            doc.nodes.push(cells);
        }
        for e in edges {
            let cells = doc.edge_cells(e);
            doc.edges.push(cells);
        }
        doc
    }

    /// One node row: six entries at most, the three scalars across.
    fn node_cells(&mut self, n: &NodeRecord) -> NodeCells {
        NodeCells {
            id: self.entry(&n.id),
            kind: self.entry(n.kind.as_str()),
            database_id: n.database_id.as_deref().map(|t| self.entry(t)),
            source: self.entry(&n.source),
            label: self.entry(&n.label),
            group: n.group.as_deref().map(|t| self.entry(t)),
            weight: n.weight,
            version: n.version,
            has_note: n.has_note,
            icon: n.icon.as_deref().map(|t| self.entry(t)),
        }
    }

    /// One edge row: five entries, and both endpoints as entries naming a node id.
    fn edge_cells(&mut self, e: &EdgeRecord) -> BatchEdgeCells {
        BatchEdgeCells {
            id: self.entry(&e.id),
            source_entry: self.entry(&e.source),
            target_entry: self.entry(&e.target),
            kind: self.entry(e.kind.as_str()),
            label: self.entry(&e.label),
            record_id: e.record_id.as_deref().map(|t| self.entry(t)),
            strength: e.strength,
            directed: e.directed,
            child_first: e.child_first,
        }
    }

    /// This batch with the one cell `damage` names overwritten, so a batch no set of records
    /// can produce reaches the append.
    pub(super) fn damage(&mut self, damage: Damage) {
        match damage {
            Damage::NodeKind(index) => {
                let kind = self.entry("not a node kind");
                self.nodes[index as usize].kind = kind;
            }
            Damage::EdgeKind(index) => {
                let kind = self.entry("not an edge kind");
                self.edges[index as usize].kind = kind;
            }
            Damage::SourceEntry { index, entry } => {
                self.edges[index as usize].source_entry = entry;
            }
        }
    }

    /// One node row, every optional present, id `id`.
    pub(super) fn node(&mut self, id: &str) {
        let id = self.entry(id);
        let kind = self.entry("record");
        let database_id = Some(self.entry("db"));
        let source = self.entry("pg");
        let label = self.entry("L");
        let group = Some(self.entry("g"));
        let icon = Some(self.entry("i"));
        self.nodes.push(NodeCells {
            id,
            kind,
            database_id,
            source,
            label,
            group,
            weight: 0.5,
            version: 0.0,
            has_note: false,
            icon,
        });
    }

    /// One node row with every optional absent: three strings, as `Load::of_batch` counts.
    pub(super) fn bare_node(&mut self, id: &str) {
        let id = self.entry(id);
        let kind = self.entry("record");
        let source = self.entry("pg");
        let label = self.entry("L");
        self.nodes.push(NodeCells {
            id,
            kind,
            database_id: None,
            source,
            label,
            group: None,
            weight: 0.5,
            version: 0.0,
            has_note: false,
            icon: None,
        });
    }

    fn entry(&mut self, text: &str) -> u32 {
        self.table.push(text.to_owned());
        (self.table.len() - 1) as u32
    }

    /// Appends this batch to `t`, the way `gm_graph_extend_columns`'s body does.
    pub(super) fn append(&self, t: &mut Topology) -> Result<(), BatchRefusal> {
        t.extend_columns(
            &Table(&self.table),
            self.nodes.iter().copied(),
            self.edges.iter().copied(),
        )
    }

    /// What this batch would add to a graph already holding `held`, as the production count
    /// counts it.
    pub(super) fn load(&self) -> Load {
        batch_load(
            &Table(&self.table),
            self.nodes.iter().copied(),
            self.edges.iter().copied(),
        )
    }
}

struct Table<'a>(&'a [String]);

impl EntryTable for Table<'_> {
    fn entries(&self) -> usize {
        self.0.len()
    }

    fn bytes(&self) -> usize {
        self.0.iter().map(String::len).sum()
    }

    fn text(&self, entry: u32) -> Option<&str> {
        self.0.get(entry as usize).map(String::as_str)
    }
}

/// The columns twin of `extend_matches_index_model`: the same seeded stream, appended through
/// the other path, judged by the same [`bytes`] and the same id lookups.
///
/// Seeds `0..96` and [`stream`] are this file's, not new randomness (D1/D10).
#[test]
fn extend_columns_matches_extend() {
    for seed in 0..96 {
        let batches = stream(seed);
        let mut from_records = empty_model();
        let mut from_columns = empty_model();
        for (k, batch) in batches.iter().enumerate() {
            from_records.extend(&batch.0, &batch.1).expect("strict");
            Doc::of(&batch.0, &batch.1)
                .append(&mut from_columns)
                .expect("strict");
            let what = format!("seed {seed}, batch {k}");
            assert_eq!(
                bytes(&from_columns),
                bytes(&from_records),
                "{what}: the topology stage's bytes"
            );
            assert_eq!(from_columns.stats(), from_records.stats(), "{what}");
            for n in &batch.0 {
                assert_eq!(
                    from_columns.node_index(&n.id),
                    from_records.node_index(&n.id),
                    "{what}: {}",
                    n.id
                );
            }
            for e in &batch.1 {
                assert_eq!(
                    from_columns.edge_index(&e.id),
                    from_records.edge_index(&e.id),
                    "{what}: {}",
                    e.id
                );
            }
        }
    }
}

/// An empty batch changes nothing, as over records.
#[test]
fn an_empty_columns_batch_changes_nothing() {
    let base = index_model(&[node("a", "db")], &[edge("e", "a", "a")]).expect("fits");
    let mut t = base.clone();
    assert_eq!(Doc::of(&[], &[]).append(&mut t), Ok(()));
    assert_eq!(bytes(&t), bytes(&base));
    assert_eq!(t.strings().len(), base.strings().len());
}
