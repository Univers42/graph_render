//! The columnar rows as the document gives them — every string a string-table entry — and
//! the per-entry memo that resolves them.
//!
//! A table entry stands for one string however many rows name it: at 1M nodes a GMC1 table
//! holds about 4M entries but its rows name about 9M strings, and every kind name is named
//! once per row. Interning by text probed the arena's hash table for each of those 9M, and
//! that probe was 36% of the build (`docs/measurements/perf-open-intern.md`). Here an entry
//! is resolved the first time a row names it, and every later row reads it back from an
//! array. A hit returns exactly the handle `intern(text)` would — the text was interned when
//! the entry was first resolved — so the arena's slot order and every handle are those of
//! interning each row's text in turn.

use super::ColumnsRefusal;
use crate::arena::{Interned, StringArena};
use crate::columns::NodeKind;
use crate::edgekind::EdgeKind;

/// A document's string table: entries in index order, duplicates allowed.
pub trait StringTable {
    /// How many entries the table holds.
    fn entries(&self) -> usize;
    /// The UTF-8 length of all entries together.
    fn bytes(&self) -> usize;
    /// Entry `entry`, or `None` past the end.
    fn text(&self, entry: u32) -> Option<&str>;
}

/// One node row, every string a table entry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeCells {
    /// Entry of the stable id.
    pub id: u32,
    /// Entry of the kind name ([`NodeKind::as_str`]).
    pub kind: u32,
    /// Entry of the source database, if any.
    pub database_id: Option<u32>,
    /// Entry of the owning backend.
    pub source: u32,
    /// Entry of the label.
    pub label: u32,
    /// Entry of the secondary label, if any.
    pub group: Option<u32>,
    /// Visual weight.
    pub weight: f64,
    /// Version.
    pub version: f64,
    /// Note overlay flag.
    pub has_note: bool,
    /// Entry of the icon, if any.
    pub icon: Option<u32>,
}

/// One edge row, every string a table entry and both endpoints node rows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeCells {
    /// Entry of the content-addressed id.
    pub id: u32,
    /// Dense index of the source node.
    pub source_row: u32,
    /// Dense index of the target node.
    pub target_row: u32,
    /// Entry of the kind name ([`EdgeKind::as_str`]).
    pub kind: u32,
    /// Entry of the label.
    pub label: u32,
    /// Strength.
    pub strength: f64,
    /// Directed flag.
    pub directed: bool,
    /// Entry of the backing row id, if any.
    pub record_id: Option<u32>,
    /// `source_row` is the child (`child_of`).
    pub child_first: bool,
}

/// What each entry of one table resolved to, filled the first time a row names it.
///
/// Caveat: each memo is as long as the highest entry named, not the entries used: about
/// 4 bytes per table entry for the handles and 1 for each kind memo, held for the build only.
/// A table whose kind names sit at its end pays a full-length kind memo for a handful of
/// kinds.
pub(super) struct Entries<'t, T: ?Sized> {
    table: &'t T,
    handles: Memo<Interned>,
    node_kinds: Memo<NodeKind>,
    edge_kinds: Memo<EdgeKind>,
}

impl<'t, T: StringTable + ?Sized> Entries<'t, T> {
    pub(super) fn new(table: &'t T) -> Self {
        Self {
            table,
            handles: Memo(Vec::with_capacity(table.entries())),
            node_kinds: Memo(Vec::new()),
            edge_kinds: Memo(Vec::new()),
        }
    }

    /// The handle `arena.intern` gives entry `entry`'s text.
    pub(super) fn string(
        &mut self,
        arena: &mut StringArena,
        entry: u32,
    ) -> Result<Interned, ColumnsRefusal> {
        let table = self.table;
        self.handles
            .get(entry, || Ok(arena.intern(text(table, entry)?)?))
    }

    /// [`string`](Self::string) for an optional cell.
    pub(super) fn optional(
        &mut self,
        arena: &mut StringArena,
        entry: Option<u32>,
    ) -> Result<Option<Interned>, ColumnsRefusal> {
        entry.map(|entry| self.string(arena, entry)).transpose()
    }

    /// The node kind entry `entry` names, refused as node row `row`'s if it names none.
    pub(super) fn node_kind(&mut self, entry: u32, row: u32) -> Result<NodeKind, ColumnsRefusal> {
        let table = self.table;
        self.node_kinds.get(entry, || {
            NodeKind::from_name(text(table, entry)?).ok_or(ColumnsRefusal::NodeKind { row })
        })
    }

    /// The edge kind entry `entry` names, refused as edge row `row`'s if it names none.
    pub(super) fn edge_kind(&mut self, entry: u32, row: u32) -> Result<EdgeKind, ColumnsRefusal> {
        let table = self.table;
        self.edge_kinds.get(entry, || {
            EdgeKind::from_name(text(table, entry)?).ok_or(ColumnsRefusal::EdgeKind { row })
        })
    }
}

fn text<T: StringTable + ?Sized>(table: &T, entry: u32) -> Result<&str, ColumnsRefusal> {
    table
        .text(entry)
        .ok_or(ColumnsRefusal::TableEntry { entry })
}

/// One value per entry, `None` until resolved.
struct Memo<V>(Vec<Option<V>>);

impl<V: Copy> Memo<V> {
    /// The value at `entry`, resolving it on first use. A refused entry is not stored, so an
    /// entry far past the table costs no growth.
    fn get<E>(&mut self, entry: u32, resolve: impl FnOnce() -> Result<V, E>) -> Result<V, E> {
        let at = entry as usize;
        if let Some(&Some(value)) = self.0.get(at) {
            return Ok(value);
        }
        let value = resolve()?;
        if at >= self.0.len() {
            self.0.resize(at + 1, None);
        }
        self.0[at] = Some(value);
        Ok(value)
    }
}
