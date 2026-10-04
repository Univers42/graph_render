//! The one refusal table, walked once for both appends, and the count refusals that have no
//! table to walk: a batch whose load a graph at the limit cannot take is refused before any
//! row is read, so no topology exists to walk.
//!
//! Six of the eight rows are the same batch refused the same way by `extend` and by
//! `extend_columns`. The other two are [`columns::Damage`] edits, because
//! [`BatchRefusal`]'s `NodeKind`, `EdgeKind` and `TableEntry` variants name a cell no record
//! type can carry — see the enum for the whole argument.

use super::columns::Doc;
use super::*;
use crate::index::extend::columns::BatchRefusal;

/// What a refusal row is judged to refuse with, on each path.
enum Want {
    /// Both paths refuse this batch, with the same [`ExtendError`].
    Both(ExtendError),
    /// Only `extend_columns` reaches this one, once the damage is applied to the batch.
    Columns(columns::Damage, BatchRefusal),
}

/// The eight refusals, in the order the validate passes find them: the five the record path
/// gives, then the three the columnar path adds because a batch's cells are entries rather
/// than enum-typed fields.
fn refusals() -> [(&'static str, Batch, Want); 8] {
    use ExtendError::{EdgeId, Endpoint, NodeId};
    let c = || node("c", "db");
    let one_edge = || (vec![c()], vec![edge("e2", "c", "a")]);
    [
        (
            "node id in the graph",
            (vec![c(), node("a", "")], vec![]),
            Want::Both(NodeId { index: 1 }),
        ),
        (
            "node id twice in the batch",
            (vec![c(), c()], vec![]),
            Want::Both(NodeId { index: 1 }),
        ),
        (
            "edge id in the graph",
            (vec![c()], vec![edge("e2", "a", "c"), edge("e1", "c", "b")]),
            Want::Both(EdgeId { index: 1 }),
        ),
        (
            "edge id twice in the batch",
            (vec![], vec![edge("e2", "a", "b"), edge("e2", "b", "a")]),
            Want::Both(EdgeId { index: 1 }),
        ),
        (
            "endpoint in neither",
            (
                vec![c()],
                vec![edge("e2", "c", "a"), edge("e3", "c", "ghost")],
            ),
            Want::Both(Endpoint { index: 1 }),
        ),
        (
            "node kind names no node kind",
            (vec![c()], vec![]),
            Want::Columns(
                columns::Damage::NodeKind(0),
                BatchRefusal::NodeKind { index: 0 },
            ),
        ),
        (
            "edge kind names no edge kind",
            one_edge(),
            Want::Columns(
                columns::Damage::EdgeKind(0),
                BatchRefusal::EdgeKind { index: 0 },
            ),
        ),
        (
            "an endpoint entry past the table",
            one_edge(),
            Want::Columns(
                columns::Damage::SourceEntry {
                    index: 0,
                    entry: u32::MAX,
                },
                BatchRefusal::TableEntry { entry: u32::MAX },
            ),
        ),
    ]
}

/// Every row refused leaves the topology byte-identical: the same stage bytes, the same arena
/// length, the same graph a rebuild over the accepted batches would give — so a refused batch
/// claimed no id and no arena slot, on the record path and the columnar one alike.
#[test]
fn extend_refusal_leaves_topology_unchanged() {
    let first: Batch = (
        vec![node("a", "db"), node("b", "")],
        vec![edge("e1", "a", "b")],
    );
    let base = index_model(&first.0, &first.1).expect("fits");
    let next: Batch = (
        vec![node("c", "db")],
        vec![edge("e2", "c", "a"), edge("e3", "c", "c")],
    );
    for (name, (nodes, edges), want) in refusals() {
        let (want, doc) = match want {
            Want::Both(refusal) => {
                let mut t = base.clone();
                assert_eq!(t.extend(&nodes, &edges), Err(refusal), "{name}");
                assert_eq!(bytes(&t), bytes(&base), "{name}: the bytes");
                assert_eq!(t.strings().len(), base.strings().len(), "{name}: the arena");
                t.extend(&next.0, &next.1).expect("the next valid batch");
                assert_matches_rebuild(&t, &[first.clone(), next.clone()], name);
                (BatchRefusal::Extend(refusal), Doc::of(&nodes, &edges))
            }
            Want::Columns(damage, refusal) => {
                let mut doc = Doc::of(&nodes, &edges);
                doc.damage(damage);
                (refusal, doc)
            }
        };
        let mut columns = base.clone();
        assert_eq!(
            doc.append(&mut columns),
            Err(want),
            "{name}: the columns path"
        );
        assert_eq!(bytes(&columns), bytes(&base), "{name}: the columns bytes");
        assert_eq!(
            columns.strings().len(),
            base.strings().len(),
            "{name}: the columns arena"
        );
        Doc::of(&next.0, &next.1)
            .append(&mut columns)
            .expect("the next valid batch, in columns");
        assert_matches_rebuild(&columns, &[first.clone(), next.clone()], name);
    }
}

/// The columns twin of `a_batch_that_could_overflow_a_count_is_refused_before_anything_is_
/// counted_in`. The refusal is the same four, and the batch that trips them is the same
/// sum over strings — six per node (id, database, source, label, group, icon) and three per
/// edge (id, label, record id), because a kind name is resolved to a `NodeKind` and never
/// interned, exactly as over records.
#[test]
fn a_columns_batch_that_could_overflow_a_count_is_refused_before_anything_is_counted_in() {
    let mut full = Doc::default();
    full.node("abc");
    assert_eq!(
        (full.load().strings, full.load().bytes, full.load().nodes),
        (6, 3 + 2 + 2 + 1 + 1 + 1, 1),
        "every optional present: six strings"
    );
    let mut bare = Doc::default();
    bare.bare_node("abc");
    assert_eq!(
        (bare.load().strings, bare.load().bytes),
        (3, 3 + 2 + 1),
        "every optional absent: three strings"
    );
    assert_capacity_refusals(full.load());
    // The same batch, appended for real. Caveat: only to a graph with room for it — `Load::of`
    // reads a live topology's own counts, and every limit `extend_columns` checks is the `u32`
    // index space, which no test-sized graph reaches. So this pins that the refusals below are
    // a function of what the graph holds and not of the batch.
    let mut t = empty_model();
    assert_eq!(full.append(&mut t), Ok(()), "a graph with room takes it");
    assert_eq!(t.node_count(), 1);
}

/// The four refusals a graph holding all but the room for one more of each gives, over the
/// load the row above built. Caveat: `held` is a count, not a graph — the append cannot be
/// driven to a `Capacity` its own `Load::of` did not already clear, so this is the only place
/// the arithmetic is pinned.
fn assert_capacity_refusals(batch: Load) {
    let max = u64::from(u32::MAX);
    let held = Load {
        strings: max - 5,
        bytes: 10,
        nodes: 5,
        edges: ADJACENCY_LIMIT - 1,
    };
    let refused = |add: Load| held.check(add).err();
    assert_eq!(
        refused(batch),
        Some(ExtendError::Capacity {
            what: "string arena"
        })
    );
    let nodes = Load {
        nodes: max - 1,
        ..Load::default()
    };
    assert_eq!(
        refused(nodes),
        Some(ExtendError::Capacity { what: "node index" })
    );
    let edges = Load {
        edges: 2,
        ..Load::default()
    };
    assert_eq!(
        refused(edges),
        Some(ExtendError::Capacity { what: "adjacency" })
    );
}