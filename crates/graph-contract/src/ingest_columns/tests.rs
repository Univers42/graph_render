use super::*;

mod cells;
mod encode;

pub(super) use cells::{EdgeCells, Marks, NodeCells};
pub(super) use encode::{Doc, Encoded};

/// `u32::MAX` in an optional column: the encoder's "absent".
const ABSENT: u32 = u32::MAX;

#[cfg(test)]
mod refusals;

#[cfg(test)]
mod shape;

/// A document with everything the contract can carry on one edge: both nodes fully
/// populated, one node with every optional absent, one `child_first` edge, a `-0.0` weight
/// and a subnormal version. Every happy-path and refusal test starts from it so a change to
/// the encoder moves all of them together.
pub(super) fn full() -> Doc {
    let mut doc = Doc::new();
    let (n0, n1, db, src, lab, grp, icon, rec, note, rel) = (
        doc.s("n-0"),
        doc.s("n-1"),
        doc.s("db-0"),
        doc.s("studio"),
        doc.s("Graph notes"),
        doc.s("Epsilon"),
        doc.s("\u{1f33f}"),
        doc.s("record"),
        doc.s("note"),
        doc.s("relation"),
    );
    let e0 = doc.s("e-0");
    let label = doc.s("relation");
    let record = doc.s("rec-7");
    let n = doc.node();
    *n = NodeCells {
        weight: -0.0,
        version: f64::from_bits(1),
        id: n0,
        kind: rec,
        database: db,
        source: src,
        label: lab,
        group: grp,
        icon,
        has_note: 1,
    };
    let n = doc.node();
    *n = NodeCells {
        weight: 1.0,
        version: 2.0,
        id: n1,
        kind: note,
        database: ABSENT,
        source: src,
        label: lab,
        group: ABSENT,
        icon: ABSENT,
        has_note: 0,
    };
    let e = doc.edge();
    *e = EdgeCells {
        strength: 0.5,
        id: e0,
        source: 0,
        target: 1,
        kind: rel,
        label,
        record_id: record,
        directed: 1,
        child_first: 1,
    };
    doc
}
