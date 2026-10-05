//! The manifest reader and its writer — everything a plugin declares before it sends a
//! record. Whether a *second* manifest may say something new about the first is
//! `growth.rs`.

use super::super::manifest::{manifest_json, read_manifest};
use super::super::*;
use super::support::TWO;

#[test]
fn a_two_collection_manifest_reads_sorted_with_its_link_targets_qualified() {
    let m = read_manifest(TWO, "tracker").expect("the manifest reads");
    assert_eq!(m.version, 1);
    assert_eq!(m.name, "Tasks");
    let ids: Vec<&str> = m.collections.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["note", "task"], "collections sort by id");
    let fields: Vec<&str> = m.collections[1]
        .fields
        .iter()
        .map(|f| f.id.as_str())
        .collect();
    assert_eq!(fields, ["name", "peer", "up"], "fields sort by id");
    // `note` is this plugin's own collection, so the manifest may spell it bare; it is
    // stored qualified so a document from two plugins can hold both.
    assert_eq!(
        m.collections[1].fields[1].link.as_ref().map(|l| l.collection.as_str()),
        Some("tracker.note")
    );
}

/// An already-qualified target is kept, and one with two dots is refused: the split is
/// on the *first* dot, so a target that could be split two ways is a mistake.
#[test]
fn a_link_target_is_qualified_once_and_only_once() {
    let one = read_manifest(TWO.replace("\"collection\": \"note\"", "\"collection\": \"other.c\""), "tracker")
        .expect("a qualified target reads");
    assert_eq!(
        one.collections[1].fields[1].link.as_ref().map(|l| l.collection.clone()),
        Some("other.c".to_owned()),
        "an already-qualified target must not be qualified twice"
    );
    let bad = TWO.replace("\"collection\": \"note\"", "\"collection\": \"a.b.c\"");
    assert_eq!(
        read_manifest(&bad, "tracker").unwrap_err().to_string(),
        "collections[1].fields[1].link.collection: `a.b.c` is not one qualified id"
    );
}

/// Every refusal, with the message it gives. One test because the point is that each
/// one says *where*, and a table of (text, message) keeps a reader from having to run
/// the suite to learn what a refusal looks like.
#[test]
fn a_manifest_that_breaks_the_contract_is_refused_naming_the_path() {
    let cases = [
        (
            "unknown member",
            TWO.replace(r#""name": "Tasks","#, r#""name": "Tasks","extra": 1,"#),
            "the manifest: unknown member `extra`",
        ),
        (
            "version 2",
            TWO.replace(r#""manifestVersion": 1"#, r#""manifestVersion": 2"#),
            "manifestVersion: unsupported hub manifest version 2",
        ),
        (
            "duplicate collection id",
            TWO.replace(
                r#"{ "id": "note", "name": "Notes""#,
                r#"{ "id": "task", "name": "Notes""#,
            ),
            "collections[0].id: duplicate collection id `task`",
        ),
        (
            "duplicate field id",
            TWO.replace(
                r#"{ "id": "up", "name": "Up", "role": "parent", "link": null }"#,
                r#"{ "id": "name", "name": "Up", "role": "parent", "link": null }"#,
            ),
            "collections[1].fields[2].id: duplicate field id `name`",
        ),
        (
            "missing title field",
            TWO.replace(r#""titleField": "name""#, r#""titleField": "nope""#),
            "collections[1].titleField: no field with id `nope`",
        ),
        (
            "a link to a collection the manifest does not declare",
            TWO.replace(r#""collection": "note""#, r#""collection": "gone""#),
            "collections[1].fields[1].link.collection: link target `tracker.gone` is not \
             declared by this manifest",
        ),
        (
            "NUL in a name",
            TWO.replace(r#""name": "Tasks","#, r#""name": "Ta\u0000sks","#),
            "name: a NUL character",
        ),
    ];
    for (what, text, message) in cases {
        assert_eq!(
            read_manifest(&text, "tracker").unwrap_err().to_string(),
            message,
            "{what}"
        );
    }
}

/// The three manifest caps, each refused as a size and not as a shape: the client has
/// to know which of its own numbers to lower.
#[test]
fn a_manifest_over_a_cap_is_refused_as_a_size() {
    let many = |count: usize| {
        let mut collections = String::new();
        for i in 0..count {
            if i > 0 {
                collections.push(',');
            }
            collections.push_str(&format!(
                r#"{{"id":"c{i}","name":"C{i}","titleField":"t","fields":[
                    {{"id":"t","name":"T","role":"title","link":null}}]}}"#
            ));
        }
        format!(r#"{{"manifestVersion":1,"name":"n","collections":[{collections}]}}"#)
    };
    assert!(read_manifest(&many(64), "tracker").is_ok());
    assert_eq!(
        read_manifest(&many(65), "tracker").unwrap_err(),
        HubError::TooLarge {
            what: "collections",
            limit: MAX_COLLECTIONS
        }
    );
    let fields = |count: usize| {
        let mut text = String::from(
            r#"{"manifestVersion":1,"name":"n","collections":[{"id":"c","name":"C","titleField":"t","fields":["#,
        );
        for i in 0..count {
            if i > 0 {
                text.push(',');
            }
            text.push_str(&format!(
                r#"{{"id":"f{i}","name":"F","role":"scalar","link":null}}"#
            ));
        }
        text.push_str(r#"]}]}"#);
        text
    };
    assert!(read_manifest(&fields(256), "tracker").is_ok());
    assert_eq!(
        read_manifest(&fields(257), "tracker").unwrap_err(),
        HubError::TooLarge {
            what: "fields",
            limit: MAX_FIELDS
        }
    );
    let huge = format!(
        r#"{{"manifestVersion":1,"name":"{}","collections":[]}}"#,
        "n".repeat(MAX_MANIFEST_BYTES as usize)
    );
    assert_eq!(
        read_manifest(&huge, "tracker").unwrap_err(),
        HubError::TooLarge {
            what: "manifest",
            limit: MAX_MANIFEST_BYTES
        }
    );
}

/// The writer and the reader are two directions of one format, held to that the only
/// way they can be: read what the writer wrote.
#[test]
fn what_the_writer_writes_reads_back_to_the_same_manifest() {
    let first = read_manifest(TWO, "tracker").unwrap();
    let text = manifest_json(&first);
    assert!(!text.ends_with('\n'), "a manifest body has no trailing newline");
    let second = read_manifest(&text, "tracker").expect("the writer's own output reads");
    assert_eq!(first, second);
    assert_eq!(manifest_json(&second), text);
    // Keys in byte order, like every canonical text this crate writes.
    let collections = text.find("\"collections\"").unwrap();
    let manifest_version = text.find("\"manifestVersion\"").unwrap();
    let name = text.find("\"name\"").unwrap();
    let version = text.find("\"version\"").unwrap();
    assert!(
        collections < manifest_version
            && manifest_version < name
            && name < version,
        "{text}"
    );
}
