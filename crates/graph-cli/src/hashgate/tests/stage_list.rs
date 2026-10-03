//! The gate's stage list as a *list*: no id twice across the whole of it, and one
//! derivation from the three registries. Split from the parent test module by the house's
//! 300-line limit.
//!
//! The claim is that `stages()` and the check the native arm makes are **one derivation**.
//! They were two: the list was walked from `LAYOUTS` while the check deduped only the
//! layout slice, so an id that both a layout registry and a graph-wasm registry carried
//! produced two stages printed under one name — and `staged::position` resolves the first,
//! so one of the two was hashed and never compared while the report counted it equal.

use super::super::staged;
use super::super::stages::stages;
use super::super::{LAYOUT, TRANSPORT, stage_bytes};
use super::honest;
use graph_core::registry::{self as core, Capability};
use std::collections::BTreeSet;

/// Every id in the gate's list appears exactly once, and the list is the three registries
/// concatenated in the order both arms print.
///
/// Two claims in one test because they fail the same way: a second derivation of the list
/// would be free to disagree with the first about both its contents and its order.
#[test]
fn every_stage_id_appears_once_in_the_gate_list() {
    let ids = stages();
    let unique: BTreeSet<&str> = ids.iter().copied().collect();
    assert_eq!(
        unique.len(),
        ids.len(),
        "an id twice in {ids:?} folds into one record key and makes the per-stage tally lie"
    );
    let mut want = vec!["topology"];
    want.extend(core::LAYOUTS.iter().map(|layout| layout.id));
    want.extend(staged::analyses());
    want.extend(staged::posts());
    want.push(TRANSPORT);
    assert_eq!(
        ids, want,
        "the list is the registries, in the order the arms print it"
    );
    assert_eq!(
        stage_bytes(4, &honest()).expect("runs").len(),
        ids.len(),
        "one native producer per stage the gate asks for"
    );
}

/// A registry slice that names a stage already in the list is refused, not printed twice.
///
/// **The negative control for the fix, and it is the half the old check missed.** The
/// repeated-id refusal was written over the layout slice alone, so this layout — carrying
/// the id of a *graph-wasm* analysis stage, which is what a real collision looks like since
/// the two registries grew independently — sailed through and hashed twice under one name.
#[test]
fn a_layout_id_that_collides_with_a_graph_wasm_stage_is_refused() {
    let grid = *graph_core::registry::find(LAYOUT).expect("the grid is registered");
    let collide = staged::analyses()[0];
    let err = stage_bytes_for_ids(
        &[
            grid,
            Capability {
                id: collide,
                ..grid
            },
        ],
        collide,
    );
    assert!(
        err.contains("twice") && err.contains(collide),
        "the refusal must name the id and say it repeats: {err}"
    );
}

/// And the half that already worked, pinned so the repair above cannot be made by widening
/// the refusal back to the slice: a plain repeat inside the layout registry.
#[test]
fn a_layout_registry_with_a_repeated_id_is_still_refused() {
    let grid = *graph_core::registry::find(LAYOUT).expect("the grid is registered");
    let second = Capability {
        id: "layout.grid.second",
        ..grid
    };
    let err = stage_bytes_for_ids(&[grid, second, second], second.id);
    assert!(err.contains("twice"), "{err}");
}

/// The refusal, or a panic naming the ids that should have been refused.
fn stage_bytes_for_ids(layouts: &[Capability], id: &str) -> String {
    match super::super::stage_bytes_for(4, &honest(), layouts) {
        Err(err) => err,
        Ok(stages) => {
            let ids: Vec<&str> = stages.iter().map(|(stage, _)| *stage).collect();
            panic!("{id} was hashed under the ids {ids:?} rather than refused")
        }
    }
}
