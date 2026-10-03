//! The report the hash gate prints and records: the detail block, the ledger's JSON and
//! the exit code, each pinned to its exact words, keys and counts.

use super::super::compare::{C20_ARM, arm};
use super::super::report::{arm_report, body, checked_arm_report, exit, passed};
use super::super::stages::stages as stage_ids;

use super::compare::{HONEST, arms};
use super::{Arm, Knob, Tally};
use crate::runner::sha256_hex;
use std::process::ExitCode;

/// `count` lines per arm, the arm's own digit repeated over them, so a report over more
/// lines than [`super::arms`] builds can still be read.
fn wide_arms(count: usize) -> Vec<Arm> {
    [
        "native run 1",
        "native run 2",
        "wasm32 run 1",
        "wasm32 run 2",
    ]
    .iter()
    .enumerate()
    .map(|(arm, name)| {
        let fill = char::from(b'a' + arm as u8);
        let lines = (0..count)
            .map(|i| format!("stage{i} {i} {}", fill.to_string().repeat(64)))
            .collect();
        (*name, lines)
    })
    .collect()
}

/// The detail report: one digest line per arm, then at most three diverged lines, each
/// naming its own `stage seed` and showing all four arms. Pinned to the exact text.
#[test]
fn the_detail_report_names_three_diverged_lines_at_most_and_all_four_arms() {
    let arms = wide_arms(10);
    let mut text = String::new();
    arm_report(&mut text, &arms, &[1, 3, 5, 7, 9]);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 4 + 3 * 5, "{text}");
    for (arm, (name, output)) in arms.iter().enumerate() {
        let digest = sha256_hex(output.join("\n").as_bytes());
        assert_eq!(lines[arm], format!("  {name:<13} digest {digest}"));
    }
    for (i, line) in [1usize, 3, 5].iter().enumerate() {
        assert_eq!(lines[4 + i * 5], format!("  DIVERGED stage{line} {line}:"));
        for (arm, (name, output)) in arms.iter().enumerate() {
            assert_eq!(
                lines[5 + i * 5 + arm],
                format!("    {name:<13} {}", output[*line])
            );
        }
    }
    assert!(!text.contains("DIVERGED stage7"), "only the first three");
}

/// The record the ledger reads, exactly as `evidence::write` serialises it (serde_json
/// orders a value's keys, so that order is pinned too).
#[test]
fn the_record_holds_the_exact_counts_it_reports() {
    let clean = Tally {
        equal: vec![3; stage_ids().len()],
        diverged_seeds: 0,
    };
    let four = arms(HONEST);
    let text =
        serde_json::to_string(&body(None, 3, &clean, 3, &four).expect("record")).expect("json");
    assert_eq!(
        text,
        r#"{"arm_names":["native run 1","native run 2","wasm32 run 1","wasm32 run 2"],"arms":4,"equal":{"analysis.centrality.betweenness":3,"analysis.centrality.closeness":3,"analysis.centrality.degree":3,"analysis.centrality.eigenvector":3,"analysis.communities.louvain":3,"analysis.components.strong":3,"analysis.components.weak":3,"analysis.depth.bfs":3,"layout.basic3d.cube":3,"layout.basic3d.helix":3,"layout.basic3d.sphere":3,"layout.basic3d.spiral":3,"layout.bipartite":3,"layout.bipartite_3d":3,"layout.circular.circo":3,"layout.circular.hierarchy":3,"layout.circular.radial":3,"layout.circular.ring":3,"layout.dag.sugiyama":3,"layout.force.barnes_hut":3,"layout.force.davidson_harel":3,"layout.force.drl":3,"layout.force.fdp":3,"layout.force.fruchterman_reingold":3,"layout.force.graphopt":3,"layout.force.kamada_kawai":3,"layout.force.lgl":3,"layout.force.neato":3,"layout.force.particle_mesh":3,"layout.force.sfdp":3,"layout.force.spring":3,"layout.force.spring3d":3,"layout.force.yifan_hu":3,"layout.forceatlas2":3,"layout.forceatlas2.barnes_hut":3,"layout.grid":3,"layout.hierarchical3d":3,"layout.mds.pivot":3,"layout.packing.circle":3,"layout.packing.osage":3,"layout.random":3,"layout.spectral":3,"layout.spiral":3,"layout.tree.tidy":3,"layout.treemap.patchwork":3,"layout.treemap.squarified":3,"layout.twopi":3,"post.bundle.fdeb":3,"post.bundle.mingle":3,"post.route.grid":3,"post.style.bezier":3,"post.style.orthogonal":3,"post.style.quadratic":3,"post.style.straight":3,"topology":3,"transport.wasm.columnar":3},"mutation":null,"pass":true,"seeds":3,"transport":{"equal":3,"reference":"layout.grid","stage":"transport.wasm.columnar"}}"#
    );
    // The arm count is in the record because the per-stage counts mean nothing without
    // it: `equal: 8` is a different claim at four arms than at nine.
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&text).expect("json")["arms"],
        4
    );
    let diverged = Tally {
        equal: vec![3; stage_ids().len()],
        diverged_seeds: 1,
    };
    let control = body(Some(Knob::GridSpacing), 3, &diverged, 3, &four).expect("record");
    assert_eq!(control["pass"], serde_json::json!(false));
    assert_eq!(control["mutation"], "GM_MUTATE_GRID_SPACING");
    assert_eq!(Knob::GridSpacing.record(), "hashgate-control-grid-spacing");
    assert_eq!(exit(true), ExitCode::SUCCESS);
    assert_eq!(exit(false), ExitCode::from(1));
}

/// **RG-06: the exit code cannot disagree with the record.** The report case: every arm
/// agrees (`diverged_seeds == 0`) but the C20 tally reached only `seeds - 1` seed.
///
/// Before the repair the record's `pass` and `report::exit`'s argument were computed from
/// different inputs, so `exit(0)` returned `ExitCode::SUCCESS` on exactly this run — a
/// verdict the record itself called a failure. One predicate now feeds both, and this test
/// is the statement that they cannot part again.
#[test]
fn a_c20_shortfall_fails_the_record_and_the_exit_code_together() {
    let arms = arms(HONEST);
    let clean = Tally {
        equal: vec![3; stage_ids().len()],
        diverged_seeds: 0,
    };
    let short = body(None, 3, &clean, 2, &arms).expect("record");
    assert_eq!(short["pass"], serde_json::json!(false));
    assert_eq!(short["transport"]["equal"], serde_json::json!(2));
    assert_eq!(exit(passed(&clean, 2, 3)), ExitCode::from(1));
    // The same tally at a full C20 tally is the only difference, and it passes both.
    let full = body(None, 3, &clean, 3, &arms).expect("record");
    assert_eq!(full["pass"], serde_json::json!(true));
    assert_eq!(exit(passed(&clean, 3, 3)), ExitCode::SUCCESS);
    // And a divergence fails both, whatever the C20 tally says.
    let diverged = Tally {
        equal: vec![3; stage_ids().len()],
        diverged_seeds: 2,
    };
    assert_eq!(
        body(None, 3, &diverged, 3, &arms).expect("record")["pass"],
        serde_json::json!(false)
    );
    assert_eq!(exit(passed(&diverged, 3, 3)), ExitCode::from(1));
}

/// **RG-07: a length mismatch is a refusal, not a truncation.** The record zips the stage
/// list against the tally's per-stage counts; with the tally one short, a `zip` silently
/// drops the last stage and the record reads as one in which that stage was equal.
#[test]
fn a_tally_that_does_not_cover_every_stage_is_refused() {
    let arms = arms(HONEST);
    let short = Tally {
        equal: vec![3; stage_ids().len() - 1],
        diverged_seeds: 0,
    };
    let err = body(None, 3, &short, 3, &arms).expect_err("a stage is missing");
    assert!(
        err.contains(&format!("{} stage counts", short.equal.len())),
        "{err}"
    );
    assert!(
        err.contains(&format!("{} stages", stage_ids().len())),
        "{err}"
    );
    // One stage *too many* is the same refusal, not a record naming a stage the gate never
    // hashed: the tally and the stage list have to be the same list, not merely overlapping.
    let long = Tally {
        equal: vec![3; stage_ids().len() + 1],
        diverged_seeds: 0,
    };
    assert!(
        body(None, 3, &long, 3, &arms).is_err(),
        "one stage too many"
    );
}

/// **RG-07's other half: an arm that printed fewer lines is a refusal, not a `panic!`.**
/// `arm_report` indexes `arms[0].1[i]` and `output[i]` by the diverged line's position; a
/// short arm made the second index past the end of the first.
#[test]
fn an_arm_shorter_than_the_others_is_refused_rather_than_indexed_past_the_end() {
    let mut arms = wide_arms(10);
    arms[2].1.truncate(4);
    let mut text = String::new();
    let err = checked_arm_report(&mut text, &arms, &[1, 3]).expect_err("short arm");
    assert!(err.contains("wasm32 run 1"), "{err}");
    assert!(err.contains("4 lines"), "{err}");
    // A diverged line past what every arm printed is refused too, not indexed.
    let mut text = String::new();
    let err = checked_arm_report(&mut text, &wide_arms(3), &[9]).expect_err("past the end");
    assert!(err.contains("past the 3 lines"), "{err}");
    // No arms at all is a refusal: a detail block with no arm in it reports nothing.
    let mut text = String::new();
    assert!(checked_arm_report(&mut text, &[], &[]).is_err());
    // The fire-and-forget wrapper is the one `forcecheck`'s gate calls: it must not
    // panic on the same inputs, and the hash gate does not read what it wrote.
    let mut text = String::new();
    arm_report(&mut text, &wide_arms(3), &[9]);
}

/// **RG-36: the C20 arm is named.** `conclude` read `arms[2]` positionally while
/// `compare` accepts any list of at least `MIN_ARMS` arms, so a shorter list was a panic
/// rather than a refusal. `arm` turns the absence into `Err`.
#[test]
fn the_c20_tally_names_its_arm_and_refuses_a_list_without_it() {
    let four = arms(HONEST);
    assert_eq!(arm(&four, C20_ARM).map(<[String]>::len), Ok(6));
    let two: Vec<Arm> = four.iter().take(2).cloned().collect();
    let err = arm(&two, C20_ARM).expect_err("no wasm arm");
    assert!(err.contains(C20_ARM), "{err}");
    assert!(err.contains("among 2"), "{err}");
}
