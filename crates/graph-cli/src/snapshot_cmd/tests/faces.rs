//! What `snapshot` writes to disk: the faces it is asked for, and the outputs it refuses.
//!
//! Split out of `tests.rs` by the house's 300-line limit; a pure move.

use super::*;

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
