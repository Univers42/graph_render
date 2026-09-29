//! The four Phase 3 stage ids, read off the registry rather than off a copy of it. Split
//! from `knob.rs` by the house's 300-line limit, and because this is the only test in
//! this module about a *name*: everything beside it is about a control moving a stage's
//! bytes, and a name is the one thing here that can be right while every byte is wrong.
//!
//! Each row is fetched by the id it is asked about, never by its position in
//! `graph_core::registry::LAYOUTS`: that array promises only the order the gate runs the
//! stages in, so an index is not a thing this test may lean on — a layout registered or
//! reordered elsewhere would redden the test below with neither id moved.

use super::*;
use graph_core::layout::{circle_packing, circular, tidy_tree, treemap};

/// The four Phase 3 stage ids are the registry's own: a constant here that drifted from
/// `graph_core::registry::LAYOUTS` would file a control under a stage the gate does not
/// hash, and it would then move nothing at all.
#[test]
fn the_p3_stage_ids_are_the_registry_s_own() {
    for id in [TIDY_TREE, TREEMAP, CIRCULAR, PACKING] {
        assert!(
            graph_core::registry::find(id).is_some(),
            "{id} is registered"
        );
    }
    // Stronger than `find(id).is_some()`: the constant must be *that row's* id, and the
    // row it names must be the one running that layout module's own `run`. Each line
    // catches one thing, and they are not the same thing. The id line catches the
    // re-export being *repointed* — `TIDY_TREE` handed another registered layout's `ID`
    // — but not that `ID`'s own value changing, because the re-export moves with it and
    // the line degrades to `tidy_tree::ID == tidy_tree::ID`, which cannot fail. The `run`
    // line is the one that survives that: the row is found by the id it carries, so a row
    // carrying this module's id while running another layout's `run` fails here alone.
    for (id, run) in [
        (TIDY_TREE, tidy_tree::run as fn(&_) -> _),
        (TREEMAP, treemap::run as fn(&_) -> _),
        (CIRCULAR, circular::run as fn(&_) -> _),
        (PACKING, circle_packing::run as fn(&_) -> _),
    ] {
        let row = graph_core::registry::find(id).expect("registered");
        assert_eq!(row.id, id, "{id} is the row the registry gives it");
        assert!(
            std::ptr::fn_addr_eq(row.run, run),
            "{id} is registered with another layout's run"
        );
    }
    assert_eq!(
        stage_bytes(P3_SEED, &honest())
            .expect("runs")
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>(),
        stages(),
        "the ids the knobs name are the ids the gate hashes, in the same order"
    );
}
