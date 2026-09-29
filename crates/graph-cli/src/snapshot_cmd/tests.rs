use super::*;
use graph_contract::binary::SnapshotParts;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};
use graph_contract::notes::{Note, NoteCode, Notes, SNAPSHOT_WIDE};
use std::collections::BTreeSet;

#[test]
fn every_registered_layout_is_offered_by_its_short_name() {
    assert_eq!(layout_names(), ["grid"]);
    let run = pipeline(1, 50, "grid").expect("runs");
    assert_eq!(
        (run.layout, run.snapshot.header().node_count),
        ("layout.grid", 50)
    );
    let err = pipeline(1, 50, "spiral").expect_err("unregistered");
    assert_eq!(err, "no layout \"spiral\": one of grid");
}

#[test]
fn both_faces_round_trip_on_the_grid_and_on_the_exercise() {
    for seed in 0..40 {
        let grid = pipeline(seed, gate_node_count(seed), "grid")
            .expect("runs")
            .snapshot;
        assert_eq!(faces_agree(&grid), Ok(()), "grid seed {seed}");
        assert_eq!(grid_by_hand(&grid), Ok(()), "grid seed {seed}");
        let exercise = exercise::snapshot(seed).expect("valid");
        assert_eq!(faces_agree(&exercise), Ok(()), "exercise seed {seed}");
    }
}

#[test]
fn nine_seeds_of_the_exercise_cover_every_pair_of_kinds_and_the_escapes() {
    let mut pairs = BTreeSet::new();
    let mut text = String::new();
    for seed in 100..109 {
        let snapshot = exercise::snapshot(seed).expect("valid");
        let h = snapshot.header();
        pairs.insert((h.node_kind as u8, h.edge_kind as u8));
        text += &to_json(&snapshot);
    }
    assert_eq!(pairs.len(), 9, "{pairs:?}");
    for escaped in ["\\\"", "\\\\", "\\u0000", "\\n\\t", "\\u001f", "\"\""] {
        assert!(text.contains(escaped), "no {escaped} in {text}");
    }
    assert_eq!(
        exercise::snapshot(7).expect("valid"),
        exercise::snapshot(7).expect("valid")
    );
}

/// Any five consecutive seeds of the exercise draw a 0.2-labelled snapshot, a 0.3 one
/// with no notes, and notes of every implemented code; each is byte-exact both ways.
#[test]
fn the_exercise_draws_every_notes_case_and_each_round_trips() {
    let mut cases = [0; 5];
    for seed in 0..5 {
        let snapshot = exercise::snapshot(seed).expect("valid");
        assert_eq!(faces_agree(&snapshot), Ok(()), "seed {seed}");
        exercise::count_notes_cases(&snapshot, &mut cases);
    }
    assert!(cases.iter().all(|&c| c > 0), "{cases:?}");
    let mut sweep = [0; 5];
    (0..1000).for_each(|seed| {
        let snapshot = exercise::snapshot(seed).expect("valid");
        exercise::count_notes_cases(&snapshot, &mut sweep);
    });
    assert_eq!(sweep[0], 200, "every fifth seed is 0.2-labelled");
    assert!(sweep[1..].iter().all(|&c| c >= 200), "{sweep:?}");
}

#[test]
fn snapshots_differing_only_in_notes_hash_differently() {
    let bare = pipeline(4, 30, "grid").expect("runs").snapshot;
    let mut parts = bare.clone().into_parts();
    parts.notes = Notes::of(&[Note {
        code: NoteCode::PackingApproximate,
        index: SNAPSHOT_WIDE,
    }]);
    let noted = Snapshot::new(parts).expect("valid");
    assert_eq!(bare.parts().nodes, noted.parts().nodes);
    let (a, b) = (bare.to_bytes(), noted.to_bytes());
    assert_ne!(a, b);
    assert_ne!(sha256_hex(&a), sha256_hex(&b));
}

/// A grid snapshot of `n` nodes with node `moved`'s x shifted by `by`.
fn grid_with(n: u32, moved: usize, by: f32) -> Snapshot {
    let mut parts: SnapshotParts = pipeline(3, n, "grid").expect("runs").snapshot.into_parts();
    if let NodeGeometry::Point { x, .. } = &mut parts.nodes {
        x[moved] += by;
    }
    Snapshot::new(parts).expect("valid")
}

#[test]
fn the_hand_oracle_catches_a_moved_node_and_a_foreign_kind() {
    assert_eq!(grid_by_hand(&grid_with(5, 0, 0.0)), Ok(()));
    let err = grid_by_hand(&grid_with(5, 4, 0.5)).expect_err("moved");
    assert_eq!(
        err,
        "node 4 at (0.5, 0.5), the conventions put it at (0.0, 0.5)"
    );
    let tiny = grid_with(5, 2, f32::EPSILON);
    assert!(
        grid_by_hand(&tiny)
            .expect_err("one ulp off")
            .starts_with("node 2 ")
    );
    let exercise = exercise::snapshot(1).expect("valid");
    assert_eq!(exercise.header().node_kind, NodeGeometryKind::Circle);
    assert_eq!(exercise.header().edge_kind, EdgeGeometryKind::Line);
    let foreign = grid_by_hand(&exercise).expect_err("circles");
    assert_eq!(foreign, "not Point nodes with Line edges");
}

#[test]
fn overall_pass_requires_both_checks_clean_not_either_one() {
    assert!(all_clear(&Findings::default()));
    let only_faces_bad = Findings {
        faces: vec!["x".into()],
        grid: vec![],
    };
    assert!(!all_clear(&only_faces_bad), "faces alone must fail it");
    let only_grid_bad = Findings {
        faces: vec![],
        grid: vec!["y".into()],
    };
    assert!(!all_clear(&only_grid_bad), "grid alone must fail it");
}

#[test]
fn print_findings_subtracts_failures_from_the_total_not_adds() {
    let found = Findings {
        faces: vec!["a".into(), "b".into()],
        grid: vec!["c".into()],
    };
    let mut text = String::new();
    write_findings(&mut text, 5, &found);
    assert!(
        text.contains("binary <-> JSON byte-exact on 8/10 snapshots"),
        "{text}"
    );
    assert!(
        text.contains("layout.grid on its stated conventions on 4/5 seeds"),
        "{text}"
    );
}

#[test]
fn the_sweep_records_nothing_wrong_and_refuses_zero_seeds() {
    let found = sweep(12).expect("runs");
    assert!(found.faces.is_empty() && found.grid.is_empty(), "{found:?}");
    assert!(found.pass(), "every notes case drawn: {:?}", found.notes);
    assert!(
        !sweep(4).expect("runs").pass(),
        "four seeds cannot draw every case"
    );
    assert!(sweep(0).expect_err("empty").starts_with("0 seeds"));
}

fn scratch(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("gm-snapshot-{}-{name}", std::process::id()))
}

#[test]
fn snapshot_writes_the_faces_it_is_asked_for_and_nothing_else() {
    let (bin, json) = (scratch("x.bin"), scratch("x.json"));
    let both = Outputs {
        bin: Some(bin.clone()),
        json: Some(json.clone()),
    };
    let summary = write_faces(1, 50, "grid", &both).expect("writes");
    assert!(
        summary.starts_with("snapshot: seed 1, layout.grid, 50 nodes, "),
        "{summary}"
    );
    assert!(summary.contains("Point/Line, format 0.3\n"), "{summary}");
    let want = pipeline(1, 50, "grid").expect("runs").snapshot;
    assert_eq!(std::fs::read(&bin).expect("bin"), want.to_bytes());
    assert_eq!(
        std::fs::read_to_string(&json).expect("json"),
        to_json(&want)
    );
    for path in [&bin, &json] {
        std::fs::remove_file(path).expect("cleanup");
    }
    let json_only = Outputs {
        json: Some(json.clone()),
        ..Outputs::default()
    };
    write_faces(2, 3, "grid", &json_only).expect("writes");
    assert!(json.exists() && !bin.exists());
    std::fs::remove_file(&json).expect("cleanup");
}

#[test]
fn snapshot_refuses_no_output_two_stdouts_and_an_unwritable_path() {
    let none = write_faces(1, 5, "grid", &Outputs::default()).expect_err("nothing");
    assert!(none.starts_with("nothing to write"), "{none}");
    let dash = Some(PathBuf::from("-"));
    let two = Outputs {
        bin: dash.clone(),
        json: dash,
    };
    let err = write_faces(1, 5, "grid", &two).expect_err("two stdouts");
    assert_eq!(err, "only one face can go to standard output");
    let nowhere = Outputs {
        bin: Some(scratch("missing-dir").join("x.bin")),
        ..Outputs::default()
    };
    let err = write_faces(1, 5, "grid", &nowhere).expect_err("no directory");
    assert!(err.starts_with("writing "), "{err}");
}
