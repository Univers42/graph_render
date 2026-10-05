//! The codegen module's own tests, in a file of their own by the house's 300-line limit
//! (`codegen.rs` is the writer, this is what holds it to the files it has already written —
//! the only way a *generated* artefact stays honest).

#[test]
fn schema_pins_every_wire_integer_to_uint32() {
    let schema = json_schema();
    for field in ["stage_count", "node_count", "edge_count"] {
        assert_eq!(schema["properties"][field]["format"], "uint32", "{field}");
    }
    let stage_count = &schema["properties"]["stage_count"];
    assert_eq!(
        (&stage_count["minimum"], &stage_count["maximum"]),
        (&1.into(), &1.into())
    );
    for field in ["major", "minor"] {
        assert_eq!(
            schema["$defs"]["FormatVersion"]["properties"][field]["format"],
            "uint32"
        );
    }
}
#[test]
fn schema_never_lists_a_reserved_kind_as_producible() {
    let text = json_schema().to_string();
    assert!(!text.contains("Ribbon") && !text.contains("\"Arc\""));
}
#[test]
fn typescript_is_declarations_only() {
    let ts = typescript();
    assert!(ts.contains("export type EdgeGeometryKind = \"Line\" | \"Polyline\" | \"Curve\";"));
    assert!(ts.contains("export type NodeGeometryKind = \"Point\" | \"Circle\" | \"Box\";"));
    assert!(ts.contains(
        "  /** Number of nodes, and the length of every node column. */\n  node_count: number;"
    ));
    assert!(ts.contains("  version: FormatVersion;"));
    for runtime in ["const ", "function", "enum ", "class ", "=>"] {
        assert!(
            !ts.contains(runtime),
            "runtime construct {runtime:?} in generated TS"
        );
    }
}
#[test]
fn the_committed_files_are_what_codegen_generates() {
    let committed = [
        include_str!("../../generated/snapshot-header.schema.json"),
        include_str!("../../generated/snapshot-header.d.ts"),
        include_str!("../../../../docs/contract/snapshot-schema.json"),
        include_str!("../../../../docs/contract/ingest-schema.json"),
        include_str!("../../generated/layout-params.schema.json"),
        include_str!("../../generated/layout-params.d.ts"),
    ];
    let stale = "stale: run `graph-cli codegen` and commit the result";
    for ((name, generated), committed) in outputs().iter().zip(committed) {
        assert_eq!(committed, generated, "{name} {stale}");
    }
    assert!(GENERATED_DIR.ends_with("graph-contract/generated"));
}
#[test]
fn ts_type_maps_every_json_schema_type_it_knows() {
    use super::typescript::ts_type;

    use serde_json::json;
    let cases = [
        (json!({"type": "integer"}), "number"),
        (json!({"type": "number"}), "number"),
        (json!({"type": "string"}), "string"),
        (json!({"type": "boolean"}), "boolean"),
        (
            json!({"type": "array", "items": {"type": "string"}}),
            "string[]",
        ),
        (json!({"$ref": "#/$defs/FormatVersion"}), "FormatVersion"),
        (json!({"type": "object"}), "unknown"),
    ];
    for (spec, want) in cases {
        assert_eq!(ts_type(&spec), want, "{spec}");
    }
}
