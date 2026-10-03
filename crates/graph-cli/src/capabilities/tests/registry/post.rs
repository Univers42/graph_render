//! The POST rows: bundling and style beside routing, each restating its module's metadata.
//!
//! Split out of `registry.rs` by the house's 300-line limit.

use super::*;

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
        EdgeGeometryKind::Curve => "Curve",
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
