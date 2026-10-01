//! The sweep's **layout routing**: that it times the stage the plan's `--layout` names,
//! names it in the report, and keeps the byte check honest for the multilevel force stage.
//!
//! Split from the parent module for the house's 300-line cap. The helpers it needs
//! (`plan`, and `base_cells`/`host` from [`report`]) come from there rather than being
//! restated, so a fixture one of them changes is a fixture this file sees.

use super::super::markdown::markdown;
use super::super::route::{Control, ROUTES};
use super::super::{Cell, Tier, layouts, run, run_under};
use super::plan;
use super::report::{base_cells, host};
use graph_core::Grid;
use graph_core::Stage as _;
use graph_core::layout::force::{BarnesHut, Split, YifanHu};
use graph_core::layout::{circular::ring, spiral};

/// The sweep must time **the layout the plan names**, and every routed layout must be
/// reachable. A sweep that ignored `--layout` and always ran Barnes-Hut would report equal,
/// fast numbers under `layout.force.yifan_hu`'s name, and the yifan_hu tier row would be a
/// copy of the barnes_hut one.
#[test]
fn the_sweep_times_every_layout_the_plan_names_and_refuses_one_it_cannot() {
    let mut p = plan(vec![40], 1);
    assert_eq!(
        layouts(&p).expect("no --layout is the default"),
        vec![BarnesHut::ID],
        "no --layout must stay Barnes-Hut, so every existing row still reproduces"
    );
    for id in ROUTES {
        p.layouts = vec![id.to_owned()];
        assert_eq!(layouts(&p).expect("routed"), vec![id], "{id}");
    }
    // A layout with no threaded tier is refused by name, not run under the default's
    // numbers: the caller asked about a stage the sweep never measured.
    p.layouts = vec!["layout.spectral".to_owned()];
    let err = layouts(&p).expect_err("no tier for spectral");
    assert!(err.contains("layout.spectral"), "must name it: {err:?}");
    for id in ROUTES {
        assert!(err.contains(id), "the refusal must name {id:?}: {err:?}");
    }
}

/// The report names the stage it timed, per layout: a yifan_hu table titled Barnes-Hut is
/// a row of Barnes-Hut's numbers under the wrong heading.
#[test]
fn the_report_names_the_layout_the_cells_were_timed_over() {
    let cells = [Cell {
        layout: YifanHu::ID,
        ..base_cells().swap_remove(0)
    }];
    let report = markdown(&plan(vec![220], 1), &cells, &host());
    assert!(report.contains("Yifan-Hu"), "{report}");
    assert!(
        !report.contains("Barnes-Hut"),
        "a yifan_hu table titled Barnes-Hut is the defect this test exists for"
    );
}

/// Every routed layout's threaded arm is byte-equal to its own serial arm, and the rescale
/// control turns red exactly the two stages that end in that merge.
/// The closed-form routes' arms: the four widths the gate proves, each against its own
/// layout's serial arm.
const CLOSED_FORM_ARMS: [Tier; 4] = [
    Tier::Scalar,
    Tier::Threads(1),
    Tier::Threads(3),
    Tier::Threads(7),
];

#[test]
fn every_routed_layout_is_byte_equal_at_every_width() {
    let routes = vec![Grid::ID, ring::ID, spiral::ID];
    let (honest, _) = run(&plan(vec![40], 1), &routes, &CLOSED_FORM_ARMS).expect("ran");
    for cell in &honest {
        assert!(
            cell.equal,
            "{} {} at {:?} disagreed with its own serial arm",
            cell.layout, cell.n, cell.tier
        );
    }
}

/// The negative control for the `equal` column above: the two stages with a merge go red,
/// and the grid — which has none — does not. A control that turned all three red would be
/// reaching the gather, not the merge, and one that turned none of them would leave the
/// column untested.
#[test]
fn the_rescale_control_bites_the_two_merged_routes_and_not_the_grid() {
    let routes = vec![Grid::ID, ring::ID, spiral::ID];
    let (split, _) = run_under(
        &plan(vec![40], 1),
        &routes,
        &CLOSED_FORM_ARMS,
        Control {
            split_rescale: true,
            ..Control::HONEST
        },
    )
    .expect("ran");
    for cell in &split {
        if cell.tier == Tier::Scalar {
            // The reference is always the honest one — a control that reached it would
            // agree with the arms it is meant to contradict.
            assert!(cell.equal, "{}: the reference was mutated", cell.layout);
            continue;
        }
        let want = cell.layout != Grid::ID;
        assert_eq!(
            !cell.equal,
            want,
            "{} at {:?}: a rescale control that moved {} is reaching the wrong half",
            cell.layout,
            cell.tier,
            if want { "the gather" } else { "the grid" }
        );
    }
}

/// The multilevel stage is byte-equal to its own serial arm at every threaded width, and
/// its own split control turns it red. Without this the yifan_hu sweep would be a table of
/// timings with an `equal` column nothing had checked.
#[test]
fn the_multilevel_layout_is_byte_equal_at_every_width_and_its_control_turns_it_red() {
    let p = plan(vec![40], 1);
    let routes = vec![YifanHu::ID];
    let arms = [
        Tier::Scalar,
        Tier::Threads(2),
        Tier::Threads(4),
        Tier::Threads(7),
    ];
    let (cells, _) = run(&p, &routes, &arms).expect("ran");
    assert_eq!(cells.len(), 4);
    for cell in &cells {
        assert!(
            cell.equal,
            "yifan_hu {} disagreed with its own serial arm",
            cell.tier.arm_name()
        );
    }
    // And the control bites on this layout, so the equality above is not a tautology.
    let (split, _) = run_under(
        &p,
        &routes,
        &[Tier::Scalar, Tier::Threads(4)],
        Control {
            split: Split::All,
            ..Control::HONEST
        },
    )
    .expect("ran");
    assert!(split[0].equal, "the serial arm is its own reference");
    assert!(
        !split[1].equal,
        "the yifan_hu sweep's byte check is vacuous: its control changed nothing"
    );
}

/// The bench row and the gate row are the same claim, so the multilevel stage is in
/// `hashgate --tiers all`'s recompute list: a stage timed threaded but hashed from the
/// scalar column would report an `equal` in one report and compare nothing in the other.
#[test]
fn the_multilevel_stage_is_in_the_gate_sweep() {
    assert!(
        crate::hashgate::tiered::THREADED_STAGES.contains(&YifanHu::ID),
        // and the arm that reads the list is the same one the gate drives:
        "the multilevel stage is timed threaded by the bench but not recomputed by the gate"
    );
}
