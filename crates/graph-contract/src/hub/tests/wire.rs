//! The ids, the cursor, the caps and the seven refusals — the parts of the hub wire
//! that are not a document.

use super::super::strict::{child, parse_strict};
use super::super::*;
use crate::canonical_json::JsonError;
use crate::ingest::IngestError;

const SLUG63: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// Every grammar's own edges: what each id checker accepts, so a checker that refuses
/// the legal edge (a 63-char slug, a mixed-case collection id) is as wrong as one that
/// accepts past it.
#[test]
fn each_id_checker_accepts_its_grammar() {
    assert_eq!(check_workspace_id("a"), Ok(()));
    assert_eq!(check_plugin_id("0"), Ok(()));
    assert_eq!(check_workspace_id(SLUG63), Ok(()));
    assert_eq!(check_plugin_id("a-b-0"), Ok(()));
    assert_eq!(check_collection_id("A_b-9"), Ok(()));
    assert_eq!(check_collection_id(&"a".repeat(64)), Ok(()));
    // A record id is opaque: only empty and `:` are refused.
    assert_eq!(check_record_id("a b/c.d"), Ok(()));
}

/// Every grammar's first refusal, named by the coordinate it reports: a refusal that
/// does not say *which* id is unactionable, since one manifest holds several.
#[test]
fn each_id_checker_refuses_outside_its_grammar_and_names_the_coordinate() {
    let cases: [(&str, &str, Result<(), HubError>); 9] = [
        ("workspace id", "", check_workspace_id("")),
        ("plugin id", "-a", check_plugin_id("-a")),
        ("workspace id", "A", check_workspace_id("A")),
        (
            "workspace id",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            check_workspace_id(&format!("{SLUG63}a")),
        ),
        ("collection id", "", check_collection_id("")),
        ("collection id", "B.coll", check_collection_id("B.coll")),
        (
            "collection id",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            check_collection_id(&"a".repeat(65)),
        ),
        ("record id", "", check_record_id("")),
        ("record id", "a:b", check_record_id("a:b")),
    ];
    for (coordinate, text, refused) in cases {
        match refused {
            Err(HubError::Grammar {
                coordinate: got,
                value,
            }) => {
                assert_eq!(got, coordinate);
                assert_eq!(value, text, "{coordinate} lost the offending text");
            }
            other => panic!("{coordinate}: expected a grammar refusal, got {other:?}"),
        }
    }
}

#[test]
fn a_qualified_id_is_the_two_ids_and_one_dot() {
    assert_eq!(qualify("tracker", "task"), "tracker.task");
    // The dot cannot be ambiguous: neither grammar contains one, so splitting on the
    // first dot is the split. Pinned so the format cannot drift to two spellings.
    let qualified = qualify("a-b", "c");
    let (plugin, collection) = qualified.split_once('.').unwrap();
    assert_eq!((plugin, collection), ("a-b", "c"));
    assert_eq!(check_plugin_id(plugin), Ok(()));
    assert_eq!(check_collection_id(collection), Ok(()));
}

#[test]
fn a_cursor_is_an_epoch_and_a_seq() {
    assert_eq!(Cursor::parse("3.118"), Ok(Cursor { epoch: 3, seq: 118 }));
    assert_eq!(Cursor::parse("0.0"), Ok(Cursor { epoch: 0, seq: 0 }));
}

#[test]
fn a_cursor_writes_back_exactly_what_it_read() {
    for text in ["0.0", "3.118", "9007199254740991.0"] {
        assert_eq!(Cursor::parse(text).unwrap().to_string(), text);
    }
}

/// Every way a cursor is not a cursor. A leading zero in particular: `"03.1"` and
/// `"3.1"` name the same position in two texts, and a cursor the caller can spell two
/// ways cannot be compared as text.
#[test]
fn a_cursor_that_is_not_one_is_refused_whole() {
    for text in [
        "118", "03.1", "1.9007199254740992", "1.2.3", "1.", "+1.2", "", ".", "-1.2", "1.2 ",
        "0x1.2", "1..2",
    ] {
        assert_eq!(
            Cursor::parse(text),
            Err(HubError::Cursor {
                text: text.to_owned()
            }),
            "{text:?}"
        );
    }
}

/// A NUL in a **string**, at the root and nested: spec — `\u0000` anywhere in a body,
/// key or string, is 422.
#[test]
fn a_nul_in_a_string_is_refused_with_its_path() {
    assert_eq!(
        parse_strict("{\"a\":\"x\\u0000\"}", 4096, "body"),
        Err(HubError::Nul {
            path: "a".to_owned()
        })
    );
    assert_eq!(
        parse_strict("{\"a\":{\"b\":[\"y\\u0000\"]}}", 4096, "body"),
        Err(HubError::Nul {
            path: "a.b[0]".to_owned()
        })
    );
}

/// A NUL in a **key**, at the root and nested. This is the case the walk exists for and
/// the one a reader that only checks string values misses: a store that keys a column
/// or a file by name is truncated exactly the way it is truncated by a NUL in the value.
///
/// The refusal names the *object* holding the key, never the key itself: a path that
/// contains the NUL is a path a caller cannot type into a reader to find the fault.
#[test]
fn a_nul_in_a_key_is_refused_at_the_path_of_the_object_that_holds_it() {
    assert_eq!(
        parse_strict("{\"k\\u0000\":1}", 4096, "body"),
        Err(HubError::Nul { path: String::new() }),
        "at the root there is no path"
    );
    assert_eq!(
        parse_strict("{\"a\":{\"k\\u0000\":1}}", 4096, "body"),
        Err(HubError::Nul { path: "a".to_owned() })
    );
}

/// The cap is on the *bytes*, decided before the parse, so an oversized body costs a
/// length and not a parse.
#[test]
fn a_body_one_byte_over_its_cap_is_refused_as_a_size() {
    let exact = format!("\"{}\"", "x".repeat(62));
    assert_eq!(exact.len(), 64, "the cap is on the bytes, quotes included");
    assert!(parse_strict(&exact, 64, "body").is_ok());
    assert_eq!(
        parse_strict(&exact, 63, "body"),
        Err(HubError::TooLarge {
            what: "body",
            limit: 63
        })
    );
}

/// A duplicate key is the ingest parser's refusal, kept whole: it is not this
/// contract's to re-derive, and a flattened message would lose the byte it names.
#[test]
fn a_duplicate_key_is_the_ingest_readers_own_refusal() {
    assert_eq!(
        parse_strict(r#"{"a":1,"a":2}"#, 4096, "body"),
        Err(HubError::Shape(IngestError::Json(JsonError::Syntax {
            at: 7,
            what: "a key repeated in one object"
        })))
    );
}

/// Every variant's class, in one table — the mapping a server would otherwise write
/// once per handler.
#[test]
fn every_refusal_answers_the_status_its_reason_implies() {
    let cases: [(HubError, u16); 7] = [
        (
            HubError::Shape(IngestError::Json(JsonError::Syntax { at: 0, what: "x" })),
            422,
        ),
        (
            HubError::Invalid {
                path: String::new(),
                what: String::new(),
            },
            422,
        ),
        (
            HubError::Grammar {
                coordinate: "workspace id",
                value: String::new(),
            },
            422,
        ),
        (HubError::Nul { path: String::new() }, 422),
        (HubError::TooLarge { what: "body", limit: 0 }, 413),
        (
            HubError::Conflict {
                what: String::new(),
            },
            409,
        ),
        (HubError::Cursor { text: String::new() }, 400),
    ];
    for (mut error, status) in cases {
        assert_eq!(error.status(), status, "{error:?}");
        // Every real refusal carries text; the table's are empty strings standing for a
        // cause, so a message is checked with them filled in rather than not at all.
        if let HubError::Conflict { what } = &mut error {
            *what = "manifest v2 removed field `x`".to_owned();
        }
        assert!(!error.to_string().is_empty(), "{error:?} has no message");
    }
}

/// `child` is what makes a refusal name a path a reader with the file open can follow.
#[test]
fn a_path_grows_one_member_at_a_time() {
    assert_eq!(child("", "a"), "a");
    assert_eq!(child("a", "b"), "a.b");
    assert_eq!(child("a[0]", "b"), "a[0].b");
}

/// The defaults the spec fixes, spelled so a change to one is a diff here too.
#[test]
fn the_default_limits_are_the_specs_caps() {
    let limits = Limits::DEFAULT;
    assert_eq!(
        (
            limits.max_body,
            limits.max_batch,
            limits.max_record_bytes
        ),
        (4 << 20, 10_000, 1 << 20)
    );
    assert_eq!(VERSION, 1);
    assert_eq!(
        (MAX_COLLECTIONS, MAX_FIELDS, MAX_MANIFEST_BYTES, MAX_PLUGINS),
        (64, 256, 262_144, 64)
    );
}

/// `MAX_SEQ` is the last integer a JSON consumer holds exactly; a cursor or a rev past
/// it would be rounded by whoever reads it.
#[test]
fn the_seq_ceiling_is_two_to_the_fifty_three_minus_one() {
    assert_eq!(MAX_SEQ, 9_007_199_254_740_991);
    assert!(Cursor::parse(&format!("1.{MAX_SEQ}")).is_ok());
    assert!(Cursor::parse(&format!("1.{}", MAX_SEQ + 1)).is_err());
}

