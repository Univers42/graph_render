//! Growth: the rule that a manifest may only add to what is already stored.
//!
//! Split out of `manifest.rs` by the house's 300-line limit, and because it is a
//! different question: `manifest.rs` asks "what is a manifest", this asks "what may a
//! second manifest say about the first".

use super::super::manifest::{Growth, growth, read_manifest};
use super::support::{BUMP, TWO, at, v1};

/// Growth, the whole rule in one test: manifests only grow, the same version is either a
/// no-op or a conflict, and a lower version is a conflict.
#[test]
fn growth_is_only_adding_or_nothing_at_all() {
    let first = v1();
    // Same version, same content: nothing to do, and *not* a conflict — a client that
    // retries an identical manifest must not be told it is wrong.
    assert_eq!(growth(&first, &first), Ok(Growth::Same));
    // Same version, other content: 409. The version is the client's own promise that this
    // manifest is a *different* one; breaking that promise is a conflict, not an addition.
    let renamed = TWO.replace(r#""name": "Tasks","#, r#""name": "Work","#);
    assert_eq!(
        growth(&first, &at(1, &renamed)).unwrap_err().status(),
        409
    );
    // A lower version is never growth.
    assert_eq!(growth(&first, &at(0, TWO)).unwrap_err().status(), 409);

    // v2 adds a collection, adds a field, and renames the manifest: all growth. A name is
    // a label the motor never derives from, so changing it costs nothing.
    let cases = [
        (
            "an added collection",
            TWO.replace(
                r#""collections": ["#,
                r#""collections": [ { "id": "aaa", "name": "A", "titleField": "t", "fields": [
                    { "id": "t", "name": "T", "role": "title", "link": null } ] }, "#,
            ),
        ),
        (
            "an added field",
            TWO.replace(
                r#"{ "id": "up", "name": "Up", "role": "parent", "link": null }"#,
                r#"{ "id": "up", "name": "Up", "role": "parent", "link": null },
            { "id": "zz", "name": "Z", "role": "scalar", "link": null }"#,
            ),
        ),
        ("a renamed manifest", renamed),
    ];
    for (what, text) in cases {
        assert_eq!(growth(&first, &at(2, &text)), Ok(Growth::Grown), "{what}");
    }
}

/// A field or a collection that goes away, or changes what it means, is the one change
/// growth cannot absorb: the records already stored carry its cells, and a hub that
/// silently dropped them would answer a read with a document the client cannot round
/// trip. Every one of them is a 409.
#[test]
fn a_removed_or_changed_declaration_is_a_conflict() {
    let cases = [
        (
            "a removed field",
            TWO.replace(
                r#",
            { "id": "up", "name": "Up", "role": "parent", "link": null }"#,
                "",
            ),
        ),
        (
            "a changed role",
            TWO.replace(
                r#"{ "id": "up", "name": "Up", "role": "parent", "link": null }"#,
                r#"{ "id": "up", "name": "Up", "role": "scalar", "link": null }"#,
            ),
        ),
        (
            "a removed collection",
            TWO.replace(
                r#"{ "id": "note", "name": "Notes", "titleField": "title",
      "fields": [ { "id": "title", "name": "Title", "role": "title", "link": null } ] },"#,
                "",
            ),
        ),
    ];
    for (what, text) in cases {
        let outcome = growth(&v1(), &at(2, &text));
        assert_eq!(
            outcome.as_ref().map(|_| ()).unwrap_err().status(),
            409,
            "{what} must be a conflict, got {outcome:?}"
        );
    }
}

/// A link that starts pointing somewhere else is the sharpest case, and it is worth its
/// own line: the stored records hold *old* references under that field, so re-pointing it
/// would leave every one of them dangling with nothing saying so.
#[test]
fn a_repointed_link_is_a_conflict() {
    let text = TWO.replace(
        r#""link": { "collection": "note", "cardinality": "one", "symmetric": false }"#,
        r#""link": { "collection": "task", "cardinality": "one", "symmetric": false }"#,
    );
    assert_eq!(growth(&v1(), &at(2, &text)).unwrap_err().status(), 409);
}

/// A field's `link` member may only appear or disappear with its role, which the reader
/// already refuses to let happen — so the growth check has nothing to add, and says so
/// rather than re-deciding it.
#[test]
fn the_version_member_is_what_growth_compares() {
    // Two manifests at the same version with different content are a conflict even when
    // the difference is only a name: the promise is the version, and a promise broken is
    // not growth however small the change.
    let first = v1();
    let second = at(1, &TWO.replace(r#""name": "Tasks","#, r#""name": "Work","#));
    assert_ne!(first, second);
    assert_eq!(growth(&first, &second).unwrap_err().status(), 409);
    assert!(read_manifest(TWO, "tracker").is_ok());
}