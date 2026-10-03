//! The registry's own shape: the ledger row's serialised form (`prompt.md` §8) and the
//! metadata each row must carry. Split out of the parent test module to keep every file
//! under the house line limit; the id-to-record routing lives in [`routing`].

use super::*;
mod force;
mod ids;
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

/// Phase 8's bundling and style rows. `post.route.grid` was the only POST row the ledger
/// carried, so `post.bundle.fdeb`, `post.bundle.mingle` and the four `post.style.*` rows
/// had no row at all — each a registered, unit-tested capability with no statement of what
/// it costs or what it owes, which is what `prompt.md` §8 asks a ledger to have.
#[test]
fn every_post_capability_is_a_row_beside_the_routing_one() {
    let rows = registry();
    let post: Vec<&Capability> = rows.iter().filter(|r| r.stage == "post").collect();
    let mut ids: Vec<&str> = post.iter().map(|r| r.id).collect();
    let mut want = vec![
        "post.route.grid",
        "post.bundle.fdeb",
        "post.bundle.mingle",
        "post.separate.grid",
        "post.style.straight",
        "post.style.orthogonal",
        "post.style.quadratic",
        "post.style.bezier",
    ];
    // The style rows follow `post::styles::STYLES`'s own order, which is the order that
    // module documents ("the order the ledger lists them"), so compare as sets and pin
    // the count: a duplicate id is caught by the registry's own uniqueness test.
    ids.sort_unstable();
    want.sort_unstable();
    assert_eq!(
        ids, want,
        "one row per registered POST capability: none missing, none invented"
    );
    assert_eq!(post.len(), 8, "eight POST rows, not a duplicate of one");
    for row in post {
        assert_eq!(
            row.status,
            Status::Implemented,
            "{}: implemented, never gated without a recorded differential",
            row.id
        );
        assert!(row.geometry.is_some(), "{}: names its geometry", row.id);
        assert!(row.scale_ceiling > 0, "{}: a stated ceiling", row.id);
        for (field, value) in [
            ("oracle", row.oracle),
            ("complexity", row.complexity),
            ("degradation", row.degradation),
            ("ponytail", row.ponytail),
        ] {
            assert!(!value.trim().is_empty(), "{}: {field} is empty", row.id);
        }
    }
}

/// Each POST row restates the metadata its own module in `graph-core` declares, rather
/// than a second and looser answer: the ceiling, oracle, complexity, degradation and
/// Ponytail are read across, so a change to the module's `META` and a stale ledger row
/// cannot disagree.
#[test]
fn a_post_row_carries_the_metadata_its_own_module_declares() {
    use graph_core::post::{fdeb, mingle, styles};
    let rows = registry();
    let of = |id: &str| {
        rows.iter()
            .find(|r| r.id == id)
            .unwrap_or_else(|| panic!("{id} is a row"))
    };
    for (id, meta) in [(fdeb::ID, &fdeb::META), (mingle::ID, &mingle::META)] {
        let row = of(id);
        assert_eq!(
            (
                row.scale_ceiling,
                row.oracle,
                row.complexity,
                row.degradation,
                row.ponytail
            ),
            (
                meta.scale_ceiling,
                meta.oracle,
                meta.complexity,
                meta.degradation,
                meta.ponytail
            ),
            "{id}"
        );
    }
    for style in styles::STYLES {
        let row = of(style.id);
        assert_eq!(
            (
                row.scale_ceiling,
                row.oracle,
                row.complexity,
                row.degradation,
                row.ponytail
            ),
            (
                style.meta.scale_ceiling,
                style.meta.oracle,
                style.meta.complexity,
                style.meta.degradation,
                style.meta.ponytail
            ),
            "{}",
            style.id
        );
    }
}

/// The style rows differ in exactly one field, `edges`, and the ledger says so: a style
/// never emits two geometry kinds, so each row names one, and no row may name another's.
#[test]
fn each_style_row_names_the_geometry_kind_its_own_style_emits() {
    use graph_contract::geometry::EdgeGeometryKind;
    let name = |kind: EdgeGeometryKind| match kind {
        EdgeGeometryKind::Line => "Line",
        EdgeGeometryKind::Polyline => "Polyline",
        _ => "Curve",
    };
    let rows = registry();
    for style in graph_core::post::styles::STYLES {
        let row = rows
            .iter()
            .find(|r| r.id == style.id)
            .unwrap_or_else(|| panic!("{} is a row", style.id));
        assert_eq!(row.geometry, Some(name(style.meta.edges)), "{}", style.id);
    }
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
