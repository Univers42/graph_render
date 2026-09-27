//! Structure-of-arrays attribute columns, indexed by dense index.
//!
//! `weight`, `version` and `strength` stay `f64`: they are JS `number`s that the oracle
//! compares with `===` (`nodesEqual`, `edgesEqual`), so narrowing them here would change
//! which nodes a diff reports. They narrow to `f32` at the wire, not before.

use crate::arena::Interned;
use crate::edgekind::EdgeKind;

/// What a node is (`types.ts:20`). The discriminant is the column byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum NodeKind {
    /// A data record.
    Record = 0,
    /// A free or overlay note.
    Note = 1,
    /// A database or collection.
    Database = 2,
    /// A synthetic tag hub.
    Tag = 3,
}

impl NodeKind {
    /// Every kind, in discriminant order.
    pub const ALL: [Self; 4] = [Self::Record, Self::Note, Self::Database, Self::Tag];

    /// The oracle's string for this kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Record => "record",
            Self::Note => "note",
            Self::Database => "database",
            Self::Tag => "tag",
        }
    }

    /// The kind whose [`as_str`](Self::as_str) is `name`, if any.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == name)
    }
}

/// Node attributes, one column each, all of length `n`.
#[derive(Debug, Clone, Default)]
pub struct NodeColumns {
    /// Stable id.
    pub id: Vec<Interned>,
    /// Kind.
    pub kind: Vec<NodeKind>,
    /// Source database.
    pub database: Vec<Option<Interned>>,
    /// Owning backend.
    pub source: Vec<Interned>,
    /// Label.
    pub label: Vec<Interned>,
    /// Secondary label (`GraphNode.group`, a string).
    pub group_label: Vec<Option<Interned>>,
    /// Visual weight.
    pub weight: Vec<f64>,
    /// Version.
    pub version: Vec<f64>,
    /// Note overlay flag.
    pub has_note: Vec<bool>,
    /// Icon.
    pub icon: Vec<Option<Interned>>,
    /// Cluster group: first-seen index of `source` (`layoutBridge.ts:77-87`), as `u32` —
    /// the oracle stores it in a `Uint8Array` and aliases group 256 onto group 0 (H9,
    /// `docs/decisions/h9-group-width.md`).
    pub group: Vec<u32>,
    /// Incident edge count — the length of the oracle's `adjacency` list.
    pub degree: Vec<u32>,
}

impl NodeColumns {
    /// Empty columns with room for `n` nodes.
    pub fn with_capacity(n: usize) -> Self {
        Self {
            id: Vec::with_capacity(n),
            kind: Vec::with_capacity(n),
            database: Vec::with_capacity(n),
            source: Vec::with_capacity(n),
            label: Vec::with_capacity(n),
            group_label: Vec::with_capacity(n),
            weight: Vec::with_capacity(n),
            version: Vec::with_capacity(n),
            has_note: Vec::with_capacity(n),
            icon: Vec::with_capacity(n),
            group: Vec::with_capacity(n),
            degree: Vec::with_capacity(n),
        }
    }

    /// Bytes held by the columns' elements (not their spare capacity).
    pub fn byte_len(&self) -> usize {
        let n = self.id.len();
        n * (6 * size_of::<Interned>() + size_of::<NodeKind>() + 2 * size_of::<f64>())
            + n * (size_of::<bool>() + 2 * size_of::<u32>())
    }
}

/// Edge attributes, one column each, all of length `m`. Endpoints are dense node indices.
#[derive(Debug, Clone, Default)]
pub struct EdgeColumns {
    /// Content-addressed id.
    pub id: Vec<Interned>,
    /// Source node, dense index.
    pub source: Vec<u32>,
    /// Target node, dense index.
    pub target: Vec<u32>,
    /// Kind.
    pub kind: Vec<EdgeKind>,
    /// Label.
    pub label: Vec<Interned>,
    /// Strength.
    pub strength: Vec<f64>,
    /// Directed flag.
    pub directed: Vec<bool>,
    /// Backing row id.
    pub record_id: Vec<Option<Interned>>,
}

impl EdgeColumns {
    /// Empty columns with room for `m` edges.
    pub fn with_capacity(m: usize) -> Self {
        Self {
            id: Vec::with_capacity(m),
            source: Vec::with_capacity(m),
            target: Vec::with_capacity(m),
            kind: Vec::with_capacity(m),
            label: Vec::with_capacity(m),
            strength: Vec::with_capacity(m),
            directed: Vec::with_capacity(m),
            record_id: Vec::with_capacity(m),
        }
    }

    /// Bytes held by the columns' elements (not their spare capacity).
    pub fn byte_len(&self) -> usize {
        let m = self.id.len();
        m * (3 * size_of::<Interned>() + 2 * size_of::<u32>() + size_of::<EdgeKind>())
            + m * (size_of::<f64>() + size_of::<bool>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_kind_names_round_trip() {
        for kind in NodeKind::ALL {
            assert_eq!(NodeKind::from_name(kind.as_str()), Some(kind));
        }
        assert_eq!(NodeKind::from_name("Record"), None);
        assert_eq!(
            NodeKind::ALL.map(|k| k as u8),
            [0, 1, 2, 3],
            "the column byte"
        );
    }

    /// Pre-sized, never grown by one in the build loop (`dsa-and-memory.md`).
    #[test]
    fn with_capacity_reserves_room_in_every_column() {
        let n = NodeColumns::with_capacity(9);
        let caps = [
            n.id.capacity(),
            n.kind.capacity(),
            n.database.capacity(),
            n.source.capacity(),
            n.label.capacity(),
            n.group_label.capacity(),
            n.weight.capacity(),
            n.version.capacity(),
            n.has_note.capacity(),
            n.icon.capacity(),
            n.group.capacity(),
            n.degree.capacity(),
        ];
        assert!(caps.iter().all(|&c| c >= 9), "{caps:?}");
        let e = EdgeColumns::with_capacity(9);
        let caps = [
            e.id.capacity(),
            e.source.capacity(),
            e.target.capacity(),
            e.kind.capacity(),
            e.label.capacity(),
            e.strength.capacity(),
            e.directed.capacity(),
            e.record_id.capacity(),
        ];
        assert!(caps.iter().all(|&c| c >= 9), "{caps:?}");
    }

    #[test]
    fn byte_len_counts_one_element_of_every_column() {
        let mut nodes = NodeColumns::with_capacity(4);
        assert_eq!(nodes.byte_len(), 0);
        nodes.id.push(
            crate::arena::StringArena::default()
                .intern("a")
                .expect("fits"),
        );
        // 6 handles × 4 + kind 1 + weight/version 2 × 8 + has_note 1 + group/degree 2 × 4.
        assert_eq!(nodes.byte_len(), 24 + 1 + 16 + 1 + 8);
        let mut edges = EdgeColumns::with_capacity(4);
        edges.id.push(nodes.id[0]);
        // 3 handles × 4 + source/target 2 × 4 + kind 1 + strength 8 + directed 1.
        assert_eq!(edges.byte_len(), 12 + 8 + 1 + 8 + 1);
    }
}
