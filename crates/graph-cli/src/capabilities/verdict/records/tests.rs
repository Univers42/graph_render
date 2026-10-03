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

/// A directory with nothing in it is no records, and a record that cannot be parsed is
/// **absent rather than fatal**: it is named on stderr, skipped, and the row that needed it
/// reads "no `<name>` record: run the gate" — which is true, and is an answer about that row
/// instead of a whole-ledger failure. It used to be an error, so one half-copied file took
/// every row's verdict with it.
///
/// The control in the same test is the case that stays an error: a path that is a
/// **directory** is unreadable rather than unparseable, and a directory is not something a
/// reader may decide to skip.
#[test]
fn an_absent_directory_is_empty_and_an_unparseable_record_is_absent_not_fatal() {
    let missing = std::env::temp_dir().join("graph-cli-records-absent-dir");
    let _ = fs::remove_dir_all(&missing);
    assert_eq!(all(&missing).expect("no records"), Default::default());
    let (dir, _) = gates(&[
        ("oracle-osage.json", "not json at all"),
        ("oracle-twopi.json", r#"{"pass":true,"seeds":1000}"#),
    ]);
    let found = all(&dir).expect("read: one mangled file is not a whole-ledger failure");
    assert_eq!(
        found.keys().map(String::as_str).collect::<Vec<&str>>(),
        ["oracle-twopi"],
        "the mangled name resolves to nothing, and its neighbour still resolves"
    );
    assert!(
        !found.contains_key("oracle-osage"),
        "an unparseable record backs nothing, so no row can read a verdict out of it"
    );
    fs::create_dir(dir.join("a-directory.json")).expect("a directory named like a record");
    assert!(
        all(&dir).is_err(),
        "the control: unreadable is not unparseable, and is still an error"
    );
    fs::remove_dir_all(&dir).expect("removed");
    // A file that is not a `<name>.json` is not a record and is skipped rather than
    // refused: another tool's file in the gates directory is not a gate that went wrong.
    let (dir, _) = gates(&[("notes.txt", "hello"), ("oracle-twopi.json", "{}")]);
    let found = all(&dir).expect("read");
    assert_eq!(
        found.keys().map(String::as_str).collect::<Vec<&str>>(),
        ["oracle-twopi"],
        "and a file that parses but is the wrong shape is still a record — refused by \
         `current`, not skipped here"
    );
    fs::remove_dir_all(&dir).expect("removed");
}
