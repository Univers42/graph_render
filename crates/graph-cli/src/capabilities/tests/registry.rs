//! The registry's own shape: the ledger row's serialised form (`prompt.md` §8) and the
//! metadata each row must carry. Split out of the parent test module to keep every file
//! under the house line limit; the id-to-record routing lives in [`routing`].

use super::*;
mod force;
mod ids;
mod post;
mod routing;
// The children reach these three through `super::`, so they are re-exported here: a glob
// through a private module does not carry the parent's own private items, and these are
// three of them. `force.rs` and `routing.rs` are the two consumers.
pub(super) use ids::IGRAPH as IGRAPH_LAYOUT_IDS;

/// The record a row names, as `(oracle_record, hash_stage)`. Each row's two names must be
/// a record `graph-cli` actually writes — a name nothing writes is a row that can never
/// be backed, however often the gate is re-run.
pub(super) fn records_of(row: &Capability) -> (&'static str, &'static str) {
    (row.oracle_record, row.hash_stage)
}

/// **One row source.** `registry::registry()` is the only place a family enters the
/// ledger, so the list `--check` iterates and the list that is published are the same list
/// by construction. It used to be half of one: `capabilities::registry()` chained four
/// families and then `.extend`ed three more, which left an `analysis.*` row reachable by
/// `--check` yet absent from `registry()` — and nothing asserted that any family was
/// reached at all.
#[test]
fn every_row_family_is_reached_by_the_one_registry() {
    use super::super::registry;
    let from_registry: Vec<&str> = registry::registry().iter().map(|r| r.id).collect();
    for family in ["analysis.components.weak", "scale.lod", "ingest.build"] {
        assert!(
            from_registry.contains(&family),
            "{family} is published by `registry()` alone, not by a second `.extend`"
        );
    }
    let published: Vec<&str> = registry().iter().map(|r| r.id).collect();
    assert_eq!(
        from_registry, published,
        "and the published list is that one list, in that one order"
    );
    for stage in [
        "topology",
        "layout",
        "transport",
        "post",
        "analysis",
        "scale",
        "ingest",
    ] {
        assert!(
            published.iter().any(|id| id.starts_with(stage)),
            "{stage} has no row in the one registry"
        );
    }
}

/// A layout id no arm names — a newly registered one — is `implemented` and names a
/// record no gate writes. It used to be the `else` of the chain in `layout_row`, which
/// stamped it `gated` on `roundtrip` and inherited a `gated` claim with no record of its
/// own.
#[test]
fn a_layout_id_no_arm_names_is_implemented_and_names_no_record() {
    use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};
    let fresh = Box::leak(Box::new(graph_core::registry::Capability {
        id: "layout.registered.but.unproven",
        run: |_| unreachable!("a test never runs a capability"),
        meta: graph_core::registry::Metadata {
            tier: 1,
            stage: "layout",
            nodes: NodeGeometryKind::Point,
            edges: EdgeGeometryKind::Line,
            oracle: "none yet",
            complexity: "O(n)",
            scale_ceiling: 1_000,
            degradation: "none",
            ponytail: "none owed",
        },
    }));
    let row = super::super::registry::layout_row::layout(fresh);
    assert_eq!(row.status, Status::Implemented, "not gated by inheritance");
    assert_eq!(row.oracle_record, "unproven", "and naming no record at all");
    assert!(
        !super::super::registry::layout_row::is_declared(row.id),
        "which is exactly the state this row is in"
    );
}

/// Every layout graph-core registers is named by exactly one arm here, so none of them
/// can reach the fail-closed arm above by accident, and a layout added to `LAYOUTS`
/// without an arm fails here rather than shipping as a silently unimplemented row.
#[test]
fn every_registered_layout_is_named_by_one_of_the_arms() {
    use super::super::registry::layout_row::is_declared;
    for layout in graph_core::registry::LAYOUTS {
        assert!(is_declared(layout.id), "{}: no arm names it", layout.id);
    }
    assert!(
        !is_declared("layout.registered.but.unproven"),
        "the control: an id no arm names is not declared"
    );
}

/// All four ingest rows name the **one** hash-gate stage this file declares, and no other
/// row claims it. The stage used to be the string `ingest.build` written four times, so
/// `adapter.rows`, `adapter.notion` and `ingest.roles` each claimed a stage belonging to
/// `ingest.build` with nothing saying so; a shared stage is now one named constant with the
/// declaration beside it, and this holds the rows to it.
#[test]
fn every_ingest_row_names_the_one_stage_this_file_declares() {
    use super::super::ingest::INGEST_STAGE;
    assert_eq!(INGEST_STAGE, "ingest.build");
    let rows = registry();
    let ingest: Vec<&Capability> = rows.iter().filter(|r| r.stage == "ingest").collect();
    assert_eq!(ingest.len(), 4, "the four rows of ingest.rs");
    for row in &ingest {
        assert_eq!(row.hash_stage, INGEST_STAGE, "{}", row.id);
    }
    let others: Vec<&str> = rows
        .iter()
        .filter(|row| row.hash_stage == INGEST_STAGE && row.stage != "ingest")
        .map(|row| row.id)
        .collect();
    assert!(
        others.is_empty(),
        "and no other family claims it: {others:?}"
    );
}

/// Each bundle row takes its **id from the module whose `META` it projects**. The two were
/// zipped positionally, so reordering the pair in graph-core would have attached one
/// bundle's ceiling, oracle and Ponytail to the other's stable row id with no compile error
/// anywhere.
#[test]
fn each_bundle_row_is_named_by_the_module_whose_metadata_it_carries() {
    use graph_core::post::{fdeb, mingle};
    let rows = registry();
    for (id, meta) in [(fdeb::ID, &fdeb::META), (mingle::ID, &mingle::META)] {
        let row = rows
            .iter()
            .find(|r| r.id == id)
            .unwrap_or_else(|| panic!("{id} is a row"));
        assert_eq!(
            (row.scale_ceiling, row.ponytail),
            (meta.scale_ceiling, meta.ponytail),
            "{id} carries {id}'s own metadata, by its own module's id"
        );
    }
    assert_eq!(fdeb::ID, "post.bundle.fdeb");
    assert_eq!(mingle::ID, "post.bundle.mingle");
}

#[test]
fn a_row_serialises_to_the_section_8_keys_in_order() {
    let text = serde_json::to_string(&row(Status::Gated)).expect("serialisable");
    let keys = [
        "id",
        "tier",
        "stage",
        "geometry",
        "status",
        "oracle",
        "oracle_diff",
        "hash_4way",
        "scale_ceiling",
        "degradation",
        "ponytail",
        "complexity",
    ];
    let mut at = 0;
    for key in keys {
        let found = text[at..]
            .find(&format!("\"{key}\":"))
            .unwrap_or_else(|| panic!("{key}"));
        at += found;
    }
    assert!(
        !text.contains("functions")
            && !text.contains("hash_stage")
            && !text.contains("oracle_record"),
        "{text}"
    );
}

#[test]
fn status_serialises_to_the_four_ledger_words() {
    let words = [
        Status::Absent,
        Status::Stub,
        Status::Implemented,
        Status::Gated,
    ]
    .map(|s| serde_json::to_string(&s).expect("serialisable"));
    assert_eq!(
        words,
        ["\"absent\"", "\"stub\"", "\"implemented\"", "\"gated\""]
    );
}
