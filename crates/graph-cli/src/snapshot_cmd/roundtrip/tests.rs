//! The sweep's own accounting: what it checked, what it found, what it records and
//! what it exits with. Every count is pinned to the exact text or JSON, so no count can
//! move without one of these going red.

use super::{
    Findings, OTHER_LAYOUTS, body, snapshot_total, sweep, swept_layouts, verdict, write_findings,
};

/// A clean run of `seeds` seeds: every check empty, every notes case drawn, at least one
/// 3D snapshot round-tripped, and exactly the snapshots the registry promises checked.
fn clean(seeds: u32) -> Findings {
    Findings {
        notes: [seeds.into(); 5],
        three_d: 1,
        checked: snapshot_total(seeds),
        ..Findings::default()
    }
}

#[test]
fn overall_pass_requires_every_check_clean_and_every_notes_case_drawn() {
    assert!(!Findings::default().pass(1), "no notes case drawn yet");
    assert!(!clean(1).pass(2), "fewer snapshots checked than promised");
    let all_drawn = clean(5);
    assert!(all_drawn.pass(5));
    for bad in [
        Findings {
            faces: vec!["x".into()],
            ..clean(5)
        },
        Findings {
            grid: vec!["x".into()],
            ..clean(5)
        },
        Findings {
            circular: vec!["x".into()],
            ..clean(5)
        },
        Findings {
            packing: vec!["x".into()],
            ..clean(5)
        },
        Findings {
            dag: vec!["x".into()],
            ..clean(5)
        },
        Findings {
            notes: [5, 5, 5, 5, 0],
            ..clean(5)
        },
        // A run that checked no 3D snapshot at all: the sweep claims the z column is
        // round-tripped, and a seed rule that stopped drawing 3D would leave that claim
        // resting on nothing.
        Findings {
            three_d: 0,
            ..clean(5)
        },
    ] {
        assert!(!bad.pass(5), "{bad:?}");
    }
}

/// The sweep's list of layouts is the registry's, so a layout added there is swept
/// without being added here, and a d3-only layout that left the registry is caught.
#[test]
fn the_sweep_runs_every_registered_layout_and_counts_exactly_them() {
    let registered: Vec<&str> = super::registry::LAYOUTS
        .iter()
        .map(|layout| super::super::short_name(layout.id))
        .collect();
    assert_eq!(swept_layouts(), registered);
    for d3_only in OTHER_LAYOUTS {
        assert!(swept_layouts().contains(&d3_only), "{d3_only}");
    }
    let found = sweep(3).expect("runs");
    assert_eq!(found.checked, snapshot_total(3));
    assert_eq!(snapshot_total(3), 3 * (1 + registered.len() as u64));
}

/// The record the ledger reads, exactly as `evidence::write` serialises it (serde_json
/// orders a value's keys, so this pins that order too): every name, count and flag
/// spelled out here, so none of them can move without this going red.
#[test]
fn the_record_holds_the_exact_counts_it_reports() {
    let found = Findings {
        faces: vec!["a".into()],
        grid: vec!["b".into(), "c".into()],
        dag: vec!["d".into()],
        notes: [1, 2, 3, 4, 5],
        three_d: 6,
        checked: snapshot_total(5),
        ..Findings::default()
    };
    let text = serde_json::to_string(&body(5, &found)).expect("json");
    assert_eq!(
        text,
        r#"{"faces_failed":1,"functions":{"layout.circular.radial":{"cases":5,"declared":0,"unexplained":0},"layout.dag.sugiyama":{"cases":5,"declared":0,"unexplained":1},"layout.grid":{"cases":5,"declared":0,"unexplained":2},"layout.packing.circle":{"cases":5,"declared":0,"unexplained":0}},"notes_cases":{"0.2-labelled":1,"0.3 k=0":2,"code 1":3,"code 2":4,"code 3":5},"pass":false,"seeds":5,"snapshots":175,"three_d_exercise":6}"#
    );
    assert_eq!(body(5, &clean(5))["pass"], serde_json::json!(true));
}

/// The exit code a gate reads: success only on a clean run, `1` on anything else.
#[test]
fn the_exit_code_is_success_only_when_everything_is_clean() {
    assert_eq!(verdict(&clean(5), 5), std::process::ExitCode::SUCCESS);
    for bad in [
        Findings {
            faces: vec!["x".into()],
            ..clean(5)
        },
        Findings {
            notes: [0; 5],
            ..clean(5)
        },
    ] {
        assert_eq!(verdict(&bad, 5), std::process::ExitCode::from(1));
    }
}

#[test]
fn print_findings_subtracts_failures_from_the_total_not_adds() {
    let found = Findings {
        faces: vec!["a".into(), "b".into()],
        grid: vec!["c".into()],
        dag: vec!["d".into(), "e".into()],
        notes: [1; 5],
        checked: snapshot_total(5),
        ..Findings::default()
    };
    let mut text = String::new();
    write_findings(&mut text, 5, &found);
    let snapshots = 5 * (1 + super::registry::LAYOUTS.len());
    assert!(
        text.contains(&format!(
            "byte-exact on {}/{snapshots} snapshots",
            snapshots - 2
        )),
        "{text}"
    );
    assert!(
        text.contains("layout.grid on its stated conventions on 4/5 seeds"),
        "{text}"
    );
    assert!(
        text.contains("layout.circular.radial on its stated conventions on 5/5 seeds"),
        "{text}"
    );
    assert!(
        text.contains("layout.dag.sugiyama on its structural invariants on 3/5 seeds"),
        "{text}"
    );
    assert!(text.contains("notes cases drawn (exercise, each needed): 0.2-labelled 1, 0.3 k=0 1, code 1 1, code 2 1, code 3 1"), "{text}");
}

/// The printed report names at most six failures, and names the first six.
#[test]
fn at_most_six_failures_are_printed_and_they_are_the_first_six() {
    let found = Findings {
        faces: (0..4).map(|i| format!("f{i}")).collect(),
        grid: (0..3).map(|i| format!("g{i}")).collect(),
        notes: [1; 5],
        checked: snapshot_total(7),
        ..Findings::default()
    };
    let mut text = String::new();
    write_findings(&mut text, 7, &found);
    let failed: Vec<&str> = text
        .lines()
        .filter_map(|l| l.strip_prefix("  FAILED "))
        .collect();
    assert_eq!(failed, ["f0", "f1", "f2", "f3", "g0", "g1"]);
}

#[test]
fn the_sweep_records_nothing_wrong_and_refuses_zero_seeds() {
    let found = sweep(12).expect("runs");
    assert!(found.faces.is_empty() && found.grid.is_empty(), "{found:?}");
    assert!(
        found.circular.is_empty() && found.packing.is_empty() && found.dag.is_empty(),
        "{found:?}"
    );
    assert!(found.pass(12), "every notes case drawn: {:?}", found.notes);
    assert!(
        !sweep(4).expect("runs").pass(4),
        "four seeds cannot draw every case"
    );
    assert!(sweep(0).expect_err("empty").starts_with("0 seeds"));
}
