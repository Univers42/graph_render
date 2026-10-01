//! The gates-directory reader: every `<name>.json` by name, and nothing else.

use super::all;
use std::fs;

/// A staged gates directory holding `files`, and its own path.
fn gates(files: &[(&str, &str)]) -> (std::path::PathBuf, ()) {
    let dir = std::env::temp_dir().join(format!(
        "graph-cli-records-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("staged");
    for (name, body) in files {
        fs::write(dir.join(name), body).expect("written");
    }
    (dir, ())
}

#[test]
fn every_record_is_found_by_the_name_in_its_own_file_name() {
    let (dir, _) = gates(&[
        ("oracle-osage.json", r#"{"pass":true,"seeds":1000}"#),
        ("oracle-twopi.json", r#"{"pass":true,"seeds":1000}"#),
        ("hashgate.json", r#"{"pass":true,"seeds":1000}"#),
    ]);
    let found = all(&dir).expect("read");
    assert_eq!(
        found.keys().map(String::as_str).collect::<Vec<&str>>(),
        ["hashgate", "oracle-osage", "oracle-twopi"],
        "every record, in name order and not in the filesystem's order"
    );
    assert_eq!(found["oracle-osage"]["seeds"], 1000);
    fs::remove_dir_all(&dir).expect("removed");
}

/// A second engine is a file, not an edit: the reader has no table of names, so a record
/// it has never seen resolves like any other. This is the property the whole change rests
/// on, and the negative control on it is the next test.
#[test]
fn a_record_the_reader_has_never_been_told_about_resolves_like_any_other() {
    let (dir, _) = gates(&[("oracle-some-later-engine.json", r#"{"pass":true}"#)]);
    let found = all(&dir).expect("read");
    assert!(
        found.contains_key("oracle-some-later-engine"),
        "a name the reader was never given must still resolve"
    );
    fs::remove_dir_all(&dir).expect("removed");
}

/// A directory with nothing in it is no records, and a file that is not a record is
/// refused: a `.json` the ledger cannot parse is not a record it may report as absent,
/// and a passing verdict must never be read out of a file that is not one.
#[test]
fn an_absent_directory_is_empty_and_a_file_that_is_not_a_record_is_refused() {
    let missing = std::env::temp_dir().join("graph-cli-records-absent-dir");
    let _ = fs::remove_dir_all(&missing);
    assert_eq!(all(&missing).expect("no records"), Default::default());
    let (dir, _) = gates(&[("oracle-osage.json", "not json at all")]);
    assert!(
        all(&dir).is_err(),
        "an unparseable record must be an error, never a missing record"
    );
    fs::remove_dir_all(&dir).expect("removed");
    // A file that is not a `<name>.json` is not a record and is skipped rather than
    // refused: another tool's file in the gates directory is not a gate that went wrong.
    let (dir, _) = gates(&[("notes.txt", "hello"), ("oracle-twopi.json", "{}")]);
    let found = all(&dir).expect("read");
    assert_eq!(
        found.keys().map(String::as_str).collect::<Vec<&str>>(),
        ["oracle-twopi"]
    );
    fs::remove_dir_all(&dir).expect("removed");
}
