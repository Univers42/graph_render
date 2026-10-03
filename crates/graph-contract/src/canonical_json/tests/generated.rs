//! What `docs/contract/snapshot-schema.json` claims, read off the generator rather than
//! off the committed file: the shape has to admit exactly what the reader accepts.
//!
//! Split out of `tests.rs` by the house's 300-line limit, and because the schema is
//! emitted under the `codegen` feature while the rest of the file needs no feature.

#[cfg(feature = "codegen")]
#[test]
fn the_schema_types_a_z_column_as_an_array_and_never_requires_it() {
    let variants = crate::codegen::snapshot_schema()["$defs"]["NodeGeometry"]["oneOf"]
        .as_array()
        .expect("NodeGeometry is one of its three kinds")
        .clone();
    assert_eq!(variants.len(), 3, "{variants:?}");
    for variant in &variants {
        let kind = variant["properties"]["kind"]["const"]
            .as_str()
            .expect("named");
        let z = &variant["properties"]["z"];
        assert_eq!(z["type"], "array", "{kind}: a null is not an array");
        assert!(
            !variant["required"]
                .as_array()
                .expect("required")
                .iter()
                .any(|r| r == "z"),
            "{kind}: z is required iff dim is 1, which JSON Schema cannot say"
        );
    }
}
