use super::*;
use crate::runner::sha256_hex;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("gm-evidence-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("sub")).expect("temp dir");
    dir
}

/// A written record's path: anything else is a failure of the test's own setup.
fn recorded(outcome: Outcome) -> PathBuf {
    match outcome {
        Outcome::Recorded(path) => path,
        other => panic!("expected a written record, got {other:?}"),
    }
}

/// A refused record's message: anything else is a failure of the test's own setup.
fn refused(outcome: Outcome) -> String {
    match outcome {
        Outcome::Refused(why) => why,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// The refusal a caller is handed, pinned as a literal: it is printed, so a wording
/// change is a change of the CLI's output and a caller matching on it would notice. It
/// names the record and says why the passing one stands.
const REFUSAL: &str = "hashgate: not recorded — the existing record passed and this run did \
     not, so the passing record stands (re-run the gate to replace it)";

/// The three outcomes of a record, and why only one of them is a run's own error: a
/// record written, a record refused because a passing one of the same name stands, and a
/// record that could not be written at all. A gate that ran and judged must read its
/// refusal as a warning — its exit code is its verdict — while a record that is missing
/// leaves that verdict unbacked, which is "could not run".
#[test]
fn a_refusal_is_a_warning_and_only_an_unwritable_record_is_an_error() {
    let dir = scratch("outcomes");
    let passed = serde_json::json!({ "seeds": 2000, "pass": true });
    let written = write_to(&dir, "hashgate", passed, "f".into());
    assert_eq!(written, Outcome::Recorded(dir.join("hashgate.json")));
    assert_eq!(
        handled(written),
        Ok(()),
        "a written record is nothing to report"
    );
    let short = serde_json::json!({ "seeds": 8, "pass": false });
    let refused = refused(write_to(&dir, "hashgate", short.clone(), "f".into()));
    assert_eq!(refused, REFUSAL);
    assert_eq!(
        handled(Outcome::Refused(refused.clone())),
        Ok(()),
        "the gate ran and judged: its refusal is a warning"
    );
    let unbacked = Outcome::Failed("a gate record is a JSON object".into());
    assert_eq!(
        handled(unbacked.clone()),
        Err("a gate record is a JSON object".into())
    );
    assert_eq!(
        write_to(&dir, "list", serde_json::json!([]), "f".into()),
        unbacked
    );
    assert_eq!(
        read_from(&dir, "hashgate")
            .expect("readable")
            .expect("present")["seeds"],
        2000,
        "the passing record stands, untouched"
    );
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

#[test]
fn the_fingerprint_moves_with_content_names_and_new_files_only() {
    let dir = scratch("fp");
    std::fs::write(dir.join("a.txt"), "one").expect("write");
    std::fs::write(dir.join("sub/b.txt"), "two").expect("write");
    let entries = ["a.txt", "sub"];
    let first = fingerprint_of(&dir, &entries).expect("fingerprint");
    assert_eq!(first, fingerprint_of(&dir, &entries).expect("again"));
    std::fs::write(dir.join("sub/b.txt"), "tw0").expect("write");
    let edited = fingerprint_of(&dir, &entries).expect("edited");
    assert_ne!(first, edited);
    std::fs::write(dir.join("sub/c.txt"), "").expect("write");
    assert_ne!(edited, fingerprint_of(&dir, &entries).expect("added"));
    assert!(fingerprint_of(&dir, &["missing"]).is_err());
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

#[test]
fn the_listing_is_path_nul_digest_lines_in_byte_order() {
    let dir = scratch("listing");
    std::fs::write(dir.join("sub/b"), "x").expect("write");
    std::fs::write(dir.join("a"), "").expect("write");
    let empty = sha256_hex(b"");
    let x = sha256_hex(b"x");
    let want = sha256_hex(format!("a\0{empty}\nsub/b\0{x}\n").as_bytes());
    assert_eq!(fingerprint_of(&dir, &["sub", "a"]), Ok(want));
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

/// A record that exists but cannot be parsed **backs nothing**: it reads as *absent*, not
/// as a fatal error, and it is named on stderr rather than deleted. The review's complaint
/// was that one truncated file stopped `capabilities --check` for the whole tree — 72 rows
/// read as unbacked because a `serde_json` error came out of the loader. With the atomic
/// write no record this crate writes can be torn any more, but a half-copied or
/// hand-mangled one still can, and it must cost its own row its evidence rather than every
/// row's.
///
/// The control in the same test is the case that must **stay** an error: a path that is a
/// directory is *unreadable*, not unparseable, and that is a real I/O failure to report.
#[test]
fn an_unparseable_record_reads_as_absent_and_a_re_run_repairs_it() {
    let dir = scratch("corrupt");
    let path = dir.join("hashgate.json");
    std::fs::write(&path, "{ \"pass\": true, \"seeds\":").expect("a half-copied record");
    assert_eq!(
        read_from(&dir, "hashgate"),
        Ok(None),
        "a record nobody can parse is not evidence, and is not a fatal error either"
    );
    assert!(
        path.exists(),
        "and it stays where it is, for the person who has to look at it"
    );
    assert!(
        matches!(
            write_to(
                &dir,
                "hashgate",
                serde_json::json!({ "seeds": 8, "pass": true }),
                "f".into()
            ),
            Outcome::Recorded(_)
        ),
        "a re-run is not blocked by it"
    );
    assert_eq!(
        read_from(&dir, "hashgate")
            .expect("readable")
            .expect("present")["seeds"],
        8,
        "so re-running the gate repairs it"
    );
    std::fs::create_dir(dir.join("dir.json")).expect("dir");
    assert!(
        read_from(&dir, "dir").is_err(),
        "the control: unreadable is still an error, not an absence"
    );
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

#[test]
fn a_record_reads_back_as_written_and_only_absence_is_none() {
    let dir = scratch("records");
    let body = serde_json::json!({ "seeds": 3, "pass": true });
    let path = recorded(write_to(&dir, "gate", body, "f".into()));
    assert_eq!(path, dir.join("gate.json"));
    let record = read_from(&dir, "gate").expect("readable").expect("present");
    assert_eq!(
        record,
        serde_json::json!({ "seeds": 3, "pass": true, "gate": "gate", "fingerprint": "f" })
    );
    assert_eq!(read_from(&dir, "absent"), Ok(None));
    std::fs::write(dir.join("torn.json"), "{").expect("write");
    assert_eq!(
        read_from(&dir, "torn"),
        Ok(None),
        "a torn record is no record (its own line names it on stderr)"
    );
    std::fs::create_dir(dir.join("dir.json")).expect("dir");
    assert!(read_from(&dir, "dir").is_err(), "unreadable is not absent");
    assert!(matches!(
        write_to(&dir, "list", serde_json::json!([]), "f".into()),
        Outcome::Failed(_)
    ));
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

#[test]
fn a_failing_run_never_overwrites_a_passing_record() {
    let dir = scratch("no-clobber");
    let passed = serde_json::json!({ "seeds": 1000, "pass": true });
    let path = recorded(write_to(&dir, "hashgate", passed, "f".into()));
    let kept = std::fs::read_to_string(&path).expect("readable");
    // A control run, a short sweep, or a run on a tree that has since regressed:
    // each writes the same record name, and each must leave the passing one standing.
    let why = refused(write_to(
        &dir,
        "hashgate",
        serde_json::json!({ "seeds": 8, "pass": false }),
        "f".into(),
    ));
    assert!(why.contains("hashgate"), "{why}");
    assert_eq!(
        std::fs::read_to_string(&path).expect("readable"),
        kept,
        "the passing record is byte-for-byte what it was"
    );
    // The reverse is allowed and is the point of a gate: a passing run replaces a
    // failing one of the same name, which is what re-running for a fix must do.
    recorded(write_to(
        &dir,
        "roundtrip",
        serde_json::json!({ "seeds": 8, "pass": false }),
        "f".into(),
    ));
    recorded(write_to(
        &dir,
        "roundtrip",
        serde_json::json!({ "seeds": 2000, "pass": true }),
        "f".into(),
    ));
    assert_eq!(
        read_from(&dir, "roundtrip").expect("readable"),
        Some(serde_json::json!({
            "seeds": 2000, "pass": true, "gate": "roundtrip", "fingerprint": "f"
        }))
    );
    // A record with no boolean `pass` is not a verdict and is not written at all, so it
    // can neither replace a passing record nor stand as one. (The oracle harnesses used
    // to write exactly that shape; every writer in `graph-cli` sets a real boolean today,
    // so this is a refusal and not a lost record.)
    assert!(
        matches!(
            write_to(
                &dir,
                "oracle-diff",
                serde_json::json!({ "seeds": 1000 }),
                "f".into()
            ),
            Outcome::Failed(_)
        ),
        "a body with no `pass` is not a record"
    );
    for not_a_verdict in [
        serde_json::json!({ "pass": null }),
        serde_json::json!({ "pass": "false" }),
    ] {
        assert!(
            matches!(
                write_to(&dir, "oracle-diff", not_a_verdict.clone(), "f".into()),
                Outcome::Failed(_)
            ),
            "{not_a_verdict} is not a verdict"
        );
        assert_eq!(
            read_from(&dir, "oracle-diff").expect("readable"),
            None,
            "nothing was written for {not_a_verdict}"
        );
    }
    // The control, in the same guard: a real failing record *does* replace a failing one.
    recorded(write_to(
        &dir,
        "oracle-diff",
        serde_json::json!({ "seeds": 1000, "pass": false }),
        "f".into(),
    ));
    recorded(write_to(
        &dir,
        "oracle-diff",
        serde_json::json!({ "seeds": 8, "pass": false }),
        "f".into(),
    ));
    assert_eq!(
        read_from(&dir, "oracle-diff").expect("readable"),
        Some(serde_json::json!({
            "seeds": 8, "pass": false, "gate": "oracle-diff", "fingerprint": "f"
        }))
    );
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

/// A passing record another tree left behind is already void to the ledger, so it must
/// not stand in the way of this tree's failing run: refusing the write would leave
/// exactly that void record as the file `capabilities --check` reads.
#[test]
fn a_stale_passing_record_from_another_tree_does_not_block_this_trees_run() {
    let dir = scratch("other-tree");
    recorded(write_to(
        &dir,
        "hashgate",
        serde_json::json!({ "seeds": 2000, "pass": true }),
        "other".into(),
    ));
    recorded(write_to(
        &dir,
        "hashgate",
        serde_json::json!({ "seeds": 8, "pass": false }),
        "this".into(),
    ));
    assert_eq!(
        read_from(&dir, "hashgate")
            .expect("readable")
            .expect("present"),
        serde_json::json!({ "seeds": 8, "pass": false, "gate": "hashgate", "fingerprint": "this" }),
        "this tree's own run is recorded; the other tree's record is gone"
    );
    // Same tree, and the rule still holds: a failing run never replaces a passing record.
    let kept = scratch("same-tree");
    recorded(write_to(
        &kept,
        "hashgate",
        serde_json::json!({ "seeds": 2000, "pass": true }),
        "this".into(),
    ));
    assert!(
        refused(write_to(
            &kept,
            "hashgate",
            serde_json::json!({ "seeds": 8, "pass": false }),
            "this".into(),
        ))
        .contains("hashgate")
    );
    for dir in [&dir, &kept] {
        std::fs::remove_dir_all(dir).expect("cleanup");
    }
}

/// The write is a temporary file renamed into place, so a reader sees the old record or
/// the new one and never half of either — and no temporary is left behind.
#[test]
fn a_record_is_renamed_into_place_and_leaves_no_temporary() {
    let dir = scratch("atomic");
    let path = recorded(write_to(
        &dir,
        "gate",
        serde_json::json!({ "seeds": 3, "pass": true }),
        "f".into(),
    ));
    std::fs::write(&path, "torn by a killed writer").expect("writable");
    recorded(write_to(
        &dir,
        "gate",
        serde_json::json!({ "seeds": 4, "pass": true }),
        "f".into(),
    ));
    let temporaries: Vec<String> = std::fs::read_dir(&dir)
        .expect("readable")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.ends_with(".tmp"))
        .collect();
    assert!(
        temporaries.is_empty(),
        "no temporary survives: {temporaries:?}"
    );
    assert_eq!(
        read_from(&dir, "gate").expect("readable").expect("present")["seeds"],
        4,
        "the new record replaced the torn one whole"
    );
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

#[test]
fn a_stamp_needs_the_built_tree_and_goes_stale_when_the_tree_moves() {
    let stamp = Stamp::against("abc", "abc".into()).expect("same tree");
    assert_eq!(stamp.fingerprint(), "abc");
    assert_eq!(stamp.unchanged("abc"), Ok(()));
    let stale = Stamp("0".repeat(64))
        .still_current()
        .expect_err("not this tree");
    assert!(stale.contains("changed during the run"), "{stale}");
    let moved = stamp.unchanged("abd").expect_err("moved");
    assert!(moved.contains("changed during the run"), "{moved}");
    let rebuilt = Stamp::against("abc", "abd".into()).expect_err("other tree");
    assert!(rebuilt.contains("rebuild before recording"), "{rebuilt}");
}

#[test]
fn the_real_tree_is_the_one_this_binary_was_built_from() {
    let fingerprint = tree_fingerprint().expect("every root exists");
    assert_eq!(fingerprint.len(), 64);
    let stamp = Stamp::take().expect("built from this tree");
    assert_eq!(stamp.fingerprint(), BUILT_FROM);
    assert_eq!(stamp.still_current(), Ok(()));
}
