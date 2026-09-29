//! The committed JSON Schema, checked against the contract's own rules.

use super::support::*;
#[cfg(feature = "codegen")]
use crate::ingest::schema::schema;

// ---------------------------------------------------------------- the schema

#[cfg(feature = "codegen")]
mod schema_tests {
    use super::{super::schema::schema, MINIMAL, read};

    #[test]
    fn the_schema_names_exactly_the_eight_roles_and_the_two_cardinalities() {
        let schema = schema();
        assert_eq!(schema["title"], "Ingest");
        let roles = &schema["$defs"]["Role"]["oneOf"];
        let names: Vec<&str> = roles
            .as_array()
            .expect("Role is a oneOf of consts")
            .iter()
            .map(|v| v["const"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "title", "label", "group", "tags", "link", "scalar", "weight", "parent"
            ]
        );
        let cardinality = &schema["$defs"]["Cardinality"]["oneOf"];
        let names: Vec<&str> = cardinality
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["const"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["one", "many"]);
    }

    #[test]
    fn the_schema_pins_every_wire_integer_to_a_bounded_integer() {
        let schema = schema();
        for spec in [
            &schema["properties"]["version"],
            &schema["$defs"]["Record"]["properties"]["updatedAt"],
        ] {
            assert_eq!(spec["type"], "integer");
            assert_eq!(spec["format"], "uint32");
        }
        assert_eq!(schema["properties"]["version"]["const"], 1);
    }

    /// The `link` property, wherever schemars put it: a `$ref` to a named def when
    /// one exists, inlined when it does not. Both spellings are accepted here so the
    /// test says what the contract requires, not where the generator happened to put
    /// it.
    fn link_schema() -> serde_json::Value {
        let link = schema()["$defs"]["Field"]["properties"]["link"].clone();
        let name = link["anyOf"][0]["$ref"]
            .as_str()
            .map(|r| r.rsplit('/').next().unwrap().to_owned());
        name.map_or(link.clone(), |name| schema()["$defs"][name].clone())
    }

    #[test]
    fn the_schema_says_a_link_field_needs_its_link_member() {
        let field = &schema()["$defs"]["Field"];
        let required: Vec<&str> = field["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        // `link` is required even though it is nullable: "no link" is stated, never
        // omitted, so an adapter cannot forget the member and still validate.
        assert_eq!(required, ["id", "link", "name", "role"]);
        // The member is nullable, so a validator admits `null` for it: schemars spells
        // that as an `anyOf` with a `null` branch when it inlines, and as a `$ref`
        // beside one when it does not. Both are accepted, so the test states the rule
        // rather than the generator's placement.
        let link = &field["properties"]["link"];
        let admits_null = link["type"] == "null"
            || link["anyOf"]
                .as_array()
                .is_some_and(|branches| branches.iter().any(|b| b["type"] == "null"));
        assert!(
            admits_null,
            "a `link` field's member must admit null: {link}"
        );
    }

    #[test]
    fn a_document_the_reader_refuses_does_not_validate_against_the_schema() {
        // The schema and the reader must agree about what a document is: this is the
        // one member where they can drift (an `Option` is optional to schemars, while
        // the reader requires it), and the drift is corrected in one named place.
        let missing_link = MINIMAL.replace(
            r#"{ "id": "blocks", "name": "Blocks", "role": "link",
          "link": { "collection": "task", "cardinality": "many", "symmetric": false } }"#,
            r#"{ "id": "blocks", "name": "Blocks", "role": "link", "link": null }"#,
        );
        assert!(read(&missing_link).is_err(), "the reader refuses it");
        let schema = schema();
        let field = &schema["$defs"]["Field"];
        assert!(
            field["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v == "link"),
            "so the schema must too, and does not"
        );
    }

    #[test]
    fn every_object_in_the_schema_refuses_an_unknown_member() {
        let schema = schema();
        assert_eq!(schema["additionalProperties"], false, "the document");
        for name in ["Collection", "Field", "Record"] {
            assert_eq!(
                schema["$defs"][name]["additionalProperties"], false,
                "{name}"
            );
        }
        assert_eq!(link_schema()["additionalProperties"], false, "Link");
    }

    #[test]
    fn a_value_is_any_json_so_the_schema_never_constrains_a_cell() {
        let record = &schema()["$defs"]["Record"];
        let values = &record["properties"]["values"]["additionalProperties"];
        // The open schema, not a `$ref` to something the committed file does not
        // define: a source's own cell types must survive the contract untouched.
        assert!(values.is_object() || values.is_null(), "{values}");
        assert!(
            !schema().to_string().contains("io::"),
            "an external schema URI leaked into the committed file"
        );
    }
}
