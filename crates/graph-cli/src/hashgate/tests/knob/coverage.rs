//! Every `registry::LAYOUTS` id has a per-stage negative control, or is on an allow list
//! that says why not.
//!
//! **This test exists because the knob tables are hand-maintained and nothing failed when a
//! layout was added to the registry without one.** `layout.basic3d.spiral` and
//! `layout.bipartite_3d` were both hashed by the gate and both had no control, so the gate
//! could report `4-way equal` for a stage whose bytes had gone constant or empty — "equal"
//! only ever compares four arms with each other. `AGENTS.md` asks for a negative control on
//! every gate row; this asks the same question of the whole table, once, so the next
//! un-controlled layout fails a test that names it rather than going unnoticed.
//!
//! **The allow list was MEASURED, not guessed.** Two kinds of entry appear in it and the
//! difference matters:
//!
//! - **`HAS_OWN_STAGE_NODES`** — the id is reached by a `Knob` variant that sets
//!   `Setting::stage_nodes` for that stage alone (`knob/setting.rs:151-195`). These are
//!   real per-stage controls that predate `knobs::all()` and live in the `Knob` enum rather
//!   than the stage tables, so a naive read of `knobs::all()` alone would wrongly list them.
//! - **`NO_CONTROL`** — nothing moves this stage but the reference model, which moves every
//!   stage at once. These are the genuine remaining holes, and they are pre-existing: this
//!   job added knobs for the two 3D layouts it names and did not open new ones elsewhere.
//!
//! Split from `ids.rs` by the house's 300-line limit.

use crate::hashgate::knobs;

/// What kind of gap an allow-listed id is, so a reader knows whether adding it is a job or
/// a rewrite.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Gap {
    /// A `Knob` variant sets `stage_nodes` for this id alone — a real per-stage control,
    /// reachable without appearing in `knobs::all()`.
    HasOwnStageNodes,
    /// Nothing scopes a perturbation to this stage. The reference model moves it, but so
    /// does that move every other stage, so the control proves nothing about this one.
    NoControl,
}

/// The layout ids `knobs::all()` does not tabulate, each with what it actually has.
///
/// Sorted by id so a diff reads as a change to the set rather than a reshuffle.
const NO_PER_STAGE_CONTROL: [(&str, Gap); 34] = [
    ("layout.bipartite", Gap::NoControl),
    ("layout.circular.circo", Gap::NoControl),
    ("layout.circular.hierarchy", Gap::HasOwnStageNodes),
    // `circular.radial` is reached by `Knob::CircularNodes`, whose variable is
    // `GM_MUTATE_CIRCULAR_NODES` — spelled for the layout family, not the id.
    ("layout.circular.radial", Gap::HasOwnStageNodes),
    ("layout.circular.ring", Gap::NoControl),
    ("layout.dag.sugiyama", Gap::NoControl),
    ("layout.force.barnes_hut", Gap::NoControl),
    // The p12-t4b 3D arms. Their 2D siblings' knobs (`IGRAPH_LAYOUT_STAGES`) scope
    // `stage_nodes` to the 2D id, so they do not reach these ids.
    ("layout.force.drl.3d", Gap::NoControl),
    ("layout.force.fdp", Gap::NoControl),
    ("layout.force.fruchterman_reingold.3d", Gap::NoControl),
    ("layout.force.kamada_kawai.3d", Gap::NoControl),
    ("layout.force.neato", Gap::NoControl),
    // `layout.force.particle_mesh` is a tiered force (`hashgate/tiered.rs`) like
    // `barnes_hut`: no knob scopes `stage_nodes` to it.
    ("layout.force.particle_mesh", Gap::NoControl),
    ("layout.force.sfdp", Gap::NoControl),
    // `layout.force.spring` is reached by `Knob::SpringIterations`, a *parameter* control
    // rather than a per-stage node control, and that control reaches `spring3d` too — so
    // neither knob names this stage alone.
    ("layout.force.spring", Gap::NoControl),
    ("layout.force.yifan_hu", Gap::NoControl),
    ("layout.force.yifan_hu.2z", Gap::NoControl),
    ("layout.forceatlas2", Gap::NoControl),
    ("layout.forceatlas2.3d", Gap::NoControl),
    ("layout.forceatlas2.barnes_hut", Gap::NoControl),
    ("layout.forceatlas2.forcesim", Gap::NoControl),
    ("layout.grid", Gap::NoControl),
    ("layout.mds.pivot", Gap::NoControl),
    // The 3D arms of the two ids above, same gap and for the same reason: no knob scopes
    // `stage_nodes` to either, and the reference model moves both along with everything else.
    ("layout.mds.pivot3d", Gap::NoControl),
    ("layout.packing.circle", Gap::NoControl),
    ("layout.random", Gap::NoControl),
    // `layout.random.3d` is the same gap as the id above and for the same reason: no knob
    // scopes `stage_nodes` to either, and the reference model moves both along with every
    // other stage. It is a separate entry rather than a widened one because the list is
    // keyed by id, and the gate is what says the hash is over the stage.
    ("layout.random.3d", Gap::NoControl),
    ("layout.spectral", Gap::NoControl),
    ("layout.spectral3d", Gap::NoControl),
    ("layout.spiral", Gap::NoControl),
    ("layout.tree.tidy", Gap::HasOwnStageNodes),
    ("layout.treemap.patchwork", Gap::HasOwnStageNodes),
    // `layout.treemap.squarified` is reached by `Knob::TreemapNodes`, which scopes
    // `stage_nodes` to it alone.
    ("layout.treemap.squarified", Gap::HasOwnStageNodes),
    ("layout.twopi", Gap::HasOwnStageNodes),
];

/// Every registered layout id is tabulated by a per-stage control or allow-listed here.
#[test]
fn every_registered_layout_has_a_per_stage_control_or_a_reason_it_does_not() {
    let mut missing: Vec<&str> = graph_core::registry::LAYOUTS
        .iter()
        .map(|layout| layout.id)
        .filter(|id| !knobs::all().any(|stage| stage.id == *id))
        .filter(|id| !NO_PER_STAGE_CONTROL.iter().any(|(listed, _)| listed == id))
        .collect();
    missing.sort_unstable();
    assert!(
        missing.is_empty(),
        "{} registered layout(s) have no per-stage negative control and no allow-list \
         entry: {missing:?}. A layout the gate hashes but cannot move is a stage whose bytes \
         could go constant and still read `4-way equal`. Either add it to hashgate::knobs \
         (the family table beside THREE_D_LAYOUT_STAGES), or add it to NO_PER_STAGE_CONTROL \
         above with the kind of gap it is.",
        missing.len()
    );
}

/// The allow list is not a place ids go to be forgotten. Two invariants, both cheap:
/// every entry names an id the registry actually has (a typo would silently exempt
/// nothing), and every entry is still genuinely un-tabled by `knobs::all()` — an id that
/// gains a control must lose its exemption, or the list becomes a place a working knob is
/// hidden behind an excuse.
#[test]
fn the_allow_list_is_sorted_names_real_ids_and_exempts_nothing_that_is_tabled() {
    let listed: Vec<&str> = NO_PER_STAGE_CONTROL.iter().map(|(id, _)| *id).collect();
    let mut sorted = listed.clone();
    sorted.sort_unstable();
    assert_eq!(listed, sorted, "the allow list is not sorted by id");
    for (id, _) in NO_PER_STAGE_CONTROL {
        assert!(
            graph_core::registry::LAYOUTS.iter().any(|l| l.id == id),
            "{id} is allow-listed but is not a registered layout id"
        );
        assert!(
            !knobs::all().any(|stage| stage.id == id),
            "{id} is allow-listed but hashgate::knobs now tables a control for it: drop the \
             exemption, which is the whole point of keeping this list honest"
        );
    }
}

/// The variable each `HasOwnStageNodes` entry's control is set by.
///
/// **Written out rather than derived from the id, because deriving it was wrong.** The
/// natural spelling — `layout.x.y` becomes `GM_MUTATE_X_Y_NODES` — is right for four of
/// these five and wrong for `layout.circular.radial`, whose control is
/// `Knob::CircularNodes` set by `GM_MUTATE_CIRCULAR_NODES`. A derived name is how a
/// coverage test starts asserting things that are not true; this table is the measured
/// answer, one row per claim below.
const VARIABLE_FOR: [(&str, &str); 6] = [
    (
        "layout.circular.hierarchy",
        "GM_MUTATE_CIRCULAR_HIERARCHY_NODES",
    ),
    ("layout.circular.radial", "GM_MUTATE_CIRCULAR_NODES"),
    ("layout.tree.tidy", "GM_MUTATE_TREE_TIDY_NODES"),
    ("layout.treemap.patchwork", "GM_MUTATE_PATCHWORK_NODES"),
    ("layout.treemap.squarified", "GM_MUTATE_TREEMAP_NODES"),
    // `GM_MUTATE_TWOPI_NODES` — two O's, because the stage is `twopi`.
    ("layout.twopi", "GM_MUTATE_TWOPI_NODES"),
];

/// `HasOwnStageNodes` is a claim about the code, not a category invented to shrink the
/// `NoControl` count. This runs each claiming variable through the gate's own parse and
/// asserts it comes back naming **that** stage — so an entry cannot claim a control that
/// does not exist, nor one that moves some other stage.
#[test]
fn every_id_claiming_its_own_stage_nodes_really_does() {
    for (id, gap) in NO_PER_STAGE_CONTROL {
        if gap != Gap::HasOwnStageNodes {
            continue;
        }
        // The variable is NOT always the id spelled out: `circular.radial` is set by
        // `Knob::CircularNodes`, whose variable is `GM_MUTATE_CIRCULAR_NODES`. So the
        // check asks the question directly — does some control scope a perturbation to
        // THIS stage — rather than guessing the variable's name from the id.
        let variable = VARIABLE_FOR
            .iter()
            .find(|(listed, _)| *listed == id)
            .map(|(_, variable)| *variable)
            .unwrap_or_else(|| {
                panic!("{id} claims HasOwnStageNodes but names no variable to check")
            });
        let setting = crate::hashgate::knob::setting::setting(|name| {
            if name == variable {
                Ok("1".to_string())
            } else {
                Err(std::env::VarError::NotPresent)
            }
        })
        .unwrap_or_else(|e| {
            panic!("{id} claims HasOwnStageNodes, but {variable} is not a control: {e}")
        });
        assert_eq!(
            setting.stage_nodes,
            Some((id, 1)),
            "{id}: {variable} does not scope a one-node perturbation to this stage"
        );
    }
}
