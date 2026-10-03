use super::*;
use graph_contract::binary::SnapshotParts;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometry, NodeGeometryKind};
use graph_contract::notes::{Note, NoteCode, Notes, SNAPSHOT_WIDE};
use std::collections::BTreeSet;

mod dim;

/// The whole list, exactly: one pair per registered layout, in registry order, with no
/// name offered twice — a new layout has to appear here or this goes red.
#[test]
fn layout_names_offers_every_registered_layout_once_by_both_of_its_names() {
    let names = layout_names();
    assert_eq!(
        names,
        [
            "layout.grid",
            "grid",
            "layout.tree.tidy",
            "tree.tidy",
            "layout.treemap.squarified",
            "treemap.squarified",
            "layout.circular.radial",
            "circular.radial",
            "layout.packing.circle",
            "packing.circle",
            "layout.spectral",
            "spectral",
            "layout.mds.pivot",
            "mds.pivot",
            "layout.force.barnes_hut",
            "force.barnes_hut",
            "layout.forceatlas2",
            "forceatlas2",
            "layout.dag.sugiyama",
            "dag.sugiyama",
            "layout.random",
            "random",
            "layout.circular.ring",
            "circular.ring",
            "layout.spiral",
            "spiral",
            "layout.bipartite",
            "bipartite",
            "layout.force.yifan_hu",
            "force.yifan_hu",
            "layout.force.fruchterman_reingold",
            "force.fruchterman_reingold",
            "layout.force.kamada_kawai",
            "force.kamada_kawai",
            "layout.force.graphopt",
            "force.graphopt",
            "layout.force.davidson_harel",
            "force.davidson_harel",
            "layout.force.lgl",
            "force.lgl",
            "layout.force.drl",
            "force.drl",
            "layout.twopi",
            "twopi",
            "layout.packing.osage",
            "packing.osage",
            "layout.force.spring",
            "force.spring",
            "layout.circular.hierarchy",
            "circular.hierarchy",
            "layout.circular.circo",
            "circular.circo",
            "layout.treemap.patchwork",
            "treemap.patchwork",
            "layout.force.neato",
            "force.neato",
            "layout.force.fdp",
            "force.fdp",
            "layout.basic3d.sphere",
            "basic3d.sphere",
            "layout.basic3d.helix",
            "basic3d.helix",
            "layout.basic3d.cube",
            "basic3d.cube",
            "layout.hierarchical3d",
            "hierarchical3d",
            "layout.force.spring3d",
            "force.spring3d",
            "layout.force.sfdp",
            "force.sfdp",
            "layout.forceatlas2.barnes_hut",
            "forceatlas2.barnes_hut",
            "layout.bipartite_3d",
            "bipartite_3d",
            "layout.basic3d.spiral",
            "basic3d.spiral",
            "layout.force.particle_mesh",
            "force.particle_mesh",
            "layout.forceatlas2.forcesim",
            "forceatlas2.forcesim",
        ]
    );
    let mut once = names.clone();
    once.sort_unstable();
    once.dedup();
    assert_eq!(
        once.len(),
        names.len(),
        "no layout is offered twice: {names:?}"
    );
    for id in registry::LAYOUTS.iter().map(|l| l.id) {
        assert!(names.contains(&id), "{names:?} missing {id}");
        assert!(
            names.contains(&short_name(id)),
            "{names:?} missing {id}'s short name"
        );
    }
    assert_eq!(short_name("layout.grid"), "grid");
    assert_eq!(
        short_name("grid"),
        "grid",
        "an id already short stays as it is"
    );
}

#[test]
fn each_layout_name_runs_the_same_pipeline_and_an_unregistered_one_names_all_the_rest() {
    for (id, short) in [
        ("layout.grid", "grid"),
        ("layout.tree.tidy", "tree.tidy"),
        ("layout.treemap.squarified", "treemap.squarified"),
        ("layout.circular.radial", "circular.radial"),
        ("layout.packing.circle", "packing.circle"),
        ("layout.spectral", "spectral"),
        ("layout.mds.pivot", "mds.pivot"),
        ("layout.random", "random"),
        ("layout.circular.ring", "circular.ring"),
        ("layout.spiral", "spiral"),
        ("layout.bipartite", "bipartite"),
        ("layout.force.yifan_hu", "force.yifan_hu"),
        ("layout.twopi", "twopi"),
        ("layout.circular.circo", "circular.circo"),
    ] {
        let by_short = pipeline(1, 50, short).expect("runs by short name");
        let by_id = pipeline(1, 50, id).expect("runs by full id");
        assert_eq!(
            (by_short.layout, by_short.snapshot.header().node_count),
            (id, 50)
        );
        assert_eq!(by_short.snapshot, by_id.snapshot);
    }
    let err = pipeline(1, 50, "helix").expect_err("unregistered");
    assert_eq!(
        err,
        format!("no layout \"helix\": one of {}", layout_names().join(", "))
    );
}

#[test]
fn both_faces_round_trip_on_the_grid_and_on_the_exercise() {
    for seed in 0..40 {
        let grid = pipeline(seed, gate_node_count(seed), "grid")
            .expect("runs")
            .snapshot;
        assert_eq!(faces_agree(&grid), Ok(()), "grid seed {seed}");
        assert_eq!(hand_oracles::grid(&grid), Ok(()), "grid seed {seed}");
        let layered = pipeline(seed, gate_node_count(seed), "dag.sugiyama")
            .expect("runs")
            .snapshot;
        assert_eq!(faces_agree(&layered), Ok(()), "layered seed {seed}");
        assert_eq!(dag::invariants(&layered), Ok(()), "layered seed {seed}");
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
    // Exact, not a floor: a case drawn only on its own seed's `seed % 5` would still
    // clear 200 for two of these, and the gate's claim is that every case is drawn. The
    // first two are 67 lower and higher than before 3D seeds arrived, since a `dim = 1`
    // snapshot cannot be 0.2-labelled; the three code columns have not moved.
    assert_eq!(sweep, [133, 324, 342, 343, 200], "the five notes cases");
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
    assert_eq!(hand_oracles::grid(&grid_with(5, 0, 0.0)), Ok(()));
    let err = hand_oracles::grid(&grid_with(5, 4, 0.5)).expect_err("moved");
    assert_eq!(
        err,
        "node 4 at (0.5, 0.5), the conventions put it at (0.0, 0.5)"
    );
    let tiny = grid_with(5, 2, f32::EPSILON);
    assert!(
        hand_oracles::grid(&tiny)
            .expect_err("one ulp off")
            .starts_with("node 2 ")
    );
    let exercise = exercise::snapshot(1).expect("valid");
    assert_eq!(exercise.header().node_kind, NodeGeometryKind::Circle);
    assert_eq!(exercise.header().edge_kind, EdgeGeometryKind::Line);
    let foreign = hand_oracles::grid(&exercise).expect_err("circles");
    assert_eq!(foreign, "not Point nodes with Line edges");
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
