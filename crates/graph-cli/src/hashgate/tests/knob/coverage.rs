//! Every `registry::LAYOUTS` id has a per-stage negative control, or is on an allow list
//! that says why not.
//!
//! **This test exists because the knob tables are hand-maintained and nothing failed when a
//! layout was added to the registry without one.** `layout.basic3d.spiral` and
//! `layout.bipartite_3d` were both hashed by the gate and both had no control, so the gate
//! could report `4-way equal` for a stage whose bytes had gone constant or empty — "equal"
//! only ever compares four arms with each other. `AGENTS.md` asks for a negative control on
//! every gate row; this is the test that asks the same question of the whole table.
//!
//! The allow list below is **measured, not guessed**: every entry names an id the gate
//! genuinely does not tabulate a per-stage control for, and each carries the reason. A new
//! layout with no control fails this test naming itself, which is the point.
//!
//! Split from `ids.rs` by the house's 300-line limit.

use crate::hashgate::knobs;

/// Layout ids the gate hashes but tables **no** per-stage control for, each with the reason
/// it is exempt. Add an entry here only with a reason that would still be true next year.
const NO_PER_STAGE_CONTROL: [(&str, &str); 0] = [];

/// Every registered layout id is either tabulated by a per-stage control or allow-listed.
#[test]
fn every_registered_layout_has_a_per_stage_control_or_a_reason_it_does_not() {
    let mut missing: Vec<&str> = graph_core::registry::LAYOUTS
        .iter()
        .map(|layout| layout.id)
        .filter(|id| {
            !knobs::all()
                .iter()
                .any(|stage| stage.id == *id && stage.env.starts_with("GM_MUTATE_"))
        })
        .filter(|id| !NO_PER_STAGE_CONTROL.iter().any(|(listed, _)| listed == id))
        .collect();
    missing.sort_unstable();
    assert!(
        missing.is_empty(),
        "{} registered layout(s) have no per-stage negative control and no allow-list \
         entry: {missing:?}. A layout the gate hashes but cannot move is a stage whose \
         bytes could go constant and still read `4-way equal`. Either add it to \
         hashgate::knobs (THREE_D_LAYOUT_STAGES or the family table beside it), or add it to \
         NO_PER_STAGE_CONTROL above with a one-line reason.",
        missing.len()
    );
}

/// The allow list is not a place ids go to be forgotten: it must not name an id the gate
/// *does* tabulate, because that would hide a knob behind an excuse.
#[test]
fn the_allow_list_names_no_layout_that_a_control_already_covers() {
    for (id, reason) in NO_PER_STAGE_CONTROL {
        assert!(
            !reason.trim().is_empty(),
            "{id} is allow-listed with an empty reason"
        );
        assert!(
            !knobs::all()
                .iter()
                .any(|stage| stage.id == id && stage.env.starts_with("GM_MUTATE_")),
            "{id} is allow-listed but hashgate::knobs already tables a control for it"
        );
    }
}