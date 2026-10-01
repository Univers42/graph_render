//! The force rows: `implemented` rather than `gated`, each naming its own oracle record.
//!
//! Split out of `registry.rs` by the house's 300-line limit.

use super::IGRAPH_LAYOUT_IDS;
use super::*;

/// The two force rows are `implemented`, and the *reason* is structural rather than
/// provisional: `Status::Gated` means a 4-way hash **and** an oracle differential
/// passed on this tree, and a force layout's differential is a margin rather than a
/// byte-equality (a force simulation amplifies a 1-ULP difference into a different
/// picture). Each names its own record, so neither can be promoted by borrowing the
/// other's evidence.
#[test]
fn a_force_row_is_implemented_and_names_its_own_oracle_record() {
    let rows = registry();
    for (id, record) in [
        ("layout.force.barnes_hut", "stress"),
        ("layout.forceatlas2", "oracle-fa2"),
        ("layout.force.fruchterman_reingold", "oracle-igraph"),
        ("layout.force.kamada_kawai", "oracle-igraph"),
        ("layout.force.drl", "oracle-igraph"),
        ("layout.force.lgl", "oracle-igraph"),
        ("layout.force.davidson_harel", "oracle-igraph"),
        ("layout.force.graphopt", "oracle-igraph"),
        ("layout.force.spring", "oracle-spring"),
        ("layout.circular.hierarchy", "oracle-circular-hierarchy"),
    ] {
        let row = rows.iter().find(|r| r.id == id).expect("registered");
        assert_eq!(row.status, Status::Implemented, "{id}");
        assert_eq!(row.oracle_record, record, "{id}");
        assert_eq!(row.hash_stage, id, "{id}: its hash stage is its own id");
        assert_eq!(row.stage, "layout");
        assert_eq!(row.geometry, Some("Point"), "{id}");
        assert!(row.scale_ceiling > 0, "{id}");
    }
    // None may borrow a record that does not speak for it: the d3-force stress arm knows
    // nothing of networkx's FA2 nor of igraph's six, and the other way round.
    let other = |id: &str, record: &str| {
        assert!(
            !rows.iter().any(|r| r.id == id && r.oracle_record == record),
            "{id} must not be held to {record}"
        );
    };
    other("layout.forceatlas2", "stress");
    other("layout.force.barnes_hut", "oracle-fa2");
    for id in IGRAPH_LAYOUT_IDS {
        other(id, "stress");
        other(id, "oracle-fa2");
    }
    // The two FR ports share a metric and share no code, and the SciGraphs closed form is
    // compared by a different arm entirely: nobody may be measured by another's run.
    other("layout.force.spring", "oracle-fa2");
    other("layout.force.spring", "stress");
    other("layout.circular.hierarchy", "oracle-closed-form");
    other("layout.circular.hierarchy", "oracle-spring");
}

/// The six igraph rows and the filter that builds them are one list. `force_record` names
/// the layouts it hands `oracle-igraph`; if a layout were in one and not the other it would
/// fall through to the `roundtrip`/`Gated` arm and the ledger would claim a gate no
/// differential of its own can earn.
#[test]
fn the_igraph_rows_are_exactly_the_layouts_the_row_builder_filters() {
    let rows = registry();
    for id in IGRAPH_LAYOUT_IDS {
        let row = rows.iter().find(|r| r.id == id).expect("registered");
        assert_eq!(row.oracle_record, "oracle-igraph", "{id}");
    }
}
