//! The sweep's **layout routing**: that it times the stage the plan's `--layout` names,
//! names it in the report, and keeps the byte check honest for both force layouts.
//!
//! Split from the parent module for the house's 300-line cap. The helpers it needs
//! (`plan`, `base_cells`, `host`) come from there rather than being restated, so a fixture
//! the parent changes is a fixture this file sees.

use super::super::layout::layout;
use super::super::markdown::markdown;
use super::super::{Layout, Tier, run, run_under};
use super::{base_cells, host, plan};
use graph_core::Stage as _;
use graph_core::layout::force::{BarnesHut, Split, YifanHu};

/// The sweep must time **the layout the plan names**. A sweep that ignored `--layout` and
/// always ran Barnes-Hut would report equal, fast numbers under `layout.force.yifan_hu`'s
/// name, and the yifan_hu tier row would be a copy of the barnes_hut one.
#[test]
fn the_sweep_times_the_layout_the_plan_names_and_names_it_in_the_report() {
    let mut p = plan(vec![40], 1);
    assert_eq!(
        layout(&p).expect("no --layout is the default"),
        Layout::BarnesHut,
        "no --layout must stay BarnesHut, so every existing row still reproduces"
    );
    for (id, want) in [
        (BarnesHut::ID, Layout::BarnesHut),
        (YifanHu::ID, Layout::YifanHu),
    ] {
        p.layouts = vec![id.to_owned()];
        assert_eq!(layout(&p).expect("registered"), want, "{id}");
    }
    // A layout with no threaded tier is refused by name, not run under the default's
    // numbers: the caller asked about a stage the sweep never measured.
    p.layouts = vec!["layout.spectral".to_owned()];
    let err = layout(&p).expect_err("no tier for spectral");
    assert!(err.contains("layout.spectral"), "must name it: {err:?}");
    p.layouts = vec![BarnesHut::ID.to_owned(), YifanHu::ID.to_owned()];
    assert!(
        layout(&p).is_err(),
        "one table has one title, so two layouts must be refused"
    );
    let report = markdown(&p, Layout::YifanHu, &base_cells(), &host());
    assert!(
        report.contains("Yifan-Hu"),
        "the report must name what it timed"
    );
    assert!(
        !report.contains("Barnes-Hut"),
        "a yifan_hu table titled Barnes-Hut is the defect this test exists for"
    );
}

/// The multilevel stage is byte-equal to its own serial arm at every threaded width, and
/// its own split control turns it red. Without this the yifan_hu sweep would be a table of
/// timings with an `equal` column nothing had checked.
#[test]
fn the_multilevel_layout_is_byte_equal_at_every_width_and_its_control_turns_it_red() {
    let p = plan(vec![40], 1);
    let (cells, _) = run(
        &p,
        Layout::YifanHu,
        &[
            Tier::Scalar,
            Tier::Threads(2),
            Tier::Threads(4),
            Tier::Threads(7),
        ],
    )
    .expect("ran");
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
        Layout::YifanHu,
        &[Tier::Scalar, Tier::Threads(4)],
        Split::All,
    )
    .expect("ran");
    assert!(split[0].equal, "the serial arm is its own reference");
    assert!(
        !split[1].equal,
        "the yifan_hu sweep's byte check is vacuous: its control changed nothing"
    );
}
