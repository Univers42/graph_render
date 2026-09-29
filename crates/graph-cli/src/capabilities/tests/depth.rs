//! `analysis.depth`, the one `analysis.*` row Phase 7 could not register: the row the
//! re-point onto p3's `layout/hierarchy.rs` unblocks.
//!
//! Split out of `registry.rs` for the house line limit, the same way `force.rs` and
//! `transport.rs` are split out of it.

use super::super::{Status, registry};
use crate::capabilities::registry::TOPOLOGY_CEILING;

/// Phase 7's ninth `analysis.*` row. `analysis.depth` was the one row the phase could
/// not register: it reads the root/forest convention through `depth::Roots`, and p3's
/// `layout/hierarchy.rs` — the one type in the workspace that has a repaired hierarchy
/// to read — was not on its base, so registering the row would have meant either a
/// second derivation of the convention or a claim nothing backed. p3 is on the base and
/// `impl depth::Roots for Hierarchy` exists, so the row is published.
///
/// `implemented`, not `gated`: no analysis stage is in the hash gate's stage list, so
/// there is no 4-way hash and no oracle differential standing behind the claim, and
/// `problems()` refuses a `gated` row in exactly that case. A row with no recorded
/// evidence never becomes `gated` by being written down.
#[test]
fn the_depth_row_is_published_with_every_required_field() {
    let rows = registry();
    let row = rows
        .iter()
        .find(|r| r.id == "analysis.depth")
        .unwrap_or_else(|| panic!("analysis.depth is a row"));
    assert_eq!(row.stage, "analysis");
    assert_eq!(
        row.geometry, None,
        "a depth is a labelling, not a geometry kind"
    );
    assert_eq!(
        row.status,
        Status::Implemented,
        "implemented, never gated without a recorded differential"
    );
    assert_eq!(
        (row.oracle_record, row.hash_stage),
        ("oracle-diff", "analysis"),
        "the same records its eight analysis siblings name, so no row borrows another's"
    );
    assert_eq!(row.scale_ceiling, TOPOLOGY_CEILING);
    assert!(row.scale_ceiling > 0, "a stated ceiling");
    for (field, value) in [
        ("oracle", row.oracle),
        ("complexity", row.complexity),
        ("degradation", row.degradation),
        ("ponytail", row.ponytail),
    ] {
        assert!(!value.trim().is_empty(), "analysis.depth: {field} is empty");
    }
}

/// The row is published because the re-point exists, and this is the test that says so
/// in the only way that survives a change: `bfs_depth` over p3's repaired `Hierarchy` is
/// a **type bound**, so undoing the re-point makes this module fail to compile rather
/// than leave a row whose `oracle` sentence describes a function that no longer does.
/// Nothing here is a text match on the row.
#[test]
fn the_depth_rows_oracle_is_a_call_that_typechecks_over_the_repaired_hierarchy() {
    use graph_core::analysis::depth::{self, Depth};
    use graph_core::layout::hierarchy::Hierarchy;
    use graph_core::{EdgeKind, EdgeRecord, NodeKind, NodeRecord, index_model};

    // The call the row's `oracle` names, written out: a `Hierarchy` handed straight to
    // `bfs_depth`, with no adapter type in between. Undoing the re-point makes this
    // function fail to compile, so the row cannot outlive the thing it describes.
    fn over_repaired(topology: &graph_core::Topology) -> Depth {
        let hierarchy = Hierarchy::of(topology).expect("n + 1 fits u32");
        depth::bfs_depth(&hierarchy)
    }

    let node = |id: &str| NodeRecord {
        id: id.into(),
        kind: NodeKind::Record,
        database_id: None,
        source: "pg".into(),
        label: id.into(),
        group: None,
        weight: 0.5,
        version: 0.0,
        has_note: false,
        icon: None,
    };
    let child_of = |id: &str, child: &str, parent: &str| EdgeRecord {
        id: id.into(),
        source: child.into(),
        target: parent.into(),
        kind: EdgeKind::Hierarchy,
        label: String::new(),
        strength: 0.5,
        directed: false,
        record_id: None,
        child_first: true,
    };
    let nodes = [node("a"), node("b"), node("c")];
    let edges = [child_of("ab", "a", "b"), child_of("bc", "b", "c")];
    let topology = index_model(&nodes, &edges).expect("three nodes fit");
    let d = over_repaired(&topology);
    assert_eq!(
        d.levels().len(),
        3,
        "one level per node, in dense-index order"
    );
    assert_eq!(d.levels(), [2, 1, 0], "c is the root, a the leaf below it");
    assert_eq!(d.max(), 2, "a is two levels below c");
}
