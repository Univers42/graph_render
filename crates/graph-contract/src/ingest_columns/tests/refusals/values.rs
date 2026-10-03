//! The refusals that are about a *value* rather than about the bytes around it: a string
//! index that names nothing, a boolean that is not `0` or `1`, a float that is not finite —
//! and the two cases that are not refusals at all but have to be pinned anyway, because a
//! decoder that quietly mishandled them would still pass every test above.

use super::*;

#[test]
fn a_nonzero_pad_byte_is_refused() {
    let mut e = full().encode();
    let pad = e.marks.pad;
    e.bytes[pad] = 1;
    assert_eq!(
        decode(&e.bytes).expect_err("a pad byte is zero by definition"),
        ColumnsError::NonZeroPadding { at: pad }
    );
}

#[test]
fn a_string_index_past_the_table_is_refused_in_every_required_column() {
    let marks = full().encode().marks;
    let at = full().string_count();
    for (column, name) in [
        (marks.node_id, "node id"),
        (marks.node_kind, "node kind"),
        (marks.node_source, "node source"),
        (marks.node_label, "node label"),
        (marks.edge_id, "edge id"),
        (marks.edge_kind, "edge kind"),
        (marks.edge_label, "edge label"),
    ] {
        assert_eq!(
            refused(move |e| e.patch_u32(column, 0, at)),
            ColumnsError::StringIndex {
                column: name,
                row: 0
            },
            "column {name}"
        );
    }
}

#[test]
fn a_string_index_past_the_table_is_refused_in_every_optional_column() {
    let marks = full().encode().marks;
    let at = full().string_count();
    for (column, name) in [
        (marks.node_database, "node database"),
        (marks.node_icon, "node icon"),
        (marks.record_id, "edge record_id"),
    ] {
        assert_eq!(
            refused(move |e| e.patch_u32(column, 0, at)),
            ColumnsError::StringIndex {
                column: name,
                row: 0
            },
            "column {name}"
        );
    }
}

#[test]
fn u32_max_in_a_required_column_is_refused() {
    let at = full().encode().marks.node_id;
    assert_eq!(
        refused(move |e| e.patch_u32(at, 0, u32::MAX)),
        ColumnsError::RequiredAbsent {
            column: "node id",
            row: 0,
        }
    );
}

#[test]
fn u32_max_in_an_optional_column_is_the_absent_marker_and_is_accepted() {
    let marks = full().encode().marks;
    let mut encoded = full().encode();
    encoded.patch_u32(marks.node_group, 0, u32::MAX);
    let doc = decode(&encoded.bytes).expect("absent is legal in an optional column");
    assert_eq!(doc.node(0).expect("row 0").group, None);
}

#[test]
fn an_endpoint_row_past_the_nodes_is_refused() {
    let marks = full().encode().marks;
    assert_eq!(
        refused(|e| e.patch_u32(marks.edge_target, 0, 2)),
        ColumnsError::EndpointRow {
            column: "edge target",
            row: 0,
        }
    );
    assert_eq!(
        refused(|e| e.patch_u32(marks.edge_source, 0, u32::MAX)),
        ColumnsError::EndpointRow {
            column: "edge source",
            row: 0,
        }
    );
}

#[test]
fn a_boolean_2_is_refused_in_every_boolean_column() {
    let marks = full().encode().marks;
    for (column, name) in [
        (marks.node_has_note, "node has_note"),
        (marks.directed, "edge directed"),
        (marks.child_first, "edge child_first"),
    ] {
        assert_eq!(
            refused(move |e| e.patch_u32(column, 0, 2)),
            ColumnsError::NotBoolean {
                column: name,
                row: 0,
                found: 2,
            },
            "column {name}"
        );
    }
}

#[test]
fn a_nan_or_an_infinity_is_refused_in_every_float_column() {
    let marks = full().encode().marks;
    let columns = [
        (marks.weight, "node weight"),
        (marks.version, "node version"),
        (marks.strength, "edge strength"),
    ];
    for (column, name) in columns {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                refused(move |e| e.patch_f64(column, 0, value)),
                ColumnsError::NotFinite {
                    column: name,
                    row: 0,
                },
                "column {name} value {value}"
            );
        }
    }
}

#[test]
fn a_negative_zero_and_a_subnormal_are_accepted() {
    let bytes = full().encode().bytes;
    assert_eq!(
        bytes.len() % 8,
        0,
        "the pad lands the columns on an eight-byte boundary"
    );
    let doc = decode(&bytes).expect("both are ordinary finite doubles");
    let node = doc.node(0).expect("row 0");
    assert!(node.weight.is_sign_negative() && node.weight == 0.0);
    assert_eq!(node.version, f64::from_bits(1));
}

#[test]
fn a_mutation_that_stays_legal_still_changes_what_the_rows_say() {
    // Not a refusal: a value the contract permits that means something else. Without this,
    // a `child_first` cell the checks ignored would still decode, and the whole differential
    // downstream would go on comparing two equal documents for ever.
    let before = full().encode();
    let doc = decode(&before.bytes).expect("valid");
    let mut after = full().encode();
    after.patch_u32(after.marks.child_first, 0, 0);
    let mutated = decode(&after.bytes).expect("0 is a legal boolean");
    assert!(doc.edge(0).expect("row 0").child_first);
    assert!(!mutated.edge(0).expect("row 0").child_first);
    assert_ne!(
        format!("{:?}", doc.edge(0)),
        format!("{:?}", mutated.edge(0))
    );
}
