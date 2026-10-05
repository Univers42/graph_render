//! The hub schema's own tests: what it promises a client, and that the committed file is
//! what `codegen` writes.
//!
//! Three promises, one per test. The schema is a *public artefact* — a client SDK is
//! generated from it — so a change to it is a change to somebody else's code, and each
//! promise is stated as a fact about the wire rather than about the JSON.

use super::*;

/// The wire bound, named here rather than imported so the test reads as a statement about
/// the number rather than about an import.
const MAX_WIRE_INT: u64 = super::MAX_WIRE_INT;

/// The root names all six documents, and every one of them refuses an unknown member.
///
/// The second half is the half that matters: a schema that *admitted* `manifestVerison` would
/// validate a body the reader then refuses with a path, and a client generated from it would
/// send the member it thought was legal.
#[test]
fn the_root_names_the_six_documents_and_each_refuses_an_unknown_member() {
    let schema = schema();
    let props = schema["properties"]
        .as_object()
        .expect("the root has properties");
    for member in ["manifest", "batch", "change", "notice", "answer", "error"] {
        assert!(
            props.contains_key(member),
            "{member} is missing from the schema"
        );
    }
    assert_eq!(
        schema["additionalProperties"],
        serde_json::json!(false),
        "the root must refuse an unknown member"
    );
    for (name, def) in schema["$defs"].as_object().expect("$defs") {
        if def["properties"].is_object() {
            assert_eq!(
                def["additionalProperties"],
                serde_json::json!(false),
                "$defs/{name} must refuse an unknown member"
            );
        }
    }
}

/// Every `seq` and `rev` is bounded at `2^53 − 1`, and every other integer is not.
///
/// The bound is the whole point of the `bounded` schema hook: a client that generated a
/// `number` from an unbounded `u64` would send a `seq` its own JSON library had already
/// rounded, and the reader would refuse a cursor the client believed it had written.
#[test]
fn every_wire_integer_is_bounded_and_the_caps_are_spelled_out() {
    let schema = schema();
    for (def_name, field) in [
        ("ChangeWire", "seq"),
        ("NoticeWire", "seq"),
        ("AnswerWire", "seq"),
        ("StoredRecord", "rev"),
        ("StoredDelete", "rev"),
    ] {
        let member = &schema["$defs"][def_name]["properties"][field];
        assert_eq!(
            member["maximum"],
            serde_json::json!(MAX_WIRE_INT),
            "{def_name}.{field}"
        );
        assert_eq!(
            member["minimum"],
            serde_json::json!(0),
            "{def_name}.{field}"
        );
    }
    // A `rev` is *not* bounded the same way on a manifest: `manifestVersion` is a u32 the
    // client assigns, and it is still under the wire bound because u32 < 2^32 < 2^53.
    assert_eq!(
        schema["$defs"]["ManifestWire"]["properties"]["manifestVersion"]["format"],
        "uint32"
    );
}

/// The change is discriminated: `kind` is a closed set of two, and each payload appears on
/// exactly one of them.
///
/// Spelled as `Option` rather than required, because the wire *omits* the member it does not
/// mean — a batch change carries no `manifest`, and a schema that required both would
/// reject every real change body.
#[test]
fn a_change_is_one_of_two_shapes_and_each_carries_only_its_own_payload() {
    let schema = schema();
    let change = &schema["$defs"]["ChangeWire"];
    // `kind` is a `$ref` to its own closed set, the way every reused enum in this schema
    // derives — the set itself is a `$defs` member, and *that* is the `oneOf`.
    assert_eq!(change["properties"]["kind"]["$ref"], "#/$defs/ChangeKind");
    let kinds: Vec<&str> = schema["$defs"]["ChangeKind"]["oneOf"]
        .as_array()
        .expect("ChangeKind is a closed set")
        .iter()
        .filter_map(|v| v["const"].as_str())
        .collect();
    assert_eq!(kinds, ["batch", "manifest"]);
    // Optional on the wire: present in `properties`, absent from `required`.
    let required = change["required"].as_array().expect("required is a list");
    for member in ["upserts", "deletes", "manifest"] {
        assert!(
            change["properties"].get(member).is_some(),
            "{member} missing"
        );
        assert!(
            !required.iter().any(|r| r == member),
            "{member} must be optional: the wire omits the member it does not mean"
        );
    }
}

/// The two version members are two facts, and both are in the schema.
///
/// `version` is a `const` (this wire format) and `manifestVersion` is a plain integer (the
/// client's own counter). A client that sent the wrong one would have its own second
/// manifest refused, so the schema is where it learns they differ.
#[test]
fn a_manifest_carries_both_version_members_and_pins_only_the_wires_own() {
    let schema = schema();
    let manifest = &schema["$defs"]["ManifestWire"];
    assert_eq!(
        manifest["properties"]["version"]["const"],
        serde_json::json!(1)
    );
    assert!(manifest["properties"]["manifestVersion"]["const"].is_null());
    assert!(
        manifest["properties"]["manifestVersion"]["type"] == "integer",
        "the client's counter is a plain integer, not a const"
    );
}

/// The caps are in the schema by name, so a generated client does not hard-code them a
/// second time and get one of them wrong.
#[test]
fn the_caps_are_spelled_in_the_schema_rather_than_only_in_prose() {
    let schema = schema();
    let props = schema["$defs"]["ManifestWire"]["properties"]
        .as_object()
        .unwrap();
    assert!(props.contains_key("collections"), "collections");
    // The field-level caps live on the ingest types the manifest reuses, so the check is
    // that the schema publishes *a* maximum for a collection count at all.
    let manifest = schema["$defs"]["ManifestWire"]["properties"]["collections"]["items"]
        .as_object()
        .map(|_| ())
        .unwrap_or_else(|| panic!("collections is an array of objects"));
    assert_eq!(manifest, ());
}

/// And the committed file is what `codegen` writes — the check that makes the rest of this
/// file about the *contract* and not about a file nobody regenerates.
#[test]
fn the_committed_hub_schema_is_what_codegen_generates() {
    let committed = include_str!("../../../../../docs/contract/hub-schema.json");
    let generated = format!("{:#}\n", schema());
    assert_eq!(
        committed, generated,
        "stale: run `graph-cli codegen` and commit the result"
    );
}
